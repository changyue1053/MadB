//! Test driver for the internal process port; not an installed MadB command.
use madb_app::{Digest, LogicalPath, ports::FileReference, process::*};
use madb_host::process::LocalProcessRunner;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tool = PathBuf::from(&args[1]);
    let root = PathBuf::from(&args[4]);
    let mode = &args[5];
    let action = &args[6];
    let mut request = ProcessRequest {
        tool: FileReference {
            path: LogicalPath::new(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
            digest: Digest::new(&args[2]).unwrap(),
            size: args[3].parse().unwrap(),
        },
        working_directory: LogicalPath::new("work").unwrap(),
        arguments: vec![
            root.join("helper.py").to_str().unwrap().into(),
            mode.clone(),
            "spaces ; $(touch forbidden) * \"quoted\"".into(),
        ],
        environment: BTreeMap::from([("SAFE_VALUE".into(), "explicit-value".into())]),
        stdin: b"hello input".to_vec(),
    };
    if ["backpressure", "no-read", "close-stdin"].contains(&mode.as_str()) {
        request.stdin = vec![b'a'; MAX_STDIN_BYTES];
    }
    if mode == "bad-env" {
        request
            .environment
            .insert("SECRET".into(), "never-leak-this".into());
    }
    if mode == "nul-arg" {
        request.arguments.push("bad\0argument".into());
    }
    let policy = ProcessPolicy {
        timeout: Duration::from_millis(
            if ["sleep", "ignore-term", "no-read"].contains(&mode.as_str()) {
                500
            } else {
                3000
            },
        ),
        termination_grace: Duration::from_millis(100),
        cleanup_timeout: Duration::from_millis(300),
        stdout_limit: if mode == "stdout-flood" { 1024 } else { 65536 },
        stderr_limit: 256,
        allowed_environment: BTreeSet::from(["SAFE_VALUE".into()]),
    };
    let runner = LocalProcessRunner::new(tool.parent().unwrap(), &root, policy).unwrap();
    let control = ExecutionControl::default();
    if action == "before" {
        control.cancel();
    }
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observer = if ["on-ready", "change-tool", "change-directory"].contains(&action.as_str()) {
        let control = control.clone();
        let root = root.clone();
        let tool = tool.clone();
        let action = action.clone();
        let finished = finished.clone();
        Some(std::thread::spawn(move || {
            let start = Instant::now();
            while !root.join("work/ready").exists() {
                assert!(
                    !finished.load(std::sync::atomic::Ordering::Acquire),
                    "process ended before fixture barrier"
                );
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "fixture barrier timeout"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            match action.as_str() {
                "on-ready" => {
                    control.cancel();
                }
                "change-tool" => {
                    std::fs::write(tool, b"changed tool image").unwrap();
                    std::fs::write(root.join("work/release"), b"go").unwrap();
                }
                "change-directory" => {
                    std::fs::rename(root.join("work"), root.join("moved")).unwrap();
                    std::fs::write(root.join("moved/release"), b"go").unwrap();
                }
                _ => unreachable!(),
            }
        }))
    } else {
        None
    };
    let start = Instant::now();
    let execution = execute_process(&runner, &request, &control);
    finished.store(true, std::sync::atomic::Ordering::Release);
    if let Some(observer) = observer {
        observer.join().unwrap();
    }
    let cancellation_after = format!("{:?}", control.cancel());
    let output = match execution {
        Err(error) => {
            json!({"error": error.code, "stage": error.stage, "message": error.to_string(), "cancellation_after": cancellation_after})
        }
        Ok(execution) => {
            let admission = format!("{:?}", execution.admission);
            match execution.process {
                None => {
                    json!({"admission": admission, "started": false, "cancellation_after": cancellation_after})
                }
                Some(run) => {
                    json!({"admission": admission, "started": true, "termination": format!("{:?}", run.report.termination), "stdout": String::from_utf8_lossy(&run.report.stdout.bytes), "stderr": String::from_utf8_lossy(&run.report.stderr.bytes), "stdout_bytes": run.report.stdout.observed_bytes, "stderr_bytes": run.report.stderr.observed_bytes, "stderr_truncated": run.report.stderr.truncated(), "stdin_written": run.report.stdin_written, "leader_reaped": run.report.cleanup.leader_reaped, "stdout_eof": run.report.cleanup.stdout_eof, "stderr_eof": run.report.cleanup.stderr_eof, "group_kill_attempted": run.report.cleanup.group_kill_attempted, "cleanup_complete": run.report.cleanup.locally_complete(), "pending_cleanup": run.pending_cleanup.is_some(), "diagnostic": run.report.diagnostic.as_ref().map(|d| d.code), "diagnostic_stage": run.report.diagnostic.as_ref().map(|d| d.stage), "diagnostic_detail": run.report.diagnostic.as_ref().map(|d| format!("{d:?}")), "cancellation_after": cancellation_after})
                }
            }
        }
    };
    println!(
        "{}",
        json!({"result": output, "elapsed_ms": start.elapsed().as_millis()})
    );
}
