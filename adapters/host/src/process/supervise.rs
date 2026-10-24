use super::{child::OwnedChild, pipes::Pipes};
use madb_app::process::{
    CancellationProbe, CapturedOutput, ProcessCleanup, ProcessError, ProcessPolicy, ProcessReport,
    ProcessRequest, ProcessRun, StopReason, Termination,
};
use rustix::process::Signal;
use std::{
    thread,
    time::{Duration, Instant},
};

const QUANTUM: Duration = Duration::from_millis(5);

pub(super) fn not_started() -> ProcessRun {
    ProcessRun {
        report: ProcessReport {
            termination: Termination::Stopped {
                reason: StopReason::Cancellation,
                forced: false,
                exit: None,
            },
            stdout: CapturedOutput::default(),
            stderr: CapturedOutput::default(),
            stdin_written: 0,
            cleanup: ProcessCleanup {
                leader_reaped: true,
                stdout_eof: true,
                stderr_eof: true,
                ..Default::default()
            },
            diagnostic: None,
        },
        pending_cleanup: None,
    }
}

pub(super) fn run(
    child: subprocess::Job,
    request: &ProcessRequest,
    policy: &ProcessPolicy,
    cancellation: &CancellationProbe<'_>,
) -> ProcessRun {
    let mut owner = OwnedChild::new(child);
    let mut pipes = match Pipes::new(owner.child_mut(), policy) {
        Ok(pipes) => pipes,
        Err(error) => return abort_setup(owner, policy, error),
    };
    let start = Instant::now();
    let mut stop = None;
    let mut stop_at = None;
    let mut kill_at = None;
    let mut exited = false;
    let mut forced = false;
    let mut signal_failed = false;
    let mut diagnostic = None;
    loop {
        let now = Instant::now();
        if cancellation.requested() {
            stop.get_or_insert(StopReason::Cancellation);
        }
        if now.duration_since(start) >= policy.timeout {
            stop.get_or_insert(StopReason::Deadline);
        }
        if stop.is_some() {
            pipes.close_input();
        }
        match pipes.pump(&request.stdin) {
            Ok(Some(reason)) => {
                stop.get_or_insert(reason);
            }
            Ok(None) => {}
            Err(error) => {
                stop.get_or_insert(StopReason::HostIo);
                diagnostic.get_or_insert(error);
            }
        }
        if !exited {
            match owner.observe_exit() {
                Ok(value) => exited = value,
                Err(error) => {
                    stop.get_or_insert(StopReason::HostIo);
                    diagnostic.get_or_insert(error);
                    if !owner.still_owned() {
                        signal_failed = true;
                        break;
                    }
                }
            }
        }
        if exited {
            if pipes.written < request.stdin.len() {
                stop.get_or_insert(StopReason::StdinClosed);
            }
            pipes.close_input();
            // Even a successful tool must not leave workers in our group.
            // Keep the waitable leader until this last signal has been sent.
            if !owner.kill_sent {
                if let Err(error) = owner.signal(Signal::KILL) {
                    signal_failed = true;
                    diagnostic.get_or_insert(error);
                }
                kill_at = Some(Instant::now());
            }
            if pipes.eof() {
                break;
            }
        } else if stop.is_some() {
            pipes.close_input();
            let stopped = *stop_at.get_or_insert_with(|| {
                if let Err(error) = owner.signal(Signal::TERM) {
                    signal_failed = true;
                    diagnostic.get_or_insert(error);
                }
                Instant::now()
            });
            if stopped.elapsed() >= policy.termination_grace && !owner.kill_sent {
                forced = true;
                if let Err(error) = owner.signal(Signal::KILL) {
                    signal_failed = true;
                    diagnostic.get_or_insert(error);
                }
                kill_at = Some(Instant::now());
            }
        }
        if kill_at.is_some_and(|time| time.elapsed() >= policy.cleanup_timeout) {
            stop.get_or_insert(StopReason::PipeTimeout);
            diagnostic.get_or_insert_with(|| {
                ProcessError::new(
                    "EXEC016",
                    "cleanup",
                    "cleanup deadline exceeded; retain any pending child and quarantine outputs",
                )
            });
            break;
        }
        thread::sleep(QUANTUM);
    }
    // No group signalling is permitted after this point. A pending owner has
    // already made its last group-kill attempt and can only poll its own child.
    let exit = if owner.kill_sent {
        match owner.reap() {
            Ok(exit) => exit,
            Err(error) => {
                diagnostic.get_or_insert(error);
                None
            }
        }
    } else {
        None
    };
    let cleanup = ProcessCleanup {
        leader_reaped: exit.is_some(),
        stdout_eof: pipes.out.eof,
        stderr_eof: pipes.err.eof,
        group_kill_attempted: owner.kill_sent,
        signal_failed,
    };
    let termination = match (stop, exit) {
        (None, Some(exit)) => Termination::Exited(exit),
        (reason, exit) => Termination::Stopped {
            reason: reason.unwrap_or(StopReason::HostIo),
            forced,
            exit,
        },
    };
    ProcessRun {
        report: ProcessReport {
            termination,
            stdout: pipes.out.output,
            stderr: pipes.err.output,
            stdin_written: pipes.written,
            cleanup,
            diagnostic,
        },
        pending_cleanup: owner
            .still_owned()
            .then(|| Box::new(owner) as Box<dyn madb_app::ports::PendingProcess>),
    }
}

fn abort_setup(mut owner: OwnedChild, policy: &ProcessPolicy, error: ProcessError) -> ProcessRun {
    let signal_failed = owner.signal(Signal::KILL).is_err();
    let start = Instant::now();
    let exit = loop {
        match owner.reap() {
            Ok(Some(exit)) => break Some(exit),
            Err(_) => break None,
            Ok(None) if start.elapsed() >= policy.cleanup_timeout => break None,
            Ok(None) => thread::sleep(QUANTUM),
        }
    };
    ProcessRun {
        report: ProcessReport {
            termination: Termination::Stopped {
                reason: StopReason::HostIo,
                forced: true,
                exit,
            },
            stdout: CapturedOutput::default(),
            stderr: CapturedOutput::default(),
            stdin_written: 0,
            cleanup: ProcessCleanup {
                leader_reaped: exit.is_some(),
                group_kill_attempted: true,
                signal_failed,
                ..Default::default()
            },
            diagnostic: Some(error),
        },
        pending_cleanup: owner
            .still_owned()
            .then(|| Box::new(owner) as Box<dyn madb_app::ports::PendingProcess>),
    }
}
