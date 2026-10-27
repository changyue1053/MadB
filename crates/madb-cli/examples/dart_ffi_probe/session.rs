use super::{Result, path};
use madb_app::{
    inputs::FrozenInputs,
    pipeline::{
        ArtifactExecutor, ArtifactNodeRun, ArtifactRecipe, ArtifactResult, ArtifactRun,
        ArtifactSequence, PipelineControl, capture_artifact_result,
    },
    process::{ExecutionControl, ProcessError, ProcessPolicy, execute_process_validated},
};
use madb_contracts::{
    extension_files::{BoundExtensionFiles, FrozenExtensionResult},
    extension_session::{HostSession, SessionOptions},
};
use madb_host::{
    extension_files::LocalExtensionWorkspace,
    process::{LocalProcessRunner, session::InteractiveProcessRunner},
};
use serde_json::{Value, json};
use std::{cell::RefCell, fs, path::Path, time::Duration};
pub struct Stage {
    pub report: Value,
    pub value: Option<ArtifactResult>,
    pub workspace: LocalExtensionWorkspace,
    pub _execution: madb_app::process::ProcessExecution,
}
pub struct Request<'a> {
    pub parent: &'a Path,
    pub tools: &'a Path,
    pub name: &'a str,
    pub files: &'a BoundExtensionFiles,
    pub pipeline: &'a PipelineControl,
    pub fault: &'a str,
}
pub struct Attempt {
    stage: Result<Stage>,
}
impl Attempt {
    pub fn stage(&self) -> Result<&Stage> {
        self.stage.as_ref().map_err(|e| e.to_string().into())
    }
}
impl ArtifactNodeRun for Attempt {
    fn take_value(&mut self) -> Option<ArtifactResult> {
        self.stage.as_mut().ok().and_then(|s| s.value.take())
    }
}
struct Executor<'a, 'b>(&'a Request<'b>);
impl<R: ArtifactRecipe> ArtifactExecutor<R> for Executor<'_, '_> {
    type Run = Attempt;
    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        limit: usize,
    ) -> Attempt {
        let checked = || -> Result<Stage> {
            for (r, b) in inputs.files() {
                if !self
                    .0
                    .files
                    .inputs()
                    .files()
                    .any(|(f, bytes)| f == r && bytes == b)
                {
                    return Err("sequence input differs from checked session snapshot".into());
                }
            }
            run_process(self.0, recipe, control, limit)
        };
        Attempt { stage: checked() }
    }
}
pub fn run<R: ArtifactRecipe>(
    sequence: &mut ArtifactSequence<'_>,
    request: &Request<'_>,
    recipe: &R,
    source: &madb_host::LocalWorkspace,
) -> Result<ArtifactRun<Attempt>> {
    Ok(sequence.run(recipe, source, &mut Executor(request))?)
}
pub fn report(run: &ArtifactRun<Attempt>, request: &Request<'_>) -> Value {
    let mut value = match run.node() {
        Some(attempt) => match &attempt.stage {
            Ok(stage) => stage.report.clone(),
            Err(error) => {
                json!({"component":request.files.composition_pins().expect("pins").component_id(),"session_id":format!("chain-{}",request.name),"request_id":format!("request-{}",request.name),"output_root":request.files.output_root(),"admission":"Unavailable","process":null,"error":error.to_string()})
            }
        },
        None => {
            json!({"component":request.files.composition_pins().expect("pins").component_id(),"session_id":format!("chain-{}",request.name),"request_id":format!("request-{}",request.name),"output_root":request.files.output_root(),"admission":if run.status==madb_app::dag::GraphStatus::Cancelled{"RejectedByCancellation"}else{"Unavailable"},"process":null,"skipped_before_execution":true})
        }
    };
    value["graph_status"] = json!(format!("{:?}", run.status));
    value
}
fn run_process<R: ArtifactRecipe>(
    request: &Request<'_>,
    recipe: &R,
    control: &ExecutionControl,
    result_limit: usize,
) -> Result<Stage> {
    let Request {
        parent,
        tools,
        name,
        files,
        pipeline,
        fault,
    } = request;
    let workspace = LocalExtensionWorkspace::create(
        parent,
        &path(name),
        files.inputs().clone(),
        &path(files.output_root()),
    )
    .map_err(|e| e.error)?;
    if *fault == "cancel-before" {
        pipeline.cancel();
    }
    let captured: RefCell<Option<FrozenExtensionResult>> = RefCell::new(None);
    let pins = files.composition_pins().ok_or("missing composition pins")?;
    let features = pins
        .required_features()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let session = HostSession::from_composition(
        files,
        SessionOptions {
            component_id: pins.component_id(),
            session_id: &format!("chain-{name}"),
            request_id: &format!("request-{name}"),
            protocol: "1.0",
            ir: "1.0",
            required_features: &features,
            handshake_timeout: Duration::from_secs(10),
            task_timeout: Duration::from_secs(150),
        },
        |output| {
            let value = files
                .capture_result(workspace.output_workspace(), output)
                .map_err(|e| ProcessError::new("OutputInvalid", "capture", e.to_string()))?;
            if *fault == "output-drift" {
                let (r, b) =
                    value.files().files().next().ok_or_else(|| {
                        ProcessError::new("OutputInvalid", "fault", "missing result")
                    })?;
                let mut changed = b.to_vec();
                changed.push(b' ');
                fs::write(workspace.output_path().join(r.path.as_str()), changed)
                    .map_err(|e| ProcessError::new("OutputInvalid", "fault", e.to_string()))?;
            }
            captured.replace(Some(value));
            if *fault == "cancel-result" {
                pipeline.cancel();
            }
            Ok(())
        },
    )?;
    let policy = ProcessPolicy {
        timeout: Duration::from_secs(180),
        termination_grace: Duration::from_secs(3),
        cleanup_timeout: Duration::from_secs(5),
        stderr_limit: 1024 * 1024,
        ..ProcessPolicy::default()
    };
    let host = LocalProcessRunner::new(tools, workspace.path(), policy)?;
    let runner = InteractiveProcessRunner::new(&host, session);
    let result = execute_process_validated(&runner, pins.process(), control, |_, _| {
        runner.driver().result_reference()?;
        let observation = captured
            .borrow_mut()
            .take()
            .ok_or_else(|| ProcessError::new("OutputInvalid", "capture", "missing terminal"))?;
        workspace
            .verify_inputs()
            .and_then(|_| workspace.verify_outputs(observation.files()))
            .map_err(|e| ProcessError::new("OutputInvalid", "snapshot", e.to_string()))?;
        observation
            .ensure_success()
            .map_err(|e| ProcessError::new("OutputInvalid", "result", e.to_string()))?;
        let bytes = observation
            .files()
            .files()
            .find(|(r, _)| r.path == recipe.invocation().response_path)
            .ok_or_else(|| ProcessError::new("OutputInvalid", "result", "missing response"))?
            .1;
        capture_artifact_result(workspace.output_workspace(), recipe, bytes, result_limit)
            .map_err(|e| ProcessError::new("OutputInvalid", "artifact", e.to_string()))
    })?;
    let report = json!({"component":pins.component_id(),"session_id":format!("chain-{name}"),"request_id":format!("request-{name}"),"output_root":files.output_root(),"admission":format!("{:?}",result.execution.admission),"peer_diagnostic":runner.driver().peer_diagnostic(),"process":result.execution.process.as_ref().map(|p|json!({"termination":format!("{:?}",p.report.termination),"cleanup":{"locally_complete":p.report.cleanup.locally_complete(),"leader_reaped":p.report.cleanup.leader_reaped,"stdout_eof":p.report.cleanup.stdout_eof,"stderr_eof":p.report.cleanup.stderr_eof,"group_kill_attempted":p.report.cleanup.group_kill_attempted,"signal_failed":p.report.cleanup.signal_failed},"pending_cleanup":p.pending_cleanup.is_some(),"diagnostic":p.report.diagnostic.as_ref().map(|e|json!({"code":e.code,"stage":e.stage,"message":e.message})),"stdout":String::from_utf8_lossy(&p.report.stdout.bytes),"stdout_observed":p.report.stdout.observed_bytes,"stdout_truncated":p.report.stdout.truncated(),"stderr":String::from_utf8_lossy(&p.report.stderr.bytes),"stderr_observed":p.report.stderr.observed_bytes,"stderr_truncated":p.report.stderr.truncated()}))});
    drop(runner);
    Ok(Stage {
        report,
        value: result.value,
        workspace,
        _execution: result.execution,
    })
}
