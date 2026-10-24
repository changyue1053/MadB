//! Static, serial, fail-fast execution of already bound recipes. This does not
//! discover dependencies, commit outputs, or certify an SDK.
use std::collections::{BTreeMap, BTreeSet};

pub use crate::dag_control::GraphControl;
use crate::{
    ErrorKind, Id, LogicalPath, Phase, QueryError, Result, Task, TaskGraph,
    inputs::{FrozenInputs, capture_inputs, validate_inventory, validate_paths},
    ports::{FileReference, ReadWorkspace},
    process::ExecutionControl,
    task::{BuildRecipe, ValidatedTaskResult},
};

const NODE_LIMIT: usize = 64;
const GRAPH_BYTES_LIMIT: usize = 256 * 1024 * 1024;

/// A recipe with explicit planning metadata. Like BuildRecipe, implementations
/// must keep the bound invocation and dependencies immutable during execution.
pub trait GraphRecipe: BuildRecipe {
    fn phase(&self) -> Phase;
}

/// A fully checked graph or named dependency closure. Only metadata is exposed;
/// callers cannot retrieve a recipe and accidentally bypass its prerequisites.
pub struct ExecutionGraph<R> {
    tasks: Vec<Task>,
    recipes: Vec<R>,
    owners: BTreeMap<LogicalPath, Id>,
    expected: BTreeMap<LogicalPath, FileReference>,
    sources: Vec<FileReference>,
    reserved_bytes: usize,
}
impl<R: GraphRecipe> ExecutionGraph<R> {
    pub fn new(recipes: Vec<R>, target: Option<&Id>, active: &[Id]) -> Result<Self> {
        let mut metadata = Vec::with_capacity(recipes.len());
        let mut expected = BTreeMap::<LogicalPath, FileReference>::new();
        for recipe in &recipes {
            let invocation = recipe.invocation();
            invocation.validate()?;
            metadata.push(Task {
                id: invocation.id.clone(),
                phase: recipe.phase(),
                dependencies: recipe.dependencies().to_vec(),
                outputs: invocation.outputs.clone(),
            });
            for reference in &invocation.inputs {
                if let Some(previous) = expected.insert(reference.path.clone(), reference.clone())
                    && previous != *reference
                {
                    return Err(graph_error(
                        "DAG001",
                        reference.path.as_str(),
                        "one logical input has inconsistent expected bytes",
                    ));
                }
            }
        }
        let complete = TaskGraph::new(metadata)
            .map_err(|error| graph_error("DAG001", "graph", error.to_string()))?;
        let owners: BTreeMap<_, _> = complete
            .tasks()
            .iter()
            .flat_map(|task| {
                task.outputs
                    .iter()
                    .map(|path| (path.clone(), task.id.clone()))
            })
            .collect();
        // Check the whole graph before selecting a closure. A missing edge to
        // an unselected producer must never turn its output into source input.
        let paths: BTreeSet<_> = owners.keys().chain(expected.keys()).cloned().collect();
        validate_paths(&paths)?;
        for recipe in &recipes {
            for input in &recipe.invocation().inputs {
                if let Some(owner) = owners.get(&input.path)
                    && !recipe.dependencies().contains(owner)
                {
                    return Err(graph_error(
                        "DAG001",
                        recipe.invocation().id.as_str(),
                        format!(
                            "input {} requires direct dependency {owner}",
                            input.path.as_str()
                        ),
                    ));
                }
            }
        }
        let selected = if let Some(target) = target {
            complete.subgraph(target, active)
        } else {
            complete.reject_active(active).map(|()| complete)
        }
        .map_err(|error| graph_error("DAG001", "graph", error.to_string()))?;
        if selected.tasks().len() > NODE_LIMIT {
            return Err(graph_error(
                "DAG002",
                "graph",
                "selected graph exceeds 64 nodes",
            ));
        }
        let mut by_id: BTreeMap<_, _> = recipes
            .into_iter()
            .map(|recipe| (recipe.invocation().id.clone(), recipe))
            .collect();
        let recipes: Vec<_> = selected
            .tasks()
            .iter()
            .map(|task| by_id.remove(&task.id).expect("validated task identity"))
            .collect();
        let mut sources = BTreeMap::new();
        // Full-graph identities were checked above. Only selected consumers
        // constrain execution results in this named dependency closure.
        let mut expected = BTreeMap::new();
        let mut reserved_bytes = 0;
        for recipe in &recipes {
            let (_, size) = validate_inventory(&recipe.invocation().inputs)?;
            reserve(&mut reserved_bytes, size)?;
            for input in &recipe.invocation().inputs {
                expected.insert(input.path.clone(), input.clone());
                if !owners.contains_key(&input.path) {
                    sources.insert(input.path.clone(), input.clone());
                }
            }
        }
        let sources: Vec<_> = sources.into_values().collect();
        if !sources.is_empty() {
            let (_, size) = validate_inventory(&sources)?;
            reserve(&mut reserved_bytes, size)?;
        }
        Ok(Self {
            tasks: selected.tasks().to_vec(),
            recipes,
            owners,
            expected,
            sources,
            reserved_bytes,
        })
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }
}

/// Host ownership survives both failed preparation and process cleanup gaps.
/// Implementations must transfer the result exactly once, so the retained node
/// cannot expose it after the graph rejects its aggregate results.
pub trait NodeRun {
    fn take_value(&mut self) -> Option<ValidatedTaskResult>;
}

/// Host execution owns directories, processes and diagnostics. A preparation
/// failure is still a Run, including any allocated directory's retained owner.
pub trait GraphExecutor<R: GraphRecipe> {
    type Run: NodeRun;
    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> Self::Run;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphStatus {
    Validated,
    Cancelled,
    Failed,
}

/// Historical node observations remain available after failure/cancellation.
/// Only a fully accepted graph exposes its result map; this is not a commit.
#[must_use]
pub struct GraphRun<N> {
    pub status: GraphStatus,
    pub failure: Option<QueryError>,
    pub failed_task: Option<Id>,
    pub skipped: Vec<Id>,
    nodes: Vec<(Id, N)>,
    results: Option<BTreeMap<Id, ValidatedTaskResult>>,
}
impl<N> GraphRun<N> {
    pub fn nodes(&self) -> &[(Id, N)] {
        &self.nodes
    }
    pub fn nodes_mut(&mut self) -> &mut [(Id, N)] {
        &mut self.nodes
    }
    pub fn results(&self) -> Option<&BTreeMap<Id, ValidatedTaskResult>> {
        self.results.as_ref()
    }
}

/// Capture the external source union before the first node, then execute in
/// deterministic topological order. Err means admission was refused; every
/// ordinary failure after admission returns a terminal run retaining owners.
/// Panics unwind normally, while the guard permanently closes graph admission.
pub fn execute_graph<R: GraphRecipe, W: ReadWorkspace, E: GraphExecutor<R>>(
    graph: &ExecutionGraph<R>,
    source: &W,
    executor: &mut E,
    control: &GraphControl,
) -> Result<GraphRun<E::Run>> {
    let guard = control.begin()?;
    let mut run = GraphRun {
        status: GraphStatus::Validated,
        failure: None,
        failed_task: None,
        skipped: Vec::new(),
        nodes: Vec::new(),
        results: None,
    };
    let mut results = BTreeMap::new();
    if !guard.cancelled() {
        let frozen = if graph.sources.is_empty() {
            Ok(None)
        } else {
            capture_inputs(source, graph.sources.clone()).map(Some)
        };
        match frozen {
            Err(error) => run.failure = Some(error),
            Ok(frozen) => {
                let mut result_limit = GRAPH_BYTES_LIMIT - graph.reserved_bytes;
                for recipe in &graph.recipes {
                    if guard.cancelled() {
                        break;
                    }
                    let id = recipe.invocation().id.clone();
                    let inputs = assemble_inputs(graph, recipe, frozen.as_ref(), &results);
                    let inputs = match inputs {
                        Ok(inputs) => inputs,
                        Err(error) => {
                            run.failure = Some(error);
                            run.failed_task = Some(id);
                            break;
                        }
                    };
                    let Some(node_control) = guard.activate() else {
                        break;
                    };
                    let mut node = executor.execute(recipe, inputs, &node_control, result_limit);
                    guard.deactivate();
                    let value = node.take_value();
                    run.nodes.push((id.clone(), node));
                    if guard.cancelled() {
                        run.failed_task = Some(id);
                        break;
                    }
                    match check_result(graph, recipe, value, result_limit) {
                        Ok(value) => {
                            result_limit -= value.files().total_bytes();
                            results.insert(id, value);
                        }
                        Err(error) => {
                            run.failure = Some(error);
                            run.failed_task = Some(id);
                            break;
                        }
                    }
                }
            }
        }
    }
    let completed: BTreeSet<_> = run.nodes.iter().map(|(id, _)| id).collect();
    run.skipped = graph
        .tasks
        .iter()
        .filter(|task| !completed.contains(&task.id))
        .map(|task| task.id.clone())
        .collect();
    // Cancellation and the last acceptance are ordered by one graph lock.
    run.status = if guard.finish() {
        GraphStatus::Cancelled
    } else if run.failure.is_some() {
        GraphStatus::Failed
    } else {
        GraphStatus::Validated
    };
    if run.status == GraphStatus::Validated {
        run.results = Some(results);
    }
    Ok(run)
}

fn assemble_inputs<R: GraphRecipe>(
    graph: &ExecutionGraph<R>,
    recipe: &R,
    source: Option<&FrozenInputs>,
    results: &BTreeMap<Id, ValidatedTaskResult>,
) -> Result<FrozenInputs> {
    let mut files = BTreeMap::new();
    for expected in &recipe.invocation().inputs {
        let file = if let Some(owner) = graph.owners.get(&expected.path) {
            results.get(owner).and_then(|result| {
                result
                    .outputs()
                    .find(|(reference, _)| reference.path == expected.path)
            })
        } else {
            source.and_then(|source| {
                source
                    .files()
                    .find(|(reference, _)| reference.path == expected.path)
            })
        };
        let Some((actual, bytes)) = file else {
            return Err(graph_error(
                "DAG003",
                expected.path.as_str(),
                "declared frozen input is unavailable",
            ));
        };
        if actual != expected {
            return Err(graph_error(
                "DAG003",
                expected.path.as_str(),
                "producer bytes do not match the consumer's fixed input identity",
            ));
        }
        files.insert(&expected.path, bytes);
    }
    capture_inputs(&FrozenWorkspace(files), recipe.invocation().inputs.clone())
}

struct FrozenWorkspace<'a>(BTreeMap<&'a LogicalPath, &'a [u8]>);
impl ReadWorkspace for FrozenWorkspace<'_> {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        let Some(bytes) = self.0.get(path) else {
            return Ok(None);
        };
        if bytes.len() > limit {
            return Err(graph_error(
                "DAG003",
                path.as_str(),
                "frozen file exceeds the requested byte budget",
            ));
        }
        Ok(Some(bytes.to_vec()))
    }
}

fn check_result<R: GraphRecipe>(
    graph: &ExecutionGraph<R>,
    recipe: &R,
    value: Option<ValidatedTaskResult>,
    limit: usize,
) -> Result<ValidatedTaskResult> {
    let value = value.ok_or_else(|| {
        graph_error(
            "DAG003",
            recipe.invocation().id.as_str(),
            "node did not provide a validated result; inspect retained node diagnostics",
        )
    })?;
    if value.files().total_bytes() > limit {
        return Err(graph_error(
            "DAG002",
            recipe.invocation().id.as_str(),
            "node returned results beyond its remaining graph budget",
        ));
    }
    let outputs: BTreeSet<_> = value
        .outputs()
        .map(|(reference, _)| &reference.path)
        .collect();
    if outputs != recipe.invocation().outputs.iter().collect() {
        return Err(graph_error(
            "DAG003",
            recipe.invocation().id.as_str(),
            "node result does not match its declared output set",
        ));
    }
    for (reference, _) in value.outputs() {
        if let Some(expected) = graph.expected.get(&reference.path)
            && reference != expected
        {
            return Err(graph_error(
                "DAG003",
                reference.path.as_str(),
                "producer result differs from the fixed consumer identity",
            ));
        }
    }
    Ok(value)
}

fn reserve(total: &mut usize, additional: usize) -> Result<()> {
    *total = total
        .checked_add(additional)
        .filter(|sum| *sum <= GRAPH_BYTES_LIMIT)
        .ok_or_else(|| graph_error("DAG002", "graph", "retained graph files exceed 256 MiB"))?;
    Ok(())
}
fn graph_error(
    code: &'static str,
    subject: impl Into<String>,
    message: impl Into<String>,
) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}
