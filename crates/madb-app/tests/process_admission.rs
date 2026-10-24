use madb_app::{
    Digest, LogicalPath,
    ports::{FileReference, PendingProcess, ProcessRunner},
    process::*,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
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
            stdout: CapturedOutput::default(),
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

#[test]
fn cancellation_before_start_never_calls_the_runner() {
    let control = ExecutionControl::default();
    assert_eq!(control.cancel(), CancelDecision::Requested);
    assert_eq!(control.cancel(), CancelDecision::AlreadyRequested);
    let result =
        execute_process(&Runner(|| panic!("must not start")), &request(), &control).unwrap();
    assert_eq!(result.admission, ResultAdmission::RejectedByCancellation);
    assert!(result.process.is_none());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

#[test]
fn cancellation_before_result_admission_rejects_a_late_zero_exit() {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let control = ExecutionControl::default();
    std::thread::scope(|scope| {
        let entered2 = entered.clone();
        let release2 = release.clone();
        let control2 = control.clone();
        let execution = scope.spawn(move || {
            execute_process(
                &Runner(|| {
                    entered2.wait();
                    release2.wait();
                    Ok(completed())
                }),
                &request(),
                &control2,
            )
            .unwrap()
        });
        entered.wait();
        assert_eq!(control.cancel(), CancelDecision::Requested);
        release.wait();
        let result = execution.join().unwrap();
        assert_eq!(result.admission, ResultAdmission::RejectedByCancellation);
        assert!(matches!(
            result.process.unwrap().report.termination,
            Termination::Exited(_)
        ));
    });
}

#[test]
fn accepted_result_wins_over_late_cancellation() {
    let control = ExecutionControl::default();
    let result = execute_process(&Runner(|| Ok(completed())), &request(), &control).unwrap();
    assert_eq!(result.admission, ResultAdmission::EligibleForValidation);
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert_eq!(
        execute_process(&Runner(|| panic!("duplicate start")), &request(), &control)
            .err()
            .unwrap()
            .code,
        "EXEC001"
    );
}

#[test]
fn a_running_control_cannot_launch_a_second_process() {
    let entered = Barrier::new(2);
    let release = Barrier::new(2);
    let control = ExecutionControl::default();
    std::thread::scope(|scope| {
        let execution = scope.spawn(|| {
            execute_process(
                &Runner(|| {
                    entered.wait();
                    release.wait();
                    Ok(completed())
                }),
                &request(),
                &control,
            )
        });
        entered.wait();
        assert_eq!(
            execute_process(&Runner(|| panic!("duplicate start")), &request(), &control)
                .err()
                .unwrap()
                .code,
            "EXEC001"
        );
        release.wait();
        assert_eq!(
            execution.join().unwrap().unwrap().admission,
            ResultAdmission::EligibleForValidation
        );
    });
}

#[test]
fn error_and_unwind_close_the_execution_gate() {
    let control = ExecutionControl::default();
    assert!(
        execute_process(
            &Runner(|| Err(ProcessError::new("EXEC010", "spawn", "fixture"))),
            &request(),
            &control
        )
        .is_err()
    );
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    let control = ExecutionControl::default();
    let panicked = std::panic::catch_unwind(|| {
        let _ = execute_process(&Runner(|| panic!("fixture panic")), &request(), &control);
    });
    assert!(panicked.is_err());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}

struct Deferred {
    polls: usize,
    owners: Arc<AtomicUsize>,
}
impl PendingProcess for Deferred {
    fn poll_reaped(&mut self) -> ProcessResult<bool> {
        self.polls += 1;
        Ok(self.polls == 2)
    }
}
impl Drop for Deferred {
    fn drop(&mut self) {
        self.owners.fetch_sub(1, Ordering::SeqCst);
    }
}
#[test]
fn incomplete_cleanup_keeps_its_owner_and_never_admits_a_result() {
    let owners = Arc::new(AtomicUsize::new(0));
    let runner = Runner(|| {
        let mut run = completed();
        run.report.cleanup.leader_reaped = false;
        owners.fetch_add(1, Ordering::SeqCst);
        run.pending_cleanup = Some(Box::new(Deferred {
            polls: 0,
            owners: owners.clone(),
        }));
        Ok(run)
    });
    let mut result = execute_process(&runner, &request(), &ExecutionControl::default()).unwrap();
    assert_eq!(result.admission, ResultAdmission::Unavailable);
    assert_eq!(owners.load(Ordering::SeqCst), 1);
    let run = result.process.as_mut().unwrap();
    assert!(!run.poll_cleanup().unwrap());
    assert_eq!(owners.load(Ordering::SeqCst), 1);
    assert!(run.poll_cleanup().unwrap());
    assert_eq!(owners.load(Ordering::SeqCst), 0);
    assert_eq!(result.admission, ResultAdmission::Unavailable);
}

#[test]
fn policy_rejects_ambient_environment_and_unbounded_requests() {
    let mut request = request();
    let mut policy = ProcessPolicy::default();
    request
        .environment
        .insert("SECRET".into(), "do-not-print-the-value".into());
    let error = policy.validate(&request).unwrap_err();
    assert_eq!(error.code, "EXEC003");
    assert!(!error.to_string().contains("do-not-print"));
    policy.allowed_environment.insert("SECRET".into());
    assert!(policy.validate(&request).is_ok());
    request.arguments.push("nul\0argument".into());
    assert_eq!(policy.validate(&request).unwrap_err().code, "EXEC002");
    request.arguments.clear();
    policy.timeout = Duration::ZERO;
    assert!(policy.validate(&request).is_err());
}
