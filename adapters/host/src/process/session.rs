//! Interactive extension transport; the existing one-shot runner is unchanged.
//! The child is an approved tool, not an untrusted-code OS sandbox.
//! Synchronous driver callbacks/host IO cannot be preempted. Their elapsed time
//! is checked on return, before changing phases or admitting any result.
use super::LocalProcessRunner;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::{
    child::OwnedChild,
    io_error, open_directory,
    pipes::{Capture, nonblocking},
    supervise,
};
use madb_app::{
    extension::{SessionDriver, SessionStep},
    ports::ProcessRunner,
    process::{
        CancellationProbe, ProcessCleanup, ProcessError, ProcessPolicy, ProcessReport,
        ProcessRequest, ProcessResult, ProcessRun, StopReason, Termination,
    },
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use rustix::process::Signal;
use std::cell::{Ref, RefCell};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
    fs::File,
    io::Write,
    os::unix::fs::MetadataExt,
    thread,
    time::{Duration, Instant},
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use subprocess::{Exec, ExecExt, Redirection};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const FRAME_LIMIT: usize = 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const QUANTUM: Duration = Duration::from_millis(5);

/// Compose with execute_process_validated so cancellation also covers output
/// validation. driver() is available after run returns, before final admission.
pub struct InteractiveProcessRunner<'a, D> {
    local: &'a LocalProcessRunner,
    driver: RefCell<D>,
}
impl<'a, D: SessionDriver> InteractiveProcessRunner<'a, D> {
    pub fn new(local: &'a LocalProcessRunner, driver: D) -> Self {
        Self {
            local,
            driver: RefCell::new(driver),
        }
    }
    pub fn driver(&self) -> Ref<'_, D> {
        self.driver.borrow()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl<D: SessionDriver> ProcessRunner for InteractiveProcessRunner<'_, D> {
    fn run(
        &self,
        request: &ProcessRequest,
        cancellation: &CancellationProbe<'_>,
    ) -> ProcessResult<ProcessRun> {
        let local = self.local;
        local.policy.validate(request)?;
        if !request.stdin.is_empty() {
            return Err(error(
                "InputInvalid",
                "session stdin is supplied by its driver",
            ));
        }
        if cancellation.requested() {
            return Ok(supervise::not_started());
        }
        local.verify_tool(request)?;
        let expected = open_directory(&local.workspace, &request.working_directory)?
            .metadata()
            .map_err(|e| io_error("work-directory", e))?;
        let path = local.work_root.join(request.working_directory.as_str());
        let check = || -> ProcessResult<()> {
            let current = open_directory(&local.workspace, &request.working_directory)?
                .metadata()
                .map_err(|e| io_error("work-directory", e))?;
            let named = std::fs::metadata(&path).map_err(|e| io_error("work-directory", e))?;
            if [current, named]
                .iter()
                .any(|m| m.dev() != expected.dev() || m.ino() != expected.ino())
            {
                return Err(error("InputInvalid", "session directory changed"));
            }
            local.verify_tool(request)
        };
        check()?;
        let mut driver = self
            .driver
            .try_borrow_mut()
            .map_err(|_| error("ProtocolInvalid", "session driver is already in use"))?;
        driver.validate_request(request)?;
        let initial = driver.initial()?;
        validate_outgoing(&initial)?;
        validate_timeout(driver.timeout())?;
        if cancellation.requested() {
            return Ok(supervise::not_started());
        }
        let child = Exec::cmd(local.tool_root.join(request.tool.path.as_str()))
            .args(&request.arguments)
            .env_clear()
            .env_extend(&request.environment)
            .cwd(&path)
            .stdin(Redirection::Pipe)
            .stdout(Redirection::Pipe)
            .stderr(Redirection::Pipe)
            .setpgid()
            .detached()
            .start()
            .map_err(|e| io_error("spawn", e))?;
        let mut run = supervise_session(child, initial, &mut *driver, &local.policy, cancellation);
        if let Err(error) = check() {
            run.report.diagnostic.get_or_insert(error);
        }
        Ok(run)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl<D: SessionDriver> ProcessRunner for InteractiveProcessRunner<'_, D> {
    fn run(&self, _: &ProcessRequest, _: &CancellationProbe<'_>) -> ProcessResult<ProcessRun> {
        Err(error(
            "ToolFailed",
            "interactive process supervision supports Linux/macOS only",
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct SessionPipes {
    input: Option<File>,
    outgoing: Vec<u8>,
    cursor: usize,
    written: usize,
    out: Capture<File>,
    err: Capture<File>,
    read_cursor: usize,
    line: Vec<u8>,
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl SessionPipes {
    fn new(
        child: &mut subprocess::Job,
        initial: Vec<u8>,
        policy: &ProcessPolicy,
    ) -> ProcessResult<Self> {
        let input = child.stdin.take().expect("piped stdin");
        let out = child.stdout.take().expect("piped stdout");
        let err = child.stderr.take().expect("piped stderr");
        nonblocking(&input)?;
        nonblocking(&out)?;
        nonblocking(&err)?;
        Ok(Self {
            input: Some(input),
            outgoing: initial,
            cursor: 0,
            written: 0,
            out: Capture::new(out, policy.stdout_limit),
            err: Capture::new(err, policy.stderr_limit),
            read_cursor: 0,
            line: Vec::new(),
        })
    }
    fn queue(&mut self, bytes: Vec<u8>) -> ProcessResult<()> {
        validate_outgoing(&bytes)?;
        self.outgoing.drain(..self.cursor);
        self.cursor = 0;
        if self.outgoing.len() + bytes.len() > 2 * FRAME_LIMIT {
            return Err(error(
                "ProtocolInvalid",
                "pending control frame budget exceeded",
            ));
        }
        self.outgoing.extend(bytes);
        Ok(())
    }
    fn pump(&mut self) -> ProcessResult<()> {
        if let Some(input) = &mut self.input
            && self.cursor < self.outgoing.len()
        {
            let end = self.outgoing.len().min(self.cursor + 16 * 1024);
            match input.write(&self.outgoing[self.cursor..end]) {
                Ok(0) => return Err(error("ToolFailed", "helper closed its control input")),
                Ok(n) => {
                    self.cursor += n;
                    self.written += n;
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(io_error("session-write", e)),
            }
        }
        self.out.pump()?;
        self.err.pump()?;
        if self.out.output.truncated() {
            return Err(error(
                "ProtocolInvalid",
                "session output byte budget exceeded",
            ));
        }
        Ok(())
    }
    fn frames(&mut self, driver: &mut impl SessionDriver, cancelling: bool) -> ProcessResult<bool> {
        let mut changed = false;
        while self.read_cursor < self.out.output.bytes.len() {
            let b = self.out.output.bytes[self.read_cursor];
            self.read_cursor += 1;
            self.line.push(b);
            if self.line.len() > FRAME_LIMIT {
                return Err(error(
                    "ProtocolInvalid",
                    "control frame byte budget exceeded",
                ));
            }
            if b == b'\n' {
                if !cancelling && self.cursor != self.outgoing.len() {
                    return Err(error(
                        "ProtocolInvalid",
                        "helper replied before the preceding control frame was written",
                    ));
                }
                match driver.receive(&self.line)? {
                    SessionStep::Send(bytes) => {
                        self.queue(bytes)?;
                        changed = true;
                    }
                    SessionStep::Terminal => {
                        self.input.take();
                        changed = true;
                    }
                }
                self.line.clear();
            }
        }
        if self.out.eof && !self.line.is_empty() {
            return Err(error("ProtocolInvalid", "truncated control frame at EOF"));
        }
        Ok(changed)
    }
    fn eof(&self) -> bool {
        self.out.eof && self.err.eof
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn supervise_session(
    child: subprocess::Job,
    initial: Vec<u8>,
    driver: &mut impl SessionDriver,
    policy: &ProcessPolicy,
    cancellation: &CancellationProbe<'_>,
) -> ProcessRun {
    let mut owner = OwnedChild::new(child);
    let mut pipes = match SessionPipes::new(owner.child_mut(), initial, policy) {
        Ok(pipes) => pipes,
        Err(e) => return supervise::abort_setup(owner, policy, e),
    };
    let started = Instant::now();
    let mut phase = started;
    let mut stop = None;
    let mut stop_at = None;
    let mut term_at = None;
    let mut kill_at = None;
    let mut exited = false;
    let mut forced = false;
    let mut signal_failed = false;
    let mut diagnostic = None;
    let mut frames_enabled = true;
    loop {
        let now = Instant::now();
        let phase_budget = if driver.is_terminal() {
            policy.cleanup_timeout
        } else {
            driver.timeout()
        };
        if stop.is_none() {
            if cancellation.requested() {
                stop = Some(StopReason::Cancellation);
            }
            if now.duration_since(phase) >= phase_budget
                || now.duration_since(started) >= policy.timeout
            {
                stop.get_or_insert(StopReason::Deadline);
                diagnostic.get_or_insert_with(|| {
                    error(
                        "ToolTimeout",
                        "extension handshake, task or final-exit budget exceeded",
                    )
                });
            }
            if stop.is_some() {
                stop_at = Some(now);
                match driver.cancel().and_then(|bytes| match bytes {
                    Some(bytes) => pipes.queue(bytes),
                    None => {
                        pipes.input.take();
                        Ok(())
                    }
                }) {
                    Ok(()) => {}
                    Err(e) => {
                        diagnostic.get_or_insert(e);
                        pipes.input.take();
                    }
                }
            }
        }
        let pumped = pipes.pump().and_then(|()| {
            if frames_enabled {
                pipes.frames(driver, stop.is_some())
            } else {
                Ok(false)
            }
        });
        // Preserve the *preceding* phase deadline across synchronous callbacks.
        // Resetting phase first would erase time spent observing a terminal.
        let returned = Instant::now();
        if stop.is_none()
            && (returned.duration_since(phase) >= phase_budget
                || returned.duration_since(started) >= policy.timeout)
        {
            stop = Some(StopReason::Deadline);
            stop_at = Some(returned);
            diagnostic.get_or_insert_with(|| {
                error(
                    "ToolTimeout",
                    "session budget exceeded during synchronous IO or callback",
                )
            });
            let _ = driver.cancel();
            // No newly queued invoke may be sent after its handshake expired.
            pipes.input.take();
        }
        match pumped {
            Ok(true) => {
                phase = Instant::now();
                if let Err(e) = validate_timeout(driver.timeout()) {
                    diagnostic.get_or_insert(e);
                    stop.get_or_insert(StopReason::HostIo);
                }
            }
            Ok(false) => {}
            Err(e) => {
                diagnostic.get_or_insert(e);
                stop.get_or_insert(StopReason::HostIo);
                pipes.input.take();
                frames_enabled = false;
            }
        }
        if stop.is_some() {
            stop_at.get_or_insert_with(Instant::now);
        }
        if !exited {
            match owner.observe_exit() {
                Ok(value) => exited = value,
                Err(e) => {
                    diagnostic.get_or_insert(e);
                    stop.get_or_insert(StopReason::HostIo);
                    if !owner.still_owned() {
                        signal_failed = true;
                        break;
                    }
                }
            }
        }
        if exited {
            pipes.input.take();
            if !owner.kill_sent {
                if let Err(e) = owner.signal(Signal::KILL) {
                    signal_failed = true;
                    diagnostic.get_or_insert(e);
                }
                kill_at = Some(Instant::now());
            }
            if pipes.eof() {
                if !driver.is_terminal() {
                    diagnostic.get_or_insert_with(|| {
                        error("ToolFailed", "helper exited before a terminal result")
                    });
                }
                break;
            }
        } else if stop_at.is_some_and(|at| at.elapsed() >= policy.termination_grace) {
            pipes.input.take();
            let terminated = *term_at.get_or_insert_with(|| {
                if let Err(e) = owner.signal(Signal::TERM) {
                    signal_failed = true;
                    diagnostic.get_or_insert(e);
                }
                Instant::now()
            });
            if terminated.elapsed() >= policy.termination_grace && !owner.kill_sent {
                forced = true;
                if let Err(e) = owner.signal(Signal::KILL) {
                    signal_failed = true;
                    diagnostic.get_or_insert(e);
                }
                kill_at = Some(Instant::now());
            }
        }
        if kill_at.is_some_and(|at| at.elapsed() >= policy.cleanup_timeout) {
            stop.get_or_insert(StopReason::PipeTimeout);
            diagnostic.get_or_insert_with(|| {
                error(
                    "ToolTimeout",
                    "session cleanup incomplete; quarantine workspace and retain child owner",
                )
            });
            break;
        }
        thread::sleep(QUANTUM);
    }
    let exit = if owner.kill_sent {
        match owner.reap() {
            Ok(exit) => exit,
            Err(e) => {
                diagnostic.get_or_insert(e);
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
    let final_phase_budget = if driver.is_terminal() {
        policy.cleanup_timeout
    } else {
        driver.timeout()
    };
    if stop.is_none()
        && (started.elapsed() >= policy.timeout || phase.elapsed() >= final_phase_budget)
    {
        stop = Some(StopReason::Deadline);
        diagnostic.get_or_insert_with(|| error("ToolTimeout", "session final deadline exceeded"));
    }
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_outgoing(bytes: &[u8]) -> ProcessResult<()> {
    if bytes.is_empty()
        || bytes.len() > FRAME_LIMIT
        || bytes.last() != Some(&b'\n')
        || bytes[..bytes.len() - 1].contains(&b'\n')
    {
        return Err(error(
            "ProtocolInvalid",
            "driver must send one bounded JSON line",
        ));
    }
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_timeout(timeout: Duration) -> ProcessResult<()> {
    if timeout.is_zero() || timeout > Duration::from_secs(3600) {
        return Err(error("InputInvalid", "invalid session phase timeout"));
    }
    Ok(())
}
fn error(code: &'static str, message: &'static str) -> ProcessError {
    ProcessError::new(code, "extension-session", message)
}
