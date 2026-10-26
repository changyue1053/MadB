//! Test composition root for the E02 protocol. Not a new CLI product command.
use madb_app::{
    Digest, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::FileReference,
    process::{
        CancelDecision, ExecutionControl, ProcessError, ProcessPolicy, ProcessRequest,
        execute_process_validated,
    },
};
use madb_contracts::{
    DocumentKind, SchemaRegistry,
    extension::{ExtensionManifest, validate_registration},
    extension_files::{BoundExtensionFiles, FrozenExtensionResult},
    extension_session::{HostSession, SessionOptions},
    task_digest,
};
use madb_host::{
    LocalWorkspace,
    extension_files::LocalExtensionWorkspace,
    process::{LocalProcessRunner, session::InteractiveProcessRunner},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub fn run(
    peer: &Path,
    mode: &str,
    root: &Path,
    component: &str,
    upstream: Option<&FrozenInputs>,
) -> Result<(Value, Option<FrozenInputs>), Box<dyn std::error::Error>> {
    fs::create_dir(root)?;
    fs::create_dir(root.join("tools"))?;
    fs::create_dir(root.join("source"))?;
    fs::copy(peer, root.join("tools/peer"))?;
    let manifest = json!({"schema":"madb.extension/1","id":"test.session","version":"0.1.0","compatibility":{"protocol":["1.0"],"ir":["1.0"]},"languages":[],"components":[{"id":component,"port":"toolchain","supported_features":["core.values@1"],"entrypoint":{"kind":"process","path":"peer","args":[mode]}}],"dependencies":[],"routes":[],"assets":[],"execution":{"transport":"stdio-jsonl","network":"none","env":[]}});
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    fs::write(root.join("tools/manifest.json"), &manifest_bytes)?;
    let manifest = ExtensionManifest::decode(&manifest_bytes)?;
    validate_registration(std::slice::from_ref(&manifest))?;
    let data = b"fixture";
    let mut input_refs = Vec::new();
    if let Some(upstream) = upstream {
        // Transfer only the independently admitted immutable bytes; never re-read
        // a helper's former writable output tree for a downstream request.
        for (reference, bytes) in upstream.files() {
            let path = root.join("source").join(reference.path.as_str());
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
            input_refs.push(reference.clone());
        }
    } else {
        fs::write(root.join("source/fixture.c"), data)?;
        input_refs.push(reference("fixture.c", data)?);
    }
    let registry = SchemaRegistry::new()?;
    let mut task: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/contracts/build-task.json"
    ))?;
    task["inputs"] = json!(
        input_refs
            .iter()
            .map(|r| json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size}))
            .collect::<Vec<_>>()
    );
    let document = registry.validate(DocumentKind::BuildTask, task.clone())?;
    task["task_digest"] = json!(task_digest(&document)?);
    let bytes = serde_json::to_vec(&task)?;
    fs::write(root.join("source/request.json"), &bytes)?;
    let mut references = input_refs;
    references.push(reference("request.json", &bytes)?);
    let snapshot = capture_inputs(&LocalWorkspace::open(&root.join("source"))?, references)?;
    let input_ref = json!({"path":"inputs/request.json","schema":"madb.build-task/1","sha256":Digest::of_bytes(&bytes).as_str(),"size":bytes.len()});
    let files =
        BoundExtensionFiles::new("build", &input_ref, "outputs/request-1", snapshot.clone())?;
    let workspace = LocalExtensionWorkspace::create(
        root,
        &LogicalPath::new("session")?,
        snapshot,
        &LogicalPath::new("outputs/request-1")?,
    )
    .map_err(|e| e.error)?;
    let observed: RefCell<Option<FrozenExtensionResult>> = RefCell::new(None);
    let slow_observer_ran = Cell::new(false);
    let session_id = if component == "toolchain" {
        "session-1".to_owned()
    } else {
        format!("{component}-session")
    };
    let request_id = if component == "toolchain" {
        "request-1".to_owned()
    } else {
        format!("{component}-request")
    };
    let options = SessionOptions {
        component_id: component,
        session_id: &session_id,
        request_id: &request_id,
        protocol: "1.0",
        ir: "1.0",
        required_features: &["core.values@1"],
        handshake_timeout: if mode == "handshake-timeout" {
            Duration::from_millis(200)
        } else if matches!(mode, "slow-observer-phase" | "slow-observer-global") {
            Duration::from_secs(3)
        } else {
            Duration::from_secs(10)
        },
        task_timeout: if matches!(mode, "timeout" | "slow-observer-phase") {
            Duration::from_millis(200)
        } else {
            Duration::from_secs(10)
        },
    };
    let session = HostSession::new(&manifest, &files, options, |output| {
        let captured = files
            .capture_result(workspace.output_workspace(), output)
            .map_err(format_error)?;
        observed.replace(Some(captured));
        let delay = match mode {
            "slow-observer-phase" => Duration::from_millis(450),
            "slow-observer-global" => Duration::from_millis(3500),
            _ => Duration::ZERO,
        };
        if !delay.is_zero() {
            slow_observer_ran.set(true);
            thread::sleep(delay);
        }
        Ok(())
    })?;
    let policy = ProcessPolicy {
        timeout: Duration::from_secs(if mode == "slow-observer-global" {
            3
        } else if matches!(
            mode,
            "handshake-timeout" | "timeout" | "slow-observer-phase"
        ) {
            10
        } else {
            30
        }),
        termination_grace: Duration::from_millis(100),
        cleanup_timeout: Duration::from_millis(500),
        ..ProcessPolicy::default()
    };
    let local = LocalProcessRunner::new(&root.join("tools"), workspace.path(), policy)?;
    let runner = InteractiveProcessRunner::new(&local, session);
    let peer_bytes = fs::read(root.join("tools/peer"))?;
    let mut request = ProcessRequest {
        tool: reference("peer", &peer_bytes)?,
        working_directory: LogicalPath::new("work")?,
        arguments: vec![mode.to_owned()],
        environment: Default::default(),
        stdin: Vec::new(),
    };
    if mode == "wrong-recipe" {
        request.arguments = vec!["ok".into()];
    }
    let control = ExecutionControl::default();
    let cancel = matches!(
        mode,
        "cancel" | "cancel-ok" | "ignore-cancel" | "cancel-before-ack" | "cancel-validation"
    );
    let ready = workspace.output_path().join("ready");
    let (result, readiness) = thread::scope(|scope| {
        let (finished, completion) = mpsc::channel();
        let watcher = if cancel && mode != "cancel-validation" {
            let token = control.clone();
            Some(scope.spawn(move || {
                let start = Instant::now();
                loop {
                    if ready.exists() {
                        return Readiness::MarkerObserved(token.cancel());
                    }
                    if start.elapsed() > Duration::from_secs(20) {
                        return Readiness::DeadlineExpired;
                    }
                    match completion.recv_timeout(Duration::from_millis(2)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Readiness::ExecutionFinishedBeforeMarker;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            }))
        } else {
            None
        };
        let result = execute_process_validated(&runner, &request, &control, |_, _| {
            runner.driver().result_reference()?;
            workspace
                .verify_inputs()
                .map_err(|e| ProcessError::new("InputInvalid", "verify", e.to_string()))?;
            let captured = observed.borrow_mut().take().ok_or_else(|| {
                ProcessError::new("OutputInvalid", "verify", "missing terminal observation")
            })?;
            workspace
                .verify_outputs(captured.files())
                .map_err(|e| ProcessError::new("OutputInvalid", "verify", e.to_string()))?;
            let frozen = captured.into_inputs().map_err(format_error)?;
            if mode == "cancel-validation" {
                control.cancel();
            }
            Ok(frozen)
        });
        // A failed/early execution must wake the fixture watcher. Its missing
        // marker is a separate harness failure, never a replacement for the
        // real process diagnostic, stdout/stderr or cleanup observation.
        let _ = finished.send(());
        let readiness = watcher.map_or(Readiness::NotRequired, |watcher| {
            watcher.join().unwrap_or(Readiness::WatcherPanicked)
        });
        (result, readiness)
    });
    match result {
        Err(error) => Ok((
            json!({"error":error.to_string(),"admitted":false,"readiness":readiness.report(),"harness_error":readiness.failure()}),
            None,
        )),
        Ok(result) => {
            let execution_admitted = result.value.is_some();
            // A fixture that never established its requested cancellation
            // cannot transfer a value downstream, even if the peer exited OK.
            // Keep the actual admission unchanged in the diagnostic report.
            let value = if readiness.failure().is_some() {
                None
            } else {
                result.value
            };
            let process = result.execution.process.as_ref();
            let report = process.map(|p| &p.report);
            let report = json!({"admission":format!("{:?}",result.execution.admission),"admitted":value.is_some(),
                "execution_admitted":execution_admitted,"readiness":readiness.report(),"harness_error":readiness.failure(),
                "files":value.as_ref().map(|v|v.references().map(|r|r.path.as_str()).collect::<Vec<_>>()),
                "diagnostic":report.and_then(|r|r.diagnostic.as_ref()).map(|e|json!({"code":e.code,"stage":e.stage,"message":e.message})),
                "peer_diagnostic":runner.driver().peer_diagnostic(),
                "cleanup":report.map(|r|json!({"complete":r.cleanup.locally_complete(),"leader_reaped":r.cleanup.leader_reaped,"stdout_eof":r.cleanup.stdout_eof,"stderr_eof":r.cleanup.stderr_eof,"group_kill_attempted":r.cleanup.group_kill_attempted,"signal_failed":r.cleanup.signal_failed})),
                "termination":report.map(|r|format!("{:?}",r.termination)),
                "stdin_written":report.map(|r|r.stdin_written),
                "stdout":report.map(|r|String::from_utf8_lossy(&r.stdout.bytes).into_owned()),
                "stderr":report.map(|r|String::from_utf8_lossy(&r.stderr.bytes).into_owned()),
                "cancel_after_finish":format!("{:?}",control.cancel()),"pending_cleanup":process.is_some_and(|p|p.pending_cleanup.is_some()), "component":component, "session":session_id,"request":request_id,"slow_observer_ran":slow_observer_ran.get()});
            Ok((report, value))
        }
    }
}
enum Readiness {
    NotRequired,
    MarkerObserved(CancelDecision),
    ExecutionFinishedBeforeMarker,
    DeadlineExpired,
    WatcherPanicked,
}
impl Readiness {
    fn failure(&self) -> Option<&'static str> {
        match self {
            Self::NotRequired
            | Self::MarkerObserved(CancelDecision::Requested | CancelDecision::AlreadyRequested) => {
                None
            }
            Self::MarkerObserved(CancelDecision::AlreadyFinished) => {
                Some("execution finished before marker cancellation")
            }
            Self::ExecutionFinishedBeforeMarker => {
                Some("execution finished before readiness marker")
            }
            Self::DeadlineExpired => Some("readiness marker deadline expired"),
            Self::WatcherPanicked => Some("readiness watcher panicked"),
        }
    }
    fn report(&self) -> Value {
        match self {
            Self::NotRequired => json!({"status":"not-required"}),
            Self::MarkerObserved(decision) => {
                json!({"status":"marker-observed","cancel_decision":format!("{decision:?}")})
            }
            Self::ExecutionFinishedBeforeMarker => {
                json!({"status":"execution-finished-before-marker"})
            }
            Self::DeadlineExpired => json!({"status":"deadline-expired"}),
            Self::WatcherPanicked => json!({"status":"watcher-panicked"}),
        }
    }
}
fn reference(path: &str, bytes: &[u8]) -> Result<FileReference, Box<dyn std::error::Error>> {
    Ok(FileReference {
        path: LogicalPath::new(path)?,
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    })
}
fn format_error(e: madb_contracts::FormatError) -> ProcessError {
    ProcessError::new(e.code, "extension-files", e.to_string())
}
