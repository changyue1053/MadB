//! Explicit operation composition. Protocol and lifecycle decisions remain in
//! contracts/app; this module supplies concrete local ports and reports owners.
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    dag::{GraphStatus, execute_graph},
    input_observation::{InputObservation, InputWatchSet},
    inputs::{FrozenInputs, capture_inputs},
    pipeline::{
        ArtifactRecipe, PipelineControl, PipelineParents, compare_current, run_artifact,
        validate_locations,
    },
    ports::{ConfigurationBytes, FileReference, ReadWorkspace, SnapshotFormat},
    process::ProcessPolicy,
    transaction::Layer,
};
use madb_contracts::{
    execution::{self, BoundArtifactRecipe, ExecutionOperation, ExecutionSpec},
    query::SnapshotAdapter,
    task::{TaskBinding, bind_graph},
};
use madb_host::{
    LocalWorkspace, artifact::LocalArtifactExecutor, dag::LocalGraphExecutor, locations,
    transaction::LocalBatchStore,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};

mod dependencies;
mod retained;
use retained::RetainOnReturn;
pub(super) use retained::RetainedExecution;

pub struct ExecutionOptions {
    pub command: String,
    pub root: PathBuf,
    pub execution: String,
    pub tool_root: Option<PathBuf>,
    pub process_timeout: Duration,
    pub process_termination_grace: Duration,
    pub check: bool,
    pub task: Option<String>,
    pub active_tasks: Vec<String>,
}

pub fn execute(options: &ExecutionOptions) -> Result<String> {
    execute_inner(options, None, None).map(|outcome| outcome.summary)
}

/// In-process initialization composition supplies one stage's existing C04
/// cancellation owner. It does not install a second signal listener or parse
/// human-readable output to recover verified metadata bytes.
pub(super) fn execute_controlled(
    options: &ExecutionOptions,
    control: &PipelineControl,
) -> std::result::Result<ExecutionOutcome, ControlledExecutionError> {
    let journal = std::cell::RefCell::new(ExecutionJournal::default());
    if control.commit().rejected_by_cancellation() {
        return Err(ControlledExecutionError {
            error: QueryError::new(
                ErrorKind::Conflict,
                "STORE002",
                "commit",
                "execution control already rejected a previous publication",
            ),
            journal: journal.into_inner(),
        });
    }
    if options.task.as_deref() == Some("dependencies") {
        return Err(ControlledExecutionError {
            error: invalid(
                "dependencies",
                "initialization must supply its frozen dependency closure",
            ),
            journal: journal.into_inner(),
        });
    }
    match execute_inner(options, Some(control), Some(&journal)) {
        Ok(mut outcome) => {
            outcome.journal = journal.into_inner();
            Ok(outcome)
        }
        Err(mut error) => {
            let mut journal = journal.into_inner();
            if control.commit().rejected_by_cancellation() {
                journal.cancelled = true;
            }
            if let Some(kind) = journal.stage_error_kind {
                error.kind = kind;
            }
            Err(ControlledExecutionError { error, journal })
        }
    }
}
#[derive(Default)]
pub(super) struct ExecutionJournal {
    pub files: Vec<(String, Vec<u8>)>,
    pub cancelled: bool,
    pub retained: Vec<RetainedExecution>,
    pub cleanup_complete: Option<bool>,
    pub committed: Option<Box<(Digest, madb_app::transaction::Batch)>>,
    stage_error_kind: Option<ErrorKind>,
}
pub(super) struct ControlledExecutionError {
    pub error: QueryError,
    pub journal: ExecutionJournal,
}
type Journal<'a> = Option<&'a std::cell::RefCell<ExecutionJournal>>;
fn record_execution(
    journal: Journal<'_>,
    id: &str,
    execution: Option<&madb_app::process::ProcessResult<madb_app::process::ProcessExecution>>,
) {
    let Some(journal) = journal else {
        return;
    };
    let mut journal = journal.borrow_mut();
    let index = journal.files.len();
    let Some(execution) = execution else {
        return;
    };
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            journal.stage_error_kind = Some(ErrorKind::Host);
            journal.files.push((
                format!("process-{index:04}.txt"),
                format!("id={id}\nprocess-error={error}\n").into_bytes(),
            ));
            return;
        }
    };
    let mut summary = format!("id={id}\nadmission={:?}\n", execution.admission);
    if let Some(process) = &execution.process {
        let complete =
            process.pending_cleanup.is_none() && process.report.cleanup.locally_complete();
        journal.cleanup_complete = Some(journal.cleanup_complete.unwrap_or(true) && complete);
        summary.push_str(&format!("termination={:?}\ncleanup={:?}\npending_cleanup={}\nstdout_observed={}\nstdout_truncated={}\nstderr_observed={}\nstderr_truncated={}\ndiagnostic={:?}\n",process.report.termination,process.report.cleanup,process.pending_cleanup.is_some(),process.report.stdout.observed_bytes,process.report.stdout.truncated(),process.report.stderr.observed_bytes,process.report.stderr.truncated(),process.report.diagnostic));
        journal.files.push((
            format!("process-{index:04}.stdout"),
            process.report.stdout.bytes.clone(),
        ));
        journal.files.push((
            format!("process-{index:04}.stderr"),
            process.report.stderr.bytes.clone(),
        ));
    }
    journal
        .files
        .push((format!("process-{index:04}.txt"), summary.into_bytes()));
}
fn record_graph_status(journal: Journal<'_>, status: GraphStatus, failure: Option<&QueryError>) {
    if let Some(journal) = journal {
        let mut journal = journal.borrow_mut();
        // A later cancellation flag does not replace an observed IO/input error.
        journal.cancelled = status == GraphStatus::Cancelled && failure.is_none();
        if failure.is_some() || status == GraphStatus::Failed {
            journal.stage_error_kind = Some(
                failure
                    .map(|e| e.kind)
                    .filter(|kind| matches!(kind, ErrorKind::Host | ErrorKind::Conflict))
                    .unwrap_or(ErrorKind::CheckFailed),
            );
        }
    }
}
pub(super) struct ExecutionOutcome {
    pub summary: String,
    pub metadata: Option<(FileReference, Vec<u8>)>,
    pub journal: ExecutionJournal,
}
impl ExecutionOutcome {
    fn plain(summary: String) -> Self {
        Self {
            summary,
            metadata: None,
            journal: ExecutionJournal::default(),
        }
    }
}
#[derive(Clone, Copy)]
struct ExecutionContext<'a> {
    control: Option<&'a PipelineControl>,
    journal: Journal<'a>,
}
fn execute_inner(
    options: &ExecutionOptions,
    control: Option<&PipelineControl>,
    journal: Journal<'_>,
) -> Result<ExecutionOutcome> {
    if options.command == "build"
        && options.task.as_deref() == Some("dependencies")
        && options.process_timeout != ProcessPolicy::default().timeout
    {
        return Err(invalid(
            "process-timeout-seconds",
            "build --task dependencies uses its dedicated discovery interface, which does not accept a non-default process budget",
        ));
    }
    if options.command == "build"
        && options.task.as_deref() == Some("dependencies")
        && options.process_termination_grace != ProcessPolicy::default().termination_grace
    {
        return Err(invalid(
            "process-termination-grace-ms",
            "build --task dependencies uses its dedicated discovery interface, which does not accept a non-default process termination grace",
        ));
    }
    let workspace = LocalWorkspace::open(&options.root)?;
    let execution_path = logical(&options.execution)?;
    let bytes = workspace
        .read(&execution_path, 1024 * 1024)?
        .ok_or_else(|| invalid("execution", "execution document is missing"))?;
    if let Some(journal) = journal {
        journal
            .borrow_mut()
            .files
            .push(("execution.json".into(), bytes.clone()));
    }
    let spec = execution::read_execution(&bytes)?;
    validate_command(options, &spec)?;
    let documents = load_documents(
        &workspace,
        &spec,
        FileReference {
            path: execution_path,
            digest: Digest::of_bytes(&bytes),
            size: bytes.len() as u64,
        },
        options.task.as_deref() != Some("dependencies"),
    )?;
    spec.context.validate(
        configuration(&spec, &documents),
        spec.context
            .contract
            .as_ref()
            .map(|reference| document(&documents, reference)),
    )?;
    let mut protected = documents
        .references()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    if let Some(recipe) = &spec.recipe {
        protected.extend(recipe.inputs.iter().map(|file| file.path.clone()));
        protected.push(recipe.installed_tool.path.clone());
    }
    protected.extend(
        spec.tasks
            .iter()
            .map(|task| task.installed_tool.path.clone()),
    );
    let task_documents = spec
        .tasks
        .iter()
        .map(|task| document(&documents, &task.request).to_vec())
        .collect::<Vec<_>>();
    protected.extend(
        execution::build_input_references(&task_documents)?
            .into_iter()
            .map(|file| file.path),
    );
    validate_locations(
        &spec.store,
        &spec.run_parent,
        spec.cache.as_ref(),
        &protected,
    )?;
    dependencies::validate_domains(&spec, &documents)?;
    if options.command == "build" && options.task.as_deref() == Some("dependencies") {
        if spec.operation == ExecutionOperation::Build {
            // Discovery precedes dependency/tool binding. Inspect phase policy
            // without requiring the dependency receipt it is about to create.
            execution::validate_build_phase_admission(
                configuration(&spec, &documents).profile,
                &task_documents,
            )?;
        }
        return dependencies::prepare(options, &spec, &documents, &workspace)
            .map(ExecutionOutcome::plain);
    }
    if spec.operation == ExecutionOperation::Metadata {
        return metadata(options, &spec, &documents, &workspace, control, journal);
    }
    let store_path = locations::directory(&options.root, &spec.store, !options.check)?
        .ok_or_else(|| drift("no committed source batch; run generate first"))?;
    let store = if options.check {
        LocalBatchStore::open_readonly(&store_path)?
            .ok_or_else(|| drift("no committed source batch"))?
    } else {
        LocalBatchStore::open(&store_path)?
    };
    let layer = match spec.operation {
        ExecutionOperation::Generate => Layer::Source,
        ExecutionOperation::Build => Layer::Build,
        ExecutionOperation::Package => Layer::Candidate,
        ExecutionOperation::Metadata => unreachable!("metadata handled above"),
    };
    let parents = PipelineParents::capture(&store, layer)?;
    if spec.operation == ExecutionOperation::Build {
        build(
            options,
            &spec,
            &documents,
            &workspace,
            &store,
            &parents,
            ExecutionContext { control, journal },
        )
        .map(ExecutionOutcome::plain)
    } else {
        artifact(
            options,
            &spec,
            &documents,
            &workspace,
            &store,
            &parents,
            ExecutionContext { control, journal },
        )
        .map(ExecutionOutcome::plain)
    }
}

fn validate_command(options: &ExecutionOptions, spec: &ExecutionSpec) -> Result<()> {
    let matches = match options.command.as_str() {
        "generate" => spec.operation == ExecutionOperation::Generate,
        "package" => spec.operation == ExecutionOperation::Package,
        "build" if options.task.as_deref() == Some("dependencies") => spec.dependencies.is_some(),
        "build" if options.task.as_deref() == Some("metadata") => {
            spec.operation == ExecutionOperation::Metadata
        }
        "build" => spec.operation == ExecutionOperation::Build,
        _ => false,
    };
    if !matches {
        return Err(invalid(
            "operation",
            "command and execution operation differ; metadata uses build --task metadata",
        ));
    }
    if spec.operation == ExecutionOperation::Metadata && !options.active_tasks.is_empty() {
        return Err(invalid(
            "active-task",
            "metadata preparation cannot recursively enter an active build",
        ));
    }
    Ok(())
}

fn load_documents(
    workspace: &LocalWorkspace,
    spec: &ExecutionSpec,
    own: FileReference,
    load_receipt: bool,
) -> Result<FrozenInputs> {
    let mut refs = vec![
        own,
        spec.context.project.clone(),
        spec.context.read_request.clone(),
        spec.context.lock.clone(),
        spec.context.profile.clone(),
    ];
    refs.extend(spec.context.contract.iter().cloned());
    if let Some(recipe) = &spec.recipe {
        refs.push(recipe.request.clone());
    }
    refs.extend(spec.tasks.iter().map(|task| task.request.clone()));
    if let Some(dependencies) = &spec.dependencies {
        refs.push(dependencies.request.clone());
        if load_receipt {
            if let Some(receipt) = &dependencies.receipt {
                let actual = workspace.read(&receipt.path, 1024 * 1024)?;
                if actual.as_ref().is_none_or(|bytes| {
                    bytes.len() as u64 != receipt.size || Digest::of_bytes(bytes) != receipt.digest
                }) {
                    return Err(QueryError::new(
                        ErrorKind::InvalidInput,
                        "DEP005",
                        receipt.path.as_str(),
                        "MetadataRequired: dependency receipt is missing or changed; run build --task dependencies and rebind",
                    ));
                }
            }
            refs.extend(dependencies.receipt.iter().cloned());
        }
    }
    let first = capture_inputs(workspace, unique_references(refs)?)?;
    let adapter = SnapshotAdapter::new()?;
    let (_, tracked) = adapter.prepare(configuration(spec, &first))?;
    let refs = first.references().cloned().chain(tracked).collect();
    capture_inputs(workspace, unique_references(refs)?)
}
fn configuration<'a>(spec: &ExecutionSpec, documents: &'a FrozenInputs) -> ConfigurationBytes<'a> {
    ConfigurationBytes {
        project: document(documents, &spec.context.project),
        request: document(documents, &spec.context.read_request),
        lock: document(documents, &spec.context.lock),
        profile: document(documents, &spec.context.profile),
    }
}
fn document<'a>(documents: &'a FrozenInputs, reference: &FileReference) -> &'a [u8] {
    documents
        .files()
        .find(|(file, _)| file.path == reference.path)
        .expect("loaded document reference")
        .1
}
fn unique_references(references: Vec<FileReference>) -> Result<Vec<FileReference>> {
    let mut files = BTreeMap::new();
    for reference in references {
        if let Some(previous) = files.insert(reference.path.clone(), reference.clone())
            && previous != reference
        {
            return Err(invalid(
                reference.path.as_str(),
                "one logical input has conflicting identities",
            ));
        }
    }
    Ok(files.into_values().collect())
}
fn run_parent(options: &ExecutionOptions, spec: &ExecutionSpec) -> Result<PathBuf> {
    if options.check {
        locations::temporary_parent()
    } else {
        locations::directory(&options.root, &spec.run_parent, true)?
            .ok_or_else(|| invalid("run_parent", "run parent is missing"))
    }
}
fn watch(spec: &ExecutionSpec, workspace: &LocalWorkspace) -> Result<InputObservation> {
    InputObservation::capture(
        workspace,
        &InputWatchSet::new(spec.watch.absent.clone(), spec.watch.directories.clone())?,
    )
}
fn policy(options: &ExecutionOptions, environment: impl Iterator<Item = String>) -> ProcessPolicy {
    ProcessPolicy {
        timeout: options.process_timeout,
        termination_grace: options.process_termination_grace,
        allowed_environment: environment.collect(),
        ..ProcessPolicy::default()
    }
}

fn metadata(
    options: &ExecutionOptions,
    spec: &ExecutionSpec,
    documents: &FrozenInputs,
    workspace: &LocalWorkspace,
    supplied_control: Option<&PipelineControl>,
    journal: Journal<'_>,
) -> Result<ExecutionOutcome> {
    let recipe_spec = spec.recipe.as_ref().expect("validated recipe");
    let dependency = dependencies::load(options, spec, documents, workspace)?;
    if let Some(dependency) = &dependency {
        dependency.validate_coverage(&recipe_spec.inputs)?;
    }
    let inputs = capture_inputs(workspace, recipe_spec.inputs.clone())?;
    let recipe = BoundArtifactRecipe::new(
        configuration(spec, documents),
        None,
        recipe_spec,
        document(documents, &recipe_spec.request),
        &inputs,
    )?;
    if recipe.operation() != spec.operation {
        return Err(invalid(
            "operation",
            "recipe payload differs from execution operation",
        ));
    }
    let observation = watch(spec, workspace)?;
    let local_control = PipelineControl::default();
    let control = supplied_control.unwrap_or(&local_control);
    let _scope = control.scope();
    let _signals = if supplied_control.is_none() {
        Some(madb_host::cancellation::CancellationSignals::install(
            control,
        )?)
    } else {
        None
    };
    let mut executor = LocalArtifactExecutor::new(
        &run_parent(options, spec)?,
        &locations::unique_run_name(),
        tool_root(options),
        policy(
            options,
            recipe.invocation().process.environment.keys().cloned(),
        ),
    )
    .map_err(|error| error.error)?;
    let run = RetainOnReturn::new(
        run_artifact(&recipe, workspace, &mut executor, control.graph())?,
        journal,
        |run| RetainedExecution::Artifact(Box::new(run)),
    );
    record_graph_status(journal, run.status, run.failure.as_ref());
    if let Some(node) = run.node() {
        record_execution(journal, "artifact", node.execution());
    }
    if let Some(journal) = journal {
        journal.borrow_mut().files.push((
            "invocation.txt".into(),
            format!(
                "tool={:?}\narguments={:?}\nenvironment={:?}\n",
                recipe.invocation().process.tool,
                recipe.invocation().process.arguments,
                recipe.invocation().process.environment
            )
            .into_bytes(),
        ));
    }
    let result = run.result().ok_or_else(|| artifact_failure(&run))?;
    documents.verify(workspace)?;
    dependencies::verify(options, &dependency, workspace)?;
    observation.verify(workspace)?;
    control.finish_without_commit()?;
    let path = run
        .node()
        .and_then(|node| node.path())
        .expect("successful owner")
        .join("work")
        .join(result.manifest().0.path.as_str());
    Ok(ExecutionOutcome {
        journal: ExecutionJournal::default(),
        metadata: Some((result.manifest().0.clone(), result.manifest().1.to_vec())),
        summary: format!(
            "MetadataPrepared\nManifest: {}\nDigest: {}\nNo source/build/candidate index changed.\n",
            path.display(),
            result.manifest().0.digest.as_str()
        ),
    })
}

fn artifact(
    options: &ExecutionOptions,
    spec: &ExecutionSpec,
    documents: &FrozenInputs,
    workspace: &LocalWorkspace,
    store: &LocalBatchStore,
    parents: &PipelineParents<madb_host::transaction::BatchLease>,
    context: ExecutionContext<'_>,
) -> Result<String> {
    let ExecutionContext {
        control: supplied_control,
        journal,
    } = context;
    let source = parents.workspace(workspace)?;
    let recipe_spec = spec.recipe.as_ref().expect("validated recipe");
    if spec.operation == ExecutionOperation::Package {
        execution::validate_package_parents(
            document(documents, &recipe_spec.request),
            parents.source().expect("required source").files(),
            parents.build().expect("required build").files(),
        )?;
    }
    let dependency = dependencies::load(options, spec, documents, workspace)?;
    if let Some(dependency) = &dependency {
        dependency.validate_coverage(&recipe_spec.inputs)?;
    }
    let inputs = capture_inputs(&source, recipe_spec.inputs.clone())?;
    let snapshot = spec
        .context
        .contract
        .as_ref()
        .map(|reference| document(documents, reference));
    let recipe = BoundArtifactRecipe::new(
        configuration(spec, documents),
        snapshot,
        recipe_spec,
        document(documents, &recipe_spec.request),
        &inputs,
    )?;
    if recipe.operation() != spec.operation {
        return Err(invalid(
            "operation",
            "recipe payload differs from execution operation",
        ));
    }
    let observation = watch(spec, workspace)?;
    let local_control = PipelineControl::default();
    let control = supplied_control.unwrap_or(&local_control);
    let _scope = control.scope();
    let _signals = if supplied_control.is_none() {
        Some(madb_host::cancellation::CancellationSignals::install(
            control,
        )?)
    } else {
        None
    };
    let mut executor = LocalArtifactExecutor::new(
        &run_parent(options, spec)?,
        &locations::unique_run_name(),
        tool_root(options),
        policy(
            options,
            recipe.invocation().process.environment.keys().cloned(),
        ),
    )
    .map_err(|error| error.error)?;
    let run = RetainOnReturn::new(
        run_artifact(&recipe, &source, &mut executor, control.graph())?,
        journal,
        |run| RetainedExecution::Artifact(Box::new(run)),
    );
    record_graph_status(journal, run.status, run.failure.as_ref());
    if let Some(node) = run.node() {
        record_execution(journal, "artifact", node.execution());
    }
    if let Some(journal) = journal {
        journal.borrow_mut().files.push((
            "invocation.txt".into(),
            format!(
                "tool={:?}\narguments={:?}\nenvironment={:?}\n",
                recipe.invocation().process.tool,
                recipe.invocation().process.arguments,
                recipe.invocation().process.environment
            )
            .into_bytes(),
        ));
    }
    let result = run.result().ok_or_else(|| artifact_failure(&run))?;
    let batch = parents.batch(result)?;
    let retained = run
        .node()
        .and_then(|node| node.path())
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let revalidate = || {
        documents.verify(workspace)?;
        dependencies::verify(options, &dependency, workspace)?;
        inputs.verify(&source)?;
        observation.verify(workspace)
    };
    if options.check {
        revalidate()?;
        let difference = compare_current(store, parents.expected(), &batch)
            .map_err(|error| retained_error(error, &retained))?;
        if !difference.is_empty() {
            return Err(drift(&format!(
                "missing={:?}; added={:?}; changed={:?}; identity_changed={}; retained={retained}",
                difference.missing,
                difference.added,
                difference.changed,
                difference.identity_changed
            )));
        }
        control.finish_without_commit()?;
        Ok(format!(
            "GenerationMatches: isolated recomputation matches the committed source batch.\nRetainedRun: {retained}\n"
        ))
    } else {
        let receipt = parents
            .publish(store, &batch, control, revalidate)
            .map_err(|error| retained_error(error, &retained))?;
        if let Some(journal) = journal {
            journal.borrow_mut().committed =
                Some(Box::new((receipt.batch_id.clone(), batch.clone())));
        }
        Ok(format!(
            "{:?}\nLayer: {}\nBatch: {}\nGeneration: {}\nRevision: {}\n{}",
            receipt.disposition,
            batch.spec().layer.as_str(),
            receipt.batch_id.as_str(),
            batch.spec().generation_id.as_str(),
            receipt.index.revision,
            cache_notice(spec)
        ))
    }
}

fn build(
    options: &ExecutionOptions,
    spec: &ExecutionSpec,
    documents: &FrozenInputs,
    workspace: &LocalWorkspace,
    store: &LocalBatchStore,
    parents: &PipelineParents<madb_host::transaction::BatchLease>,
    context: ExecutionContext<'_>,
) -> Result<String> {
    let ExecutionContext {
        control: supplied_control,
        journal,
    } = context;
    let source = parents.workspace(workspace)?;
    let snapshot = document(
        documents,
        spec.context.contract.as_ref().expect("validated context"),
    );
    let task_bytes = spec
        .tasks
        .iter()
        .map(|task| document(documents, &task.request).to_vec())
        .collect::<Vec<_>>();
    let target = options.task.as_deref().map(identifier).transpose()?;
    let active = options
        .active_tasks
        .iter()
        .map(|id| identifier(id))
        .collect::<Result<Vec<_>>>()?;
    let graph = bind_graph(
        configuration(spec, documents),
        snapshot,
        spec.tasks
            .iter()
            .zip(&task_bytes)
            .map(|(task, bytes)| TaskBinding {
                task: bytes,
                installed_tool: task.installed_tool.clone(),
            })
            .collect(),
        target.as_ref(),
        &active,
    )?;
    let selected = graph
        .tasks()
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let selected_bytes = execution::selected_task_documents(&task_bytes, &selected)?;
    let dependency = dependencies::load(options, spec, documents, workspace)?;
    if let Some(dependency) = &dependency {
        dependency.validate_coverage(&execution::build_input_references(&selected_bytes)?)?;
    }
    let source_lease = parents.source().expect("required source parent");
    execution::validate_build_generation(
        &selected_bytes,
        execution::source_manifest(source_lease.files())?,
        source_lease.files(),
    )?;
    let observation = watch(spec, workspace)?;
    let local_control = PipelineControl::default();
    let control = supplied_control.unwrap_or(&local_control);
    let _scope = control.scope();
    let _signals = if supplied_control.is_none() {
        Some(madb_host::cancellation::CancellationSignals::install(
            control,
        )?)
    } else {
        None
    };
    let environment = execution::build_environment_names(&selected_bytes)?;
    let mut executor = LocalGraphExecutor::new(
        &run_parent(options, spec)?,
        &locations::unique_run_name(),
        tool_root(options),
        policy(options, environment.into_iter()),
    )
    .map_err(|error| error.error)?;
    let run = RetainOnReturn::new(
        execute_graph(&graph, &source, &mut executor, control.graph())?,
        journal,
        RetainedExecution::Graph,
    );
    record_graph_status(journal, run.status, run.failure.as_ref());
    for (id, node) in run.nodes() {
        record_execution(journal, id.as_str(), node.execution());
    }
    if let Some(journal) = journal {
        for (index, bytes) in selected_bytes.iter().enumerate() {
            journal
                .borrow_mut()
                .files
                .push((format!("task-{index:04}.json"), bytes.clone()));
        }
    }
    if run.status != GraphStatus::Validated {
        let mut failure = invalid("build", format!("graph failed: {:?}", run.status));
        if let Some(cause) = &run.failure {
            failure.findings.extend(cause.findings.clone());
        }
        for (id, node) in run.nodes() {
            append_node_diagnostics(
                &mut failure,
                id.as_str(),
                node.preparation_error(),
                node.execution(),
            );
            if let Some(path) = node.path() {
                failure.findings.push(madb_app::Finding::new(
                    "RETAINED",
                    id.as_str(),
                    path.display().to_string(),
                ));
            }
        }
        return Err(failure);
    }
    let completion = execution::build_completion_policy(configuration(spec, documents).profile)?;
    let complete = completion.is_complete(graph.tasks().iter().map(|task| task.phase));
    if !complete {
        documents.verify(workspace)?;
        dependencies::verify(options, &dependency, workspace)?;
        observation.verify(workspace)?;
        run.inputs()
            .expect("validated root inputs")
            .verify(&source)?;
        control.finish_without_commit()?;
        return Ok("Executed: selected tasks passed; the selected profile's complete build evidence is required before a build batch can be committed.\n".to_owned());
    }
    let artifacts = execution::assemble_build_files(
        &selected_bytes,
        run.results().expect("validated results"),
        configuration(spec, documents).profile,
    )?;
    let batch = parents.build_batch(artifacts.identity, artifacts.files)?;
    let receipt = parents.publish(store, &batch, control, || {
        documents.verify(workspace)?;
        dependencies::verify(options, &dependency, workspace)?;
        run.inputs()
            .expect("validated root inputs")
            .verify(&source)?;
        observation.verify(workspace)
    })?;
    if let Some(journal) = journal {
        journal.borrow_mut().committed = Some(Box::new((receipt.batch_id.clone(), batch.clone())));
    }
    Ok(format!(
        "{:?}\nLayer: build\nBatch: {}\nGeneration: {}\nRevision: {}\n{}",
        receipt.disposition,
        receipt.batch_id.as_str(),
        batch.spec().generation_id.as_str(),
        receipt.index.revision,
        cache_notice(spec)
    ))
}
fn cache_notice(spec: &ExecutionSpec) -> &'static str {
    if spec.cache.is_some() {
        "CacheDisabled: arbitrary helper dependency completeness is not proven; executed normally.\n"
    } else {
        ""
    }
}
fn artifact_failure(
    run: &madb_app::pipeline::ArtifactRun<madb_host::artifact::LocalArtifactRun>,
) -> QueryError {
    let mut error = invalid("artifact", format!("artifact failed: {:?}", run.status));
    if let Some(cause) = &run.failure {
        error.findings.extend(cause.findings.clone());
    }
    if let Some(node) = run.node() {
        if let Some(path) = node.path() {
            error.findings.push(madb_app::Finding::new(
                "RETAINED",
                "run",
                path.display().to_string(),
            ));
        }
        append_node_diagnostics(
            &mut error,
            "artifact",
            node.preparation_error(),
            node.execution(),
        );
    }
    error
}
fn append_node_diagnostics(
    error: &mut QueryError,
    subject: &str,
    preparation: Option<&madb_host::inputs::PreparationError>,
    execution: Option<&madb_app::process::ProcessResult<madb_app::process::ProcessExecution>>,
) {
    if let Some(cause) = preparation {
        error.findings.extend(cause.error.findings.clone());
    }
    let diagnostic = match execution {
        Some(Err(cause)) => Some(cause.to_string()),
        Some(Ok(value)) => value.process.as_ref().map(|process| {
            let report = &process.report;
            let observation = format!(
                "termination={:?}; locally_complete={}; pending_cleanup={}",
                report.termination,
                report.cleanup.locally_complete(),
                process.pending_cleanup.is_some()
            );
            match &report.diagnostic {
                Some(cause) => format!("{cause}; {observation}"),
                None => observation,
            }
        }),
        None => None,
    };
    if let Some(message) = diagnostic {
        error
            .findings
            .push(madb_app::Finding::new("PROCESS", subject, message));
    }
}

fn logical(value: &str) -> Result<LogicalPath> {
    LogicalPath::new(value).map_err(|error| invalid("path", error.to_string()))
}
fn identifier(value: &str) -> Result<Id> {
    Id::new(value).map_err(|error| invalid("task", error.to_string()))
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "CLI001", subject, message)
}
fn drift(message: &str) -> QueryError {
    QueryError::new(ErrorKind::CheckFailed, "GEN001", "generation", message)
}

fn retained_error(mut error: QueryError, path: &str) -> QueryError {
    error
        .findings
        .push(madb_app::Finding::new("RETAINED", "run", path));
    error
}

fn tool_root(options: &ExecutionOptions) -> &std::path::Path {
    options.tool_root.as_deref().unwrap_or(&options.root)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use madb_app::process::{
        ExecutionControl, ProcessRequest, StopReason, Termination, execute_process,
    };
    use madb_host::process::LocalProcessRunner;
    use std::time::Instant;

    struct TemporaryDirectory(PathBuf);
    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn explicit_budget_controls_the_real_child_deadline() {
        let directory =
            TemporaryDirectory(std::env::temp_dir().join(locations::unique_run_name().as_str()));
        std::fs::create_dir_all(directory.0.join("work")).unwrap();
        let tool = std::fs::canonicalize("/bin/sleep").unwrap();
        let image = std::fs::read(&tool).unwrap();
        let request = ProcessRequest {
            tool: FileReference {
                path: logical(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
                digest: Digest::of_bytes(&image),
                size: image.len() as u64,
            },
            working_directory: logical("work").unwrap(),
            arguments: vec!["2".into()],
            environment: BTreeMap::new(),
            stdin: Vec::new(),
        };
        for seconds in [1, 3] {
            let options = ExecutionOptions {
                command: "build".into(),
                root: directory.0.clone(),
                execution: "unused.json".into(),
                tool_root: None,
                process_timeout: Duration::from_secs(seconds),
                process_termination_grace: Duration::from_millis(250),
                check: false,
                task: Some("metadata".into()),
                active_tasks: Vec::new(),
            };
            let runner = LocalProcessRunner::new(
                tool.parent().unwrap(),
                &directory.0,
                policy(&options, std::iter::empty()),
            )
            .unwrap();
            let start = Instant::now();
            let execution =
                execute_process(&runner, &request, &ExecutionControl::default()).unwrap();
            let run = execution.process.expect("real child was started");
            if seconds == 1 {
                assert!(matches!(
                    run.report.termination,
                    Termination::Stopped {
                        reason: StopReason::Deadline,
                        ..
                    }
                ));
                assert!(
                    start.elapsed() < Duration::from_secs(8),
                    "deadline was not enforced promptly"
                );
            } else {
                assert!(
                    matches!(run.report.termination, Termination::Exited(exit) if exit.code == Some(0))
                );
            }
            assert!(run.report.cleanup.locally_complete());
            assert!(run.pending_cleanup.is_none());
        }
    }
}

#[cfg(all(test, unix))]
#[path = "execution_grace_tests.rs"]
mod grace_tests;

#[cfg(test)]
mod controlled_reuse_tests {
    use super::*;
    #[test]
    fn previous_veto_is_not_reported_as_this_attempt_cancellation() {
        let control = PipelineControl::default();
        control.cancel();
        assert!(control.finish_without_commit().is_err());
        assert!(control.commit().rejected_by_cancellation());
        let options = ExecutionOptions {
            command: "generate".into(),
            root: PathBuf::from("/not-an-execution-root"),
            execution: "missing.json".into(),
            tool_root: None,
            process_timeout: Duration::from_secs(1),
            process_termination_grace: Duration::from_millis(1),
            check: false,
            task: None,
            active_tasks: Vec::new(),
        };
        let Err(error) = execute_controlled(&options, &control) else {
            panic!("consumed control")
        };
        assert_eq!(error.error.kind, ErrorKind::Conflict);
        assert_eq!(error.error.findings[0].code, "STORE002");
        assert!(!error.journal.cancelled);
        assert!(control.commit().rejected_by_cancellation());
    }
}
