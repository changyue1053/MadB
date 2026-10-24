use madb_app::{
    Digest, LogicalPath,
    ports::{FileReference, PendingProcess, ProcessRunner},
    process::*,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

fn request() -> ProcessRequest {
    ProcessRequest {
        tool: FileReference {
            path: LogicalPath::new("tool").unwrap(),
            digest: Digest::of_bytes(b"tool"),
            size: 4,
        },
        working_directory: LogicalPath::new("work").unwrap(),
        arguments: vec![],
        environment: BTreeMap::new(),
        stdin: vec![],
    }
}
fn completed() -> ProcessRun {
    ProcessRun {
        report: ProcessReport {
            termination: Termination::Exited(ProcessExit {
                code: Some(0),
                signal: None,
            }),
            stdout: CapturedOutput {
                bytes: b"raw result".to_vec(),
                observed_bytes: 10,
            },
            stderr: CapturedOutput::default(),
            stdin_written: 0,
            cleanup: ProcessCleanup {
                leader_reaped: true,
                stdout_eof: true,
                stderr_eof: true,
                group_kill_attempted: true,
                signal_failed: false,
            },
            diagnostic: None,
        },
        pending_cleanup: None,
    }
}
struct Runner<F>(F);
impl<F: Fn() -> ProcessResult<ProcessRun>> ProcessRunner for Runner<F> {
    fn run(&self, _: &ProcessRequest, _: &CancellationProbe<'_>) -> ProcessResult<ProcessRun> {
        (self.0)()
    }
}
struct TrackedValue(Arc<AtomicUsize>);
impl Drop for TrackedValue {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cancellation_during_validation_discards_value_before_admission() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let watchdog = Duration::from_secs(5);
    let control = ExecutionControl::default();
    let dropped = Arc::new(AtomicUsize::new(0));
    let worker_control = control.clone();
    let worker_dropped = dropped.clone();
    // An owned worker avoids a scoped join blocking forever after a watchdog
    // failure. The channels establish order; the duration never decides it.
    let task = std::thread::spawn(move || {
        let result = execute_process_validated(
            &Runner(|| Ok(completed())),
            &request(),
            &worker_control,
            |run, probe| {
                assert_eq!(run.report.stdout.bytes, b"raw result");
                assert!(!probe.requested());
                entered_tx
                    .send(())
                    .expect("test must observe validation entry");
                release_rx
                    .recv_timeout(watchdog)
                    .expect("validation release watchdog");
                assert!(probe.requested());
                Ok(TrackedValue(worker_dropped))
            },
        );
        assert!(
            completed_tx.send(result).is_ok(),
            "test must observe execution completion"
        );
    });
    entered_rx
        .recv_timeout(watchdog)
        .expect("validation entry watchdog");
    assert_eq!(control.cancel(), CancelDecision::Requested);
    release_tx
        .send(())
        .expect("validator must wait for cancellation");
    let result = completed_rx
        .recv_timeout(watchdog)
        .expect("execution completion watchdog")
        .unwrap();
    task.join().unwrap();
    assert_eq!(
        result.execution.admission,
        ResultAdmission::RejectedByCancellation
    );
    assert!(result.value.is_none());
    assert!(result.execution.process.is_some());
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

#[test]
fn validation_error_preserves_raw_process_and_closes_the_gate() {
    let control = ExecutionControl::default();
    let result = execute_process_validated::<()>(
        &Runner(|| Ok(completed())),
        &request(),
        &control,
        |_, _| Err(ProcessError::new("TEST001", "validate", "bad evidence")),
    )
    .unwrap();
    assert_eq!(result.execution.admission, ResultAdmission::Unavailable);
    assert!(result.value.is_none());
    let run = result.execution.process.unwrap();
    assert_eq!(run.report.stdout.bytes, b"raw result");
    assert_eq!(run.report.diagnostic.unwrap().code, "TEST001");
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

struct Pending(Arc<AtomicUsize>);
impl PendingProcess for Pending {
    fn poll_reaped(&mut self) -> ProcessResult<bool> {
        Ok(true)
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[test]
fn incomplete_process_never_invokes_validation_and_retains_pending_owner() {
    let owners = Arc::new(AtomicUsize::new(1));
    let result = execute_process_validated::<()>(
        &Runner(|| {
            let mut run = completed();
            run.report.cleanup.leader_reaped = false;
            run.pending_cleanup = Some(Box::new(Pending(owners.clone())));
            Ok(run)
        }),
        &request(),
        &ExecutionControl::default(),
        |_, _| panic!("ineligible validator"),
    )
    .unwrap();
    assert_eq!(result.execution.admission, ResultAdmission::Unavailable);
    assert!(result.value.is_none());
    assert_eq!(owners.load(Ordering::SeqCst), 1);
    let mut run = result.execution.process.unwrap();
    assert!(run.poll_cleanup().unwrap());
    assert_eq!(owners.load(Ordering::SeqCst), 0);
}

#[test]
fn failed_exit_or_diagnostic_never_invokes_validation() {
    for diagnostic in [false, true] {
        let result = execute_process_validated::<()>(
            &Runner(|| {
                let mut run = completed();
                if diagnostic {
                    run.report.diagnostic = Some(ProcessError::new("TEST002", "run", "fault"));
                } else {
                    run.report.termination = Termination::Exited(ProcessExit {
                        code: Some(1),
                        signal: None,
                    });
                }
                Ok(run)
            }),
            &request(),
            &ExecutionControl::default(),
            |_, _| panic!("ineligible validator"),
        )
        .unwrap();
        assert_eq!(result.execution.admission, ResultAdmission::Unavailable);
        assert!(result.value.is_none());
        assert!(result.execution.process.is_some());
    }
}

#[test]
fn cancellation_before_start_or_before_validation_does_not_call_validator() {
    let control = ExecutionControl::default();
    control.cancel();
    let result = execute_process_validated::<()>(
        &Runner(|| panic!("cancelled runner")),
        &request(),
        &control,
        |_, _| panic!("cancelled validator"),
    )
    .unwrap();
    assert!(result.execution.process.is_none());
    assert!(result.value.is_none());
    let control = ExecutionControl::default();
    let result = execute_process_validated::<()>(
        &Runner(|| {
            control.cancel();
            Ok(completed())
        }),
        &request(),
        &control,
        |_, _| panic!("cancelled validator"),
    )
    .unwrap();
    assert_eq!(
        result.execution.admission,
        ResultAdmission::RejectedByCancellation
    );
    assert!(result.execution.process.is_some());
    assert!(result.value.is_none());
}

#[test]
fn validation_panic_closes_the_gate_and_prevents_reuse() {
    let control = ExecutionControl::default();
    let panic = std::panic::catch_unwind(|| {
        let _ = execute_process_validated::<()>(
            &Runner(|| Ok(completed())),
            &request(),
            &control,
            |_, _| panic!("fixture validator panic"),
        );
    });
    assert!(panic.is_err());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert_eq!(
        execute_process_validated::<()>(
            &Runner(|| panic!("reused runner")),
            &request(),
            &control,
            |_, _| panic!("reused validator"),
        )
        .err()
        .unwrap()
        .code,
        "EXEC001"
    );
}

#[test]
fn accepted_validation_wins_over_late_cancellation() {
    let control = ExecutionControl::default();
    let result = execute_process_validated(
        &Runner(|| Ok(completed())),
        &request(),
        &control,
        |run, probe| {
            assert!(!probe.requested());
            Ok(run.report.stdout.bytes.clone())
        },
    )
    .unwrap();
    assert_eq!(result.value.as_deref(), Some(b"raw result".as_slice()));
    assert_eq!(
        result.execution.admission,
        ResultAdmission::EligibleForValidation
    );
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}
