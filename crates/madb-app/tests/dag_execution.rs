use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, Phase, QueryError, Result,
    dag::{
        ExecutionGraph, GraphControl, GraphExecutor, GraphRecipe, GraphRun, GraphStatus, NodeRun,
        execute_graph,
    },
    inputs::FrozenInputs,
    ports::{FileReference, ProcessRunner, ReadWorkspace},
    process::{
        CancelDecision, CancellationProbe, CapturedOutput, ExecutionControl, ProcessCleanup,
        ProcessExit, ProcessReport, ProcessRequest, ProcessResult, ProcessRun, ResultAdmission,
        Termination, execute_process,
    },
    task::{
        BuildRecipe, TaskFileReferences, TaskInvocation, ValidatedTaskResult,
        capture_task_result_limited,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

const GRAPH_BYTES: usize = 256 * 1024 * 1024;

fn id(name: &str) -> Id {
    Id::new(name).unwrap()
}
fn path(name: &str) -> LogicalPath {
    LogicalPath::new(name).unwrap()
}
fn reference(name: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: path(name),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn code(error: &QueryError) -> &'static str {
    error.findings[0].code
}

#[derive(Default)]
struct Workspace {
    files: RefCell<BTreeMap<LogicalPath, Vec<u8>>>,
    reads: RefCell<Vec<(String, usize)>>,
    on_read: RefCell<Option<Box<dyn FnOnce()>>>,
}
impl Workspace {
    fn put(&self, name: &str, bytes: &[u8]) {
        self.files.borrow_mut().insert(path(name), bytes.to_vec());
    }
    fn source() -> Self {
        let workspace = Self::default();
        workspace.put("src/seed", b"x");
        workspace
    }
}
impl ReadWorkspace for Workspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.reads.borrow_mut().push((path.as_str().into(), limit));
        let hook = self.on_read.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
        let bytes = self.files.borrow().get(path).cloned();
        if let Some(bytes) = &bytes {
            assert!(bytes.len() <= limit);
        }
        Ok(bytes)
    }
}

struct Recipe {
    invocation: TaskInvocation,
    dependencies: Vec<Id>,
    references: TaskFileReferences,
    suffix: Vec<u8>,
    invalid_payload: bool,
}
impl BuildRecipe for Recipe {
    fn invocation(&self) -> &TaskInvocation {
        &self.invocation
    }
    fn dependencies(&self) -> &[Id] {
        &self.dependencies
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<TaskFileReferences> {
        assert_eq!(stdout, b"checked payload");
        if self.invalid_payload {
            return Err(QueryError::new(
                ErrorKind::InvalidInput,
                "TEST_PAYLOAD",
                "result",
                "invalid payload",
            ));
        }
        Ok(self.references.clone())
    }
}
impl GraphRecipe for Recipe {
    fn phase(&self) -> Phase {
        Phase::Generate
    }
}
fn recipe(
    name: &str,
    inputs: &[(&str, &[u8])],
    dependencies: &[&str],
    suffix: &[u8],
    output: &[u8],
) -> Recipe {
    let output_path = format!("out/{name}");
    Recipe {
        invocation: TaskInvocation {
            id: id(name),
            process: ProcessRequest {
                tool: reference("tool", b"tool"),
                working_directory: path("work"),
                arguments: vec![],
                environment: BTreeMap::new(),
                stdin: vec![],
            },
            inputs: inputs
                .iter()
                .map(|(name, bytes)| reference(name, bytes))
                .collect(),
            outputs: vec![path(&output_path)],
        },
        dependencies: dependencies.iter().map(|name| id(name)).collect(),
        references: TaskFileReferences {
            outputs: vec![reference(&output_path, output)],
            logs: vec![reference(&format!("logs/{name}"), b"log")],
        },
        suffix: suffix.to_vec(),
        invalid_payload: false,
    }
}
fn leaf(name: &str) -> Recipe {
    recipe(name, &[("src/seed", b"x")], &[], b"", b"x")
}
fn chain() -> Vec<Recipe> {
    vec![
        recipe("a", &[("src/seed", b"x")], &[], b"A", b"xA"),
        recipe("b", &[("out/a", b"xA")], &["a"], b"B", b"xAB"),
        recipe("c", &[("out/b", b"xAB")], &["b"], b"C", b"xABC"),
    ]
}
fn graph(recipes: Vec<Recipe>) -> ExecutionGraph<Recipe> {
    ExecutionGraph::new(recipes, None, &[]).unwrap()
}
fn reject(recipes: Vec<Recipe>, expected: &str) {
    let error = ExecutionGraph::new(recipes, None, &[]).err().unwrap();
    assert_eq!(code(&error), expected);
}

fn transform(
    recipe: &Recipe,
    inputs: &FrozenInputs,
    _: &ExecutionControl,
    limit: usize,
) -> Result<ValidatedTaskResult> {
    let workspace = transformed_workspace(recipe, inputs);
    capture_task_result_limited(&workspace, recipe, b"checked payload", limit)
}
fn transformed_workspace(recipe: &Recipe, inputs: &FrozenInputs) -> Workspace {
    let mut bytes: Vec<_> = inputs
        .files()
        .flat_map(|(_, bytes)| bytes.iter().copied())
        .collect();
    bytes.extend_from_slice(&recipe.suffix);
    let workspace = Workspace::default();
    workspace.put(recipe.invocation.outputs[0].as_str(), &bytes);
    workspace.put(recipe.references.logs[0].path.as_str(), b"log");
    workspace
}

struct Owner(Rc<Cell<usize>>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
struct Node {
    value: Option<ValidatedTaskResult>,
    diagnostic: Option<QueryError>,
    was_validated: bool,
    on_take: Option<Box<dyn FnOnce()>>,
    _owner: Owner,
}
impl NodeRun for Node {
    fn take_value(&mut self) -> Option<ValidatedTaskResult> {
        let value = self.value.take();
        if let Some(hook) = self.on_take.take() {
            hook();
        }
        value
    }
}
struct Executor<F> {
    action: F,
    calls: Vec<Id>,
    limits: Vec<usize>,
    owners: Rc<Cell<usize>>,
    on_take: Option<Box<dyn FnOnce()>>,
}
impl<F> Executor<F> {
    fn new(action: F) -> Self {
        Self {
            action,
            calls: vec![],
            limits: vec![],
            owners: Rc::new(Cell::new(0)),
            on_take: None,
        }
    }
}
impl<F> GraphExecutor<Recipe> for Executor<F>
where
    F: FnMut(&Recipe, &FrozenInputs, &ExecutionControl, usize) -> Result<ValidatedTaskResult>,
{
    type Run = Node;
    fn execute(
        &mut self,
        recipe: &Recipe,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        limit: usize,
    ) -> Node {
        self.calls.push(recipe.invocation.id.clone());
        self.limits.push(limit);
        self.owners.set(self.owners.get() + 1);
        let owner = Owner(self.owners.clone());
        let result = (self.action)(recipe, &inputs, control, limit);
        let was_validated = result.is_ok();
        let (value, diagnostic) = match result {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(error)),
        };
        Node {
            value,
            diagnostic,
            was_validated,
            on_take: self.on_take.take(),
            _owner: owner,
        }
    }
}
fn assert_drained(run: &mut GraphRun<Node>) {
    assert!(
        run.nodes_mut()
            .iter_mut()
            .all(|(_, node)| node.take_value().is_none())
    );
}
fn names(ids: &[Id]) -> Vec<&str> {
    ids.iter().map(Id::as_str).collect()
}
fn output<'a>(run: &'a GraphRun<Node>, name: &str) -> &'a [u8] {
    run.results().unwrap()[&id(name)]
        .outputs()
        .next()
        .unwrap()
        .1
}

#[test]
fn producer_bytes_flow_to_consumer_without_reading_a_same_named_source_file() {
    let source = Workspace::source();
    source.put("out/a", b"stale");
    source.put("out/b", b"stale");
    let mut executor = Executor::new(transform);
    let mut run = execute_graph(
        &graph(chain()),
        &source,
        &mut executor,
        &GraphControl::default(),
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(names(&executor.calls), ["a", "b", "c"]);
    assert_eq!(output(&run, "a"), b"xA");
    assert_eq!(output(&run, "c"), b"xABC");
    assert!(run.skipped.is_empty());
    assert!(run.failure.is_none());
    assert_eq!(
        *source.reads.borrow(),
        [("src/seed".into(), 1), ("src/seed".into(), 1)]
    );
    // One root copy plus every selected node's input copy, then cumulative outputs/logs.
    assert_eq!(
        executor.limits,
        [GRAPH_BYTES - 7, GRAPH_BYTES - 12, GRAPH_BYTES - 18]
    );
    assert_eq!(executor.owners.get(), 3);
    assert_drained(&mut run);
    drop(run);
    assert_eq!(executor.owners.get(), 0);
}

#[test]
fn shuffled_fork_join_runs_in_deterministic_order_and_joins_both_outputs() {
    let recipes = vec![
        recipe(
            "join",
            &[("out/right", b"xAR"), ("out/left", b"xAL")],
            &["right", "left"],
            b"J",
            b"xALxARJ",
        ),
        recipe("right", &[("out/a", b"xA")], &["a"], b"R", b"xAR"),
        recipe("left", &[("out/a", b"xA")], &["a"], b"L", b"xAL"),
        recipe("a", &[("src/seed", b"x")], &[], b"A", b"xA"),
    ];
    let graph = graph(recipes);
    assert_eq!(
        graph
            .tasks()
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "left", "right", "join"]
    );
    let mut executor = Executor::new(transform);
    let run = execute_graph(
        &graph,
        &Workspace::source(),
        &mut executor,
        &GraphControl::default(),
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(names(&executor.calls), ["a", "left", "right", "join"]);
    assert_eq!(output(&run, "join"), b"xALxARJ");
}

#[test]
fn shared_source_is_captured_once_before_any_node_and_stays_frozen() {
    let source = Workspace::source();
    let graph = graph(vec![
        recipe("a", &[("src/seed", b"x")], &[], b"A", b"xA"),
        recipe(
            "b",
            &[("src/seed", b"x"), ("out/a", b"xA")],
            &["a"],
            b"B",
            b"xAxB",
        ),
    ]);
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, control: &ExecutionControl, limit| {
            assert_eq!(source.reads.borrow().len(), 2);
            source.put("src/seed", b"changed after graph capture");
            transform(recipe, inputs, control, limit)
        },
    );
    let run = execute_graph(&graph, &source, &mut executor, &GraphControl::default()).unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(output(&run, "b"), b"xAxB");
    assert_eq!(source.reads.borrow().len(), 2);
}

#[test]
fn named_closure_ignores_unrelated_source_io_and_unrelated_active_tasks() {
    let mut recipes = chain();
    recipes.push(recipe(
        "unused",
        &[("missing/unrelated", b"missing")],
        &[],
        b"",
        b"missing",
    ));
    let graph = ExecutionGraph::new(recipes, Some(&id("b")), &[id("unused")]).unwrap();
    assert_eq!(graph.tasks().len(), 2);
    let source = Workspace::source();
    let mut executor = Executor::new(transform);
    let run = execute_graph(&graph, &source, &mut executor, &GraphControl::default()).unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(names(&executor.calls), ["a", "b"]);
    assert_eq!(run.results().unwrap().len(), 2);
    assert!(
        source
            .reads
            .borrow()
            .iter()
            .all(|(name, _)| name == "src/seed")
    );
}

#[test]
fn only_selected_consumers_constrain_runtime_output_identity() {
    for target in [Some(id("a")), Some(id("b")), None] {
        let recipes = vec![
            recipe("a", &[("src/seed", b"x")], &[], b"A", b"xA"),
            recipe("b", &[("out/a", b"wrong")], &["a"], b"", b"wrong"),
        ];
        let graph = ExecutionGraph::new(recipes, target.as_ref(), &[]).unwrap();
        let mut executor = Executor::new(transform);
        let mut run = execute_graph(
            &graph,
            &Workspace::source(),
            &mut executor,
            &GraphControl::default(),
        )
        .unwrap();
        assert_eq!(names(&executor.calls), ["a"]);
        if target.as_ref() == Some(&id("a")) {
            assert_eq!(run.status, GraphStatus::Validated);
            assert_eq!(output(&run, "a"), b"xA");
            assert!(run.failure.is_none());
            assert!(run.skipped.is_empty());
        } else {
            assert_eq!(run.status, GraphStatus::Failed);
            assert_eq!(code(run.failure.as_ref().unwrap()), "DAG003");
            assert_eq!(run.failed_task, Some(id("a")));
            assert_eq!(names(&run.skipped), ["b"]);
            assert!(run.results().is_none());
        }
        assert_drained(&mut run);
    }
}

#[test]
fn missing_direct_edge_cannot_turn_an_unselected_producer_output_into_source() {
    let source = Workspace::source();
    source.put("out/a", b"xA");
    for target in [Some(id("a")), Some(id("b")), None] {
        let mut recipes = chain();
        recipes[1].dependencies.clear();
        let error = ExecutionGraph::new(recipes, target.as_ref(), &[])
            .err()
            .unwrap();
        assert_eq!(code(&error), "DAG001");
        assert!(source.reads.borrow().is_empty());
    }
}

#[test]
fn complete_graph_structure_is_checked_before_selecting_a_named_closure() {
    for case in 0..5 {
        let mut recipes = vec![leaf("selected"), leaf("unused")];
        match case {
            0 => recipes[1].dependencies.push(id("missing")),
            1 => recipes[1].dependencies = vec![id("selected"), id("selected")],
            2 => recipes[1].dependencies.push(id("unused")),
            3 => recipes[1].invocation.id = id("selected"),
            _ => recipes[1].invocation.outputs = recipes[0].invocation.outputs.clone(),
        }
        let error = ExecutionGraph::new(recipes, Some(&id("selected")), &[])
            .err()
            .unwrap();
        assert_eq!(code(&error), "DAG001", "case {case}");
    }
    for (target, active) in [
        (Some(id("missing")), vec![]),
        (Some(id("b")), vec![id("a")]),
        (None, vec![id("c")]),
    ] {
        let error = ExecutionGraph::new(chain(), target.as_ref(), &active)
            .err()
            .unwrap();
        assert_eq!(code(&error), "DAG001");
    }
}

#[test]
fn shared_input_hash_or_size_disagreement_is_rejected_even_outside_selected_closure() {
    for different_size in [false, true] {
        let mut recipes = vec![leaf("a"), leaf("b")];
        if different_size {
            recipes[1].invocation.inputs[0].size += 1;
        } else {
            recipes[1].invocation.inputs[0].digest = Digest::of_bytes(b"y");
        }
        let error = ExecutionGraph::new(recipes, Some(&id("a")), &[])
            .err()
            .unwrap();
        assert_eq!(code(&error), "DAG001");
    }
}

#[test]
fn cross_node_case_aliases_and_file_directory_conflicts_are_rejected() {
    for conflicting in ["OUT/b", "out", "out/a/nested"] {
        let mut recipes = vec![leaf("a"), leaf("b")];
        recipes[1].invocation.outputs = vec![path(conflicting)];
        reject(recipes, "DAG001");
    }
    for input in ["OUT/a", "out", "out/a/nested", "SRC/other"] {
        let mut recipes = vec![leaf("a"), leaf("b")];
        recipes[1].invocation.inputs = vec![reference(input, b"x")];
        reject(recipes, "INPUT001");
    }
}

#[test]
fn node_and_reserved_byte_limits_are_checked_without_allocating_large_files() {
    let nodes = |count| {
        (0..count)
            .map(|index| leaf(&format!("task-{index:02}")))
            .collect()
    };
    assert_eq!(graph(nodes(64)).tasks().len(), 64);
    reject(nodes(65), "DAG002");
    assert_eq!(
        ExecutionGraph::new(nodes(65), Some(&id("task-00")), &[])
            .unwrap()
            .tasks()
            .len(),
        1
    );
    let large_nodes = |count| {
        (0..count)
            .map(|index| {
                let mut recipe = leaf(&format!("task-{index}"));
                recipe.invocation.inputs[0].size = 32 * 1024 * 1024;
                recipe
            })
            .collect()
    };
    // Seven copies plus the single root inventory exactly fill 256 MiB.
    assert_eq!(graph(large_nodes(7)).tasks().len(), 7);
    reject(large_nodes(8), "DAG002");
}

#[test]
fn producer_identity_mismatch_stops_before_consumer_and_never_uses_matching_stale_source() {
    for expected in [b"zA".as_slice(), b"xAA"] {
        let source = Workspace::source();
        source.put("out/a", expected);
        let recipes = vec![
            recipe("a", &[("src/seed", b"x")], &[], b"A", b"xA"),
            recipe("b", &[("out/a", expected)], &["a"], b"", expected),
        ];
        let mut executor = Executor::new(transform);
        let mut run = execute_graph(
            &graph(recipes),
            &source,
            &mut executor,
            &GraphControl::default(),
        )
        .unwrap();
        assert_eq!(run.status, GraphStatus::Failed);
        assert_eq!(code(run.failure.as_ref().unwrap()), "DAG003");
        assert_eq!(run.failed_task, Some(id("a")));
        assert_eq!(names(&executor.calls), ["a"]);
        assert_eq!(names(&run.skipped), ["b"]);
        assert!(run.results().is_none());
        assert!(run.nodes()[0].1.was_validated);
        assert!(
            source
                .reads
                .borrow()
                .iter()
                .all(|(name, _)| name == "src/seed")
        );
        assert_eq!(executor.owners.get(), 1);
        assert_drained(&mut run);
    }
}

#[test]
fn preparation_or_result_failure_preserves_started_owners_and_discards_all_values() {
    for invalid_payload in [false, true] {
        let mut recipes = chain();
        recipes[1].invalid_payload = invalid_payload;
        let mut executor = Executor::new(
            |recipe: &Recipe, inputs: &FrozenInputs, control: &ExecutionControl, limit| {
                if recipe.invocation.id == id("b") && !invalid_payload {
                    return Err(QueryError::new(
                        ErrorKind::Host,
                        "TEST_PREPARE",
                        "directory",
                        "preparation failed",
                    ));
                }
                transform(recipe, inputs, control, limit)
            },
        );
        let mut run = execute_graph(
            &graph(recipes),
            &Workspace::source(),
            &mut executor,
            &GraphControl::default(),
        )
        .unwrap();
        assert_eq!(run.status, GraphStatus::Failed);
        assert_eq!(run.failed_task, Some(id("b")));
        assert_eq!(code(run.failure.as_ref().unwrap()), "DAG003");
        assert_eq!(names(&executor.calls), ["a", "b"]);
        assert_eq!(names(&run.skipped), ["c"]);
        assert!(run.nodes()[0].1.was_validated);
        assert!(!run.nodes()[1].1.was_validated);
        assert_eq!(
            code(run.nodes()[1].1.diagnostic.as_ref().unwrap()),
            if invalid_payload {
                "TEST_PAYLOAD"
            } else {
                "TEST_PREPARE"
            }
        );
        assert!(run.results().is_none());
        assert_eq!(executor.owners.get(), 2);
        assert_drained(&mut run);
        drop(run);
        assert_eq!(executor.owners.get(), 0);
    }
}

#[test]
fn source_capture_failure_and_logs_used_as_inputs_never_start_a_node() {
    for log_input in [false, true] {
        let mut recipes = chain();
        if log_input {
            recipes[1].invocation.inputs = vec![reference("logs/a", b"log")];
        }
        let source = if log_input {
            Workspace::source()
        } else {
            Workspace::default()
        };
        let control = GraphControl::default();
        let mut executor = Executor::new(transform);
        let run = execute_graph(&graph(recipes), &source, &mut executor, &control).unwrap();
        assert_eq!(run.status, GraphStatus::Failed);
        assert_eq!(code(run.failure.as_ref().unwrap()), "INPUT003");
        assert!(run.failed_task.is_none());
        assert_eq!(names(&run.skipped), ["a", "b", "c"]);
        assert!(executor.calls.is_empty());
        assert!(run.nodes().is_empty());
        assert!(run.results().is_none());
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    }
}

#[test]
fn pre_cancelled_graph_skips_source_io_and_every_node() {
    let source = Workspace::source();
    let control = GraphControl::default();
    assert_eq!(control.cancel(), CancelDecision::Requested);
    assert_eq!(control.clone().cancel(), CancelDecision::AlreadyRequested);
    let graph = graph(chain());
    let mut executor = Executor::new(transform);
    let run = execute_graph(&graph, &source, &mut executor, &control).unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(names(&run.skipped), ["a", "b", "c"]);
    assert!(run.results().is_none());
    assert!(run.failure.is_none());
    assert!(source.reads.borrow().is_empty());
    assert!(executor.calls.is_empty());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert_eq!(
        code(
            &execute_graph(&graph, &source, &mut executor, &control)
                .err()
                .unwrap()
        ),
        "DAG001"
    );
}

#[test]
fn cancellation_during_source_capture_never_starts_a_node() {
    let source = Workspace::source();
    let control = GraphControl::default();
    let cancel = control.clone();
    *source.on_read.borrow_mut() = Some(Box::new(move || {
        assert_eq!(cancel.cancel(), CancelDecision::Requested);
    }));
    let mut executor = Executor::new(transform);
    let mut run = execute_graph(&graph(chain()), &source, &mut executor, &control).unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert!(!source.reads.borrow().is_empty());
    assert!(executor.calls.is_empty());
    assert_eq!(names(&run.skipped), ["a", "b", "c"]);
    assert!(run.nodes().is_empty());
    assert!(run.results().is_none());
    assert!(run.failure.is_none());
    assert_drained(&mut run);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

struct Runner<F>(F);
impl<F: Fn(&CancellationProbe<'_>)> ProcessRunner for Runner<F> {
    fn run(&self, _: &ProcessRequest, probe: &CancellationProbe<'_>) -> ProcessResult<ProcessRun> {
        (self.0)(probe);
        Ok(ProcessRun {
            report: ProcessReport {
                termination: Termination::Exited(ProcessExit {
                    code: Some(0),
                    signal: None,
                }),
                stdout: CapturedOutput::default(),
                stderr: CapturedOutput::default(),
                stdin_written: 0,
                cleanup: ProcessCleanup {
                    leader_reaped: true,
                    stdout_eof: true,
                    stderr_eof: true,
                    group_kill_attempted: false,
                    signal_failed: false,
                },
                diagnostic: None,
            },
            pending_cleanup: None,
        })
    }
}

#[test]
fn cancellation_during_preparation_prevents_child_start_and_drains_late_value() {
    let control = GraphControl::default();
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, child: &ExecutionControl, limit| {
            assert_eq!(control.cancel(), CancelDecision::Requested);
            let execution = execute_process(
                &Runner(|_: &CancellationProbe<'_>| panic!("cancelled child must not start")),
                &recipe.invocation.process,
                child,
            )
            .unwrap();
            assert_eq!(execution.admission, ResultAdmission::RejectedByCancellation);
            assert!(execution.process.is_none());
            assert_eq!(child.cancel(), CancelDecision::AlreadyFinished);
            transform(recipe, inputs, child, limit)
        },
    );
    let mut run = execute_graph(
        &graph(chain()),
        &Workspace::source(),
        &mut executor,
        &control,
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(names(&executor.calls), ["a"]);
    assert_eq!(names(&run.skipped), ["b", "c"]);
    assert!(run.nodes()[0].1.was_validated);
    assert!(run.results().is_none());
    assert_eq!(executor.owners.get(), 1);
    assert_drained(&mut run);
    drop(run);
    assert_eq!(executor.owners.get(), 0);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

#[test]
fn graph_cancellation_reaches_the_active_child_and_drains_a_late_valid_result() {
    let control = GraphControl::default();
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, child: &ExecutionControl, limit| {
            let process = execute_process(
                &Runner(|probe: &CancellationProbe<'_>| {
                    assert!(!probe.requested());
                    assert_eq!(control.cancel(), CancelDecision::Requested);
                    assert!(probe.requested());
                }),
                &recipe.invocation.process,
                child,
            )
            .unwrap();
            assert_eq!(process.admission, ResultAdmission::RejectedByCancellation);
            transform(recipe, inputs, child, limit)
        },
    );
    let mut run = execute_graph(
        &graph(chain()),
        &Workspace::source(),
        &mut executor,
        &control,
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(names(&executor.calls), ["a"]);
    assert_eq!(names(&run.skipped), ["b", "c"]);
    assert!(run.nodes()[0].1.was_validated);
    assert!(run.results().is_none());
    assert_eq!(executor.owners.get(), 1);
    assert_drained(&mut run);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

#[test]
fn cancellation_during_later_result_capture_discards_current_and_previous_values() {
    let control = GraphControl::default();
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, child: &ExecutionControl, limit| {
            if recipe.invocation.id == id("a") {
                return transform(recipe, inputs, child, limit);
            }
            let workspace = transformed_workspace(recipe, inputs);
            let cancel = control.clone();
            *workspace.on_read.borrow_mut() = Some(Box::new(move || {
                assert_eq!(cancel.cancel(), CancelDecision::Requested);
            }));
            let value = capture_task_result_limited(&workspace, recipe, b"checked payload", limit)?;
            assert_eq!(child.cancel(), CancelDecision::AlreadyRequested);
            Ok(value)
        },
    );
    let mut run = execute_graph(
        &graph(chain()),
        &Workspace::source(),
        &mut executor,
        &control,
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(names(&executor.calls), ["a", "b"]);
    assert_eq!(names(&run.skipped), ["c"]);
    assert!(run.nodes().iter().all(|(_, node)| node.was_validated));
    assert!(run.results().is_none());
    assert_eq!(executor.owners.get(), 2);
    assert_drained(&mut run);
    drop(run);
    assert_eq!(executor.owners.get(), 0);
}

#[test]
fn finished_child_does_not_finish_graph_cancellation_admission() {
    let control = GraphControl::default();
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, child: &ExecutionControl, limit| {
            let execution = execute_process(
                &Runner(|_: &CancellationProbe<'_>| {}),
                &recipe.invocation.process,
                child,
            )
            .unwrap();
            assert_eq!(execution.admission, ResultAdmission::EligibleForValidation);
            assert_eq!(child.cancel(), CancelDecision::AlreadyFinished);
            assert_eq!(control.cancel(), CancelDecision::Requested);
            transform(recipe, inputs, child, limit)
        },
    );
    let mut run = execute_graph(
        &graph(chain()),
        &Workspace::source(),
        &mut executor,
        &control,
    )
    .unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(names(&run.skipped), ["b", "c"]);
    assert!(run.results().is_none());
    assert_drained(&mut run);
}

#[test]
fn cancellation_after_child_deactivation_wins_between_nodes_and_before_final_acceptance() {
    for single_node in [false, true] {
        let control = GraphControl::default();
        let cancel = control.clone();
        let mut executor = Executor::new(transform);
        executor.on_take = Some(Box::new(move || {
            assert_eq!(cancel.cancel(), CancelDecision::Requested);
        }));
        let recipes = if single_node {
            vec![leaf("a")]
        } else {
            chain()
        };
        let mut run = execute_graph(
            &graph(recipes),
            &Workspace::source(),
            &mut executor,
            &control,
        )
        .unwrap();
        assert_eq!(run.status, GraphStatus::Cancelled);
        assert_eq!(names(&executor.calls), ["a"]);
        assert_eq!(
            names(&run.skipped),
            if single_node { vec![] } else { vec!["b", "c"] }
        );
        assert!(run.results().is_none());
        assert_drained(&mut run);
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    }
}

#[test]
fn a_running_or_finished_graph_control_cannot_admit_a_second_graph() {
    let source = Workspace::source();
    let graph = graph(chain());
    let control = GraphControl::default();
    let mut executor = Executor::new(
        |recipe: &Recipe, inputs: &FrozenInputs, child: &ExecutionControl, limit| {
            let mut duplicate = Executor::new(transform);
            let error = execute_graph(&graph, &source, &mut duplicate, &control)
                .err()
                .unwrap();
            assert_eq!(error.kind, ErrorKind::Conflict);
            assert_eq!(code(&error), "DAG001");
            assert!(duplicate.calls.is_empty());
            transform(recipe, inputs, child, limit)
        },
    );
    let run = execute_graph(&graph, &source, &mut executor, &control).unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    let error = execute_graph(&graph, &source, &mut executor, &control)
        .err()
        .unwrap();
    assert_eq!(code(&error), "DAG001");
    assert_eq!(names(&executor.calls), ["a", "b", "c"]);
}

#[test]
fn panic_in_source_or_executor_closes_graph_admission_and_cancels_active_child() {
    for source_panics in [false, true] {
        let source = Workspace::source();
        if source_panics {
            *source.on_read.borrow_mut() = Some(Box::new(|| panic!("source fixture panic")));
        }
        let control = GraphControl::default();
        let child = RefCell::new(None);
        let mut executor = Executor::new(
            |_: &Recipe,
             _: &FrozenInputs,
             active: &ExecutionControl,
             _: usize|
             -> Result<ValidatedTaskResult> {
                *child.borrow_mut() = Some(active.clone());
                panic!("executor fixture panic");
            },
        );
        let graph = graph(chain());
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = execute_graph(&graph, &source, &mut executor, &control);
            }))
            .is_err()
        );
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
        assert_eq!(executor.owners.get(), 0);
        if source_panics {
            assert!(child.borrow().is_none());
        } else {
            assert_eq!(
                child.borrow().as_ref().unwrap().cancel(),
                CancelDecision::AlreadyRequested
            );
        }
        assert_eq!(
            code(
                &execute_graph(&graph, &source, &mut executor, &control)
                    .err()
                    .unwrap()
            ),
            "DAG001"
        );
    }
}
