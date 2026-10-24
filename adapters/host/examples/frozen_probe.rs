//! Test-only driver: expectations come from independently prepared fixture JSON.
use madb_app::{
    Digest, LogicalPath, QueryError, inputs::capture_inputs, ports::FileReference, process::*,
};
use madb_host::{LocalWorkspace, inputs::PreparedRun};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}

fn reference(value: &Value) -> FileReference {
    FileReference {
        path: LogicalPath::new(string(value, "path")).expect("fixture logical path"),
        digest: Digest::new(string(value, "sha256")).expect("fixture digest"),
        size: value["size"].as_u64().expect("fixture size"),
    }
}

fn query_error(phase: &str, error: &QueryError, retained: Option<&Path>) -> Value {
    json!({
        "phase": phase,
        "kind": format!("{:?}", error.kind),
        "codes": error.findings.iter().map(|finding| finding.code).collect::<Vec<_>>(),
        "message": error.to_string(),
        "retained_path": retained,
        "retained_exists": retained.is_some_and(Path::exists),
    })
}

fn execution_value(execution: &ProcessResult<ProcessExecution>) -> Value {
    match execution {
        Err(error) => json!({"phase": "execute", "error": error.code, "stage": error.stage}),
        Ok(execution) => match &execution.process {
            None => {
                json!({"phase": "execute", "admission": format!("{:?}", execution.admission), "started": false})
            }
            Some(run) => json!({
                "phase": "execute", "admission": format!("{:?}", execution.admission), "started": true,
                "termination": format!("{:?}", run.report.termination),
                "stdout": String::from_utf8_lossy(&run.report.stdout.bytes),
                "stderr": String::from_utf8_lossy(&run.report.stderr.bytes),
                "cleanup_complete": run.report.cleanup.locally_complete(),
                "leader_reaped": run.report.cleanup.leader_reaped,
                "stdout_eof": run.report.cleanup.stdout_eof,
                "stderr_eof": run.report.cleanup.stderr_eof,
                "pending_cleanup": run.pending_cleanup.is_some(),
                "diagnostic": run.report.diagnostic.as_ref().map(|error| error.code),
                "diagnostic_stage": run.report.diagnostic.as_ref().map(|error| error.stage),
            }),
        },
    }
}

fn make_writable(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn mutate_inputs(root: &Path, action: &str) {
    let inputs = root.join("inputs");
    let value = inputs.join("value");
    match action {
        "pre-tamper" => {
            make_writable(&value);
            fs::write(value, b"tampered input").unwrap();
        }
        "pre-extra-file" => {
            make_writable(&inputs);
            fs::write(inputs.join("undeclared"), b"extra").unwrap();
        }
        "pre-extra-directory" => {
            make_writable(&inputs);
            fs::create_dir(inputs.join("undeclared")).unwrap();
        }
        "pre-missing" => {
            make_writable(&inputs);
            fs::remove_file(value).unwrap();
        }
        "pre-symlink" | "pre-hardlink" => {
            make_writable(&inputs);
            let target = root.join("work/replacement");
            fs::copy(&value, &target).unwrap();
            fs::remove_file(&value).unwrap();
            if action == "pre-symlink" {
                symlink(target, value).unwrap();
            } else {
                fs::hard_link(target, value).unwrap();
            }
        }
        _ => {}
    }
}

fn run(config: &Value) -> Value {
    let source = PathBuf::from(string(config, "source_root"));
    let parent = PathBuf::from(string(config, "run_parent"));
    let name = LogicalPath::new(string(config, "run_name")).unwrap();
    let workspace = LocalWorkspace::open(&source).unwrap();
    let refs = config["file_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(reference)
        .collect();
    let snapshot = match capture_inputs(&workspace, refs) {
        Ok(snapshot) => snapshot,
        Err(error) => return query_error("capture", &error, None),
    };
    let prepared = match PreparedRun::create(&parent, &name, snapshot) {
        Ok(prepared) => prepared,
        Err(error) => return query_error("prepare", &error.error, error.retained_path.as_deref()),
    };
    let root = prepared.path().to_owned();
    let mode = config["mode"].as_str().unwrap_or("normal");
    let action = config["action"].as_str().unwrap_or("none");
    if action == "change-source" {
        fs::write(source.join("value"), b"new source after capture").unwrap();
    }
    mutate_inputs(&root, action);
    if action == "prepare-only" {
        drop(prepared);
        return json!({"phase": "prepared", "retained_path": root, "retained_after_drop": root.is_dir()});
    }
    let tool = PathBuf::from(string(&config["tool"], "path"));
    let request = ProcessRequest {
        tool: FileReference {
            path: LogicalPath::new(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
            digest: Digest::new(string(&config["tool"], "sha256")).unwrap(),
            size: config["tool"]["size"].as_u64().unwrap(),
        },
        working_directory: LogicalPath::new(if action == "bad-cwd" {
            "inputs"
        } else {
            "work"
        })
        .unwrap(),
        arguments: vec![
            "../inputs/helper.py".into(),
            mode.into(),
            "../inputs/value".into(),
            root.to_str().unwrap().into(),
        ],
        environment: BTreeMap::new(),
        stdin: vec![],
    };
    let policy = ProcessPolicy {
        timeout: Duration::from_secs(3),
        termination_grace: Duration::from_millis(100),
        cleanup_timeout: Duration::from_millis(300),
        stdout_limit: 4096,
        stderr_limit: 4096,
        ..Default::default()
    };
    let control = ExecutionControl::default();
    if action == "cancel-before" {
        control.cancel();
    }
    let finished = Arc::new(AtomicBool::new(false));
    let observer = if action == "cancel-ready" {
        let root = root.clone();
        let control = control.clone();
        let finished = finished.clone();
        Some(std::thread::spawn(move || {
            let start = Instant::now();
            while !root.join("work/ready").exists() {
                assert!(
                    !finished.load(Ordering::Acquire),
                    "fixture ended before readiness"
                );
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "fixture readiness timeout"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            control.cancel();
        }))
    } else {
        None
    };
    let retained = prepared.execute(tool.parent().unwrap(), policy, &request, &control);
    finished.store(true, Ordering::Release);
    if let Some(observer) = observer {
        observer.join().unwrap();
    }
    let mut output = execution_value(retained.execution());
    output["retained_path"] = json!(retained.path());
    output["cancellation_after"] = json!(format!("{:?}", control.cancel()));
    drop(retained);
    output["retained_after_drop"] = json!(root.is_dir());
    output
}

fn main() {
    let config_path = std::env::args().nth(1).expect("fixture JSON path");
    let config: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    let start = Instant::now();
    let result = run(&config);
    println!(
        "{}",
        json!({"result": result, "elapsed_ms": start.elapsed().as_millis()})
    );
}
