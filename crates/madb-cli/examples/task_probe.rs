//! Test-only composition root for independent one-shot build recipe fixtures.
use madb_app::{
    Digest, LogicalPath, QueryError,
    ports::{ConfigurationBytes, FileReference},
    process::{CancelDecision, ExecutionControl, ProcessExecution, ProcessPolicy, ProcessResult},
    task::{BuildRecipe, TaskFileReferences, TaskInvocation, ValidatedTaskResult},
};
use madb_contracts::task::BoundBuildTask;
use madb_host::{LocalWorkspace, task::PreparedTask};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct ControlledRecipe<'a> {
    inner: &'a BoundBuildTask,
    control: &'a ExecutionControl,
    cancel_validation: bool,
    validation_calls: Cell<u32>,
    cancel_decision: RefCell<Option<CancelDecision>>,
    action: &'a str,
    run_path: &'a Path,
}
impl BuildRecipe for ControlledRecipe<'_> {
    fn invocation(&self) -> &TaskInvocation {
        self.inner.invocation()
    }
    fn validate_result(&self, stdout: &[u8]) -> madb_app::Result<TaskFileReferences> {
        self.validation_calls.set(self.validation_calls.get() + 1);
        let result = self.inner.validate_result(stdout)?;
        if self.cancel_validation {
            self.cancel_decision.replace(Some(self.control.cancel()));
        }
        if self.action == "tamper-input-validating" {
            let path = self.run_path.join("inputs/value");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(path, b"changed inside validation").unwrap();
        }
        if self.action == "replace-work-validating" {
            fs::rename(self.run_path.join("work"), self.run_path.join("old-work")).unwrap();
            fs::create_dir(self.run_path.join("work")).unwrap();
        }
        Ok(result)
    }
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}

fn query_error(phase: &str, error: &QueryError, retained: Option<&Path>) -> Value {
    json!({
        "phase": phase, "codes": error.findings.iter().map(|f| f.code).collect::<Vec<_>>(),
        "message": error.to_string(), "retained_path": retained,
    })
}

fn files(result: Option<&ValidatedTaskResult>) -> Value {
    result.map_or(Value::Null, |result| {
        let encode = |(reference, bytes): (&FileReference, &[u8])| {
            json!({"path":reference.path.as_str(),"size":reference.size,
                   "sha256":reference.digest.as_str(),"bytes":bytes})
        };
        json!({"outputs":result.outputs().map(encode).collect::<Vec<_>>(),
               "logs":result.logs().map(encode).collect::<Vec<_>>()})
    })
}

fn execution(execution: &ProcessResult<ProcessExecution>) -> Value {
    match execution {
        Err(error) => json!({"phase":"execute","error":error.code,"stage":error.stage,
                            "source":error.source().map(ToString::to_string)}),
        Ok(execution) => match &execution.process {
            None => {
                json!({"phase":"execute","admission":format!("{:?}",execution.admission),"started":false})
            }
            Some(run) => json!({
                "phase":"execute","admission":format!("{:?}",execution.admission),"started":true,
                "termination":format!("{:?}",run.report.termination),
                "diagnostic":run.report.diagnostic.as_ref().map(|e|e.code),
                "diagnostic_stage":run.report.diagnostic.as_ref().map(|e|e.stage),
                "diagnostic_source":run.report.diagnostic.as_ref().and_then(|e|e.source()).map(ToString::to_string),
                "leader_reaped":run.report.cleanup.leader_reaped,
                "cleanup_complete":run.report.cleanup.locally_complete(),
                "pending_cleanup":run.pending_cleanup.is_some(),
            }),
        },
    }
}

fn run(config: &Value) -> Value {
    let source = PathBuf::from(string(config, "source_root"));
    let parent = PathBuf::from(string(config, "run_parent"));
    let tool = PathBuf::from(string(&config["tool"], "path"));
    let action = config["action"].as_str().unwrap_or("none");
    let project = fs::read(source.join("project.json")).unwrap();
    let request = fs::read(source.join("read.json")).unwrap();
    let lock = fs::read(source.join("madb.lock")).unwrap();
    let profile = fs::read(source.join("profiles/rust-c.json")).unwrap();
    let ir = fs::read(source.join("contract.json")).unwrap();
    let task = fs::read(source.join("task.json")).unwrap();
    let recipe = match BoundBuildTask::new(
        ConfigurationBytes {
            request: &request,
            project: &project,
            lock: &lock,
            profile: &profile,
        },
        &ir,
        &task,
        FileReference {
            path: LogicalPath::new(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
            digest: Digest::new(string(&config["tool"], "sha256")).unwrap(),
            size: config["tool"]["size"].as_u64().unwrap(),
        },
    ) {
        Ok(recipe) => recipe,
        Err(error) => return query_error("bind", &error, None),
    };
    let control = ExecutionControl::default();
    let expected_run_path = parent.join("run");
    let recipe = ControlledRecipe {
        inner: &recipe,
        control: &control,
        cancel_validation: action == "cancel-validating",
        validation_calls: Cell::new(0),
        cancel_decision: RefCell::new(None),
        action,
        run_path: &expected_run_path,
    };
    let workspace = LocalWorkspace::open(&source).unwrap();
    let prepared = match PreparedTask::create(
        &parent,
        &LogicalPath::new("run").unwrap(),
        &workspace,
        &recipe,
    ) {
        Ok(prepared) => prepared,
        Err(error) => return query_error("prepare", &error.error, error.retained_path.as_deref()),
    };
    let path = prepared.path().to_owned();
    if action == "change-source" {
        fs::write(source.join("value"), b"changed after preparation").unwrap();
    }
    if action == "cancel-before" {
        control.cancel();
    }
    let finished = Arc::new(AtomicBool::new(false));
    let observer = if action == "cancel-running" {
        let marker = PathBuf::from(string(config, "marker"));
        let control = control.clone();
        let finished = finished.clone();
        Some(std::thread::spawn(move || {
            let start = Instant::now();
            loop {
                let ready = fs::read(&marker)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|v| v["ready"] == true);
                if ready {
                    break;
                }
                assert!(
                    !finished.load(Ordering::Acquire),
                    "fixture ended before readiness"
                );
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "readiness deadline"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            control.cancel()
        }))
    } else {
        None
    };
    let retained = prepared.execute(
        tool.parent().unwrap(),
        ProcessPolicy {
            timeout: Duration::from_secs(3),
            termination_grace: Duration::from_millis(100),
            cleanup_timeout: Duration::from_millis(300),
            stdout_limit: 1024 * 1024,
            stderr_limit: 4096,
            ..Default::default()
        },
        &control,
    );
    finished.store(true, Ordering::Release);
    let observer_cancel = observer.map(|observer| format!("{:?}", observer.join().unwrap()));
    let mut result = execution(retained.execution());
    result["value"] = files(retained.value());
    if action == "change-result" {
        fs::write(
            path.join("work/output.bin"),
            b"changed after captured result",
        )
        .unwrap();
        result["value_after_change"] = files(retained.value());
    }
    result["validation_calls"] = json!(recipe.validation_calls.get());
    result["validation_cancel"] = json!(
        recipe
            .cancel_decision
            .borrow()
            .as_ref()
            .map(|d| format!("{d:?}"))
    );
    result["observer_cancel"] = json!(observer_cancel);
    result["late_cancel"] = json!(format!("{:?}", control.cancel()));
    result["retained_path"] = json!(retained.path());
    drop(retained);
    result["retained_after_drop"] = json!(path.is_dir());
    result
}

fn main() {
    let config: Value = serde_json::from_slice(
        &fs::read(std::env::args().nth(1).expect("fixture config")).unwrap(),
    )
    .unwrap();
    let start = Instant::now();
    let result = run(&config);
    println!(
        "{}",
        json!({"result":result,"elapsed_ms":start.elapsed().as_millis()})
    );
}
