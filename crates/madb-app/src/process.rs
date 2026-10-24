//! One process execution, its cancellation admission and bounded observations.
//! Process completion is not payload validation or an artifact commit.
use crate::LogicalPath;
use crate::ports::{FileReference, PendingProcess, ProcessRunner};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

pub const MAX_TOOL_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_STDIN_BYTES: usize = 1024 * 1024;

pub struct ProcessRequest {
    pub tool: FileReference,
    pub working_directory: LogicalPath,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub stdin: Vec<u8>,
}

#[derive(Clone)]
pub struct ProcessPolicy {
    /// Budget after successful spawn and pipe setup; not a wall-clock bound
    /// on tool/directory validation, OS spawn, or post-run filesystem reads.
    pub timeout: Duration,
    pub termination_grace: Duration,
    pub cleanup_timeout: Duration,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub allowed_environment: BTreeSet<String>,
}
impl Default for ProcessPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            termination_grace: Duration::from_millis(250),
            cleanup_timeout: Duration::from_secs(2),
            stdout_limit: 1024 * 1024,
            stderr_limit: 64 * 1024,
            allowed_environment: BTreeSet::new(),
        }
    }
}
impl ProcessPolicy {
    pub fn validate(&self, request: &ProcessRequest) -> ProcessResult<()> {
        if self.timeout.is_zero()
            || self.timeout > Duration::from_secs(3600)
            || self.termination_grace > Duration::from_secs(30)
            || self.cleanup_timeout.is_zero()
            || self.cleanup_timeout > Duration::from_secs(30)
            || self.stdout_limit > 16 * 1024 * 1024
            || self.stderr_limit > 1024 * 1024
        {
            return Err(ProcessError::new(
                "EXEC002",
                "policy",
                "invalid process budgets",
            ));
        }
        if request.tool.size == 0
            || request.tool.size > MAX_TOOL_BYTES
            || request.stdin.len() > MAX_STDIN_BYTES
            || request.arguments.len() > 4096
            || request.environment.len() > 256
        {
            return Err(ProcessError::new(
                "EXEC002",
                "request",
                "process request exceeds budget",
            ));
        }
        let mut size = 0usize;
        for argument in &request.arguments {
            size = size
                .checked_add(argument.len())
                .ok_or_else(request_budget)?;
            if argument.contains('\0') {
                return Err(request_budget());
            }
        }
        for (name, value) in &request.environment {
            if !valid_environment_name(name) || !self.allowed_environment.contains(name) {
                return Err(ProcessError::new(
                    "EXEC003",
                    "environment",
                    format!("environment name is not permitted: {name:?}"),
                ));
            }
            if value.contains('\0') {
                return Err(request_budget());
            }
            size = size
                .checked_add(name.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or_else(request_budget)?;
        }
        if size > 128 * 1024 {
            return Err(request_budget());
        }
        Ok(())
    }
}
fn request_budget() -> ProcessError {
    ProcessError::new(
        "EXEC002",
        "request",
        "invalid or oversized arguments/environment",
    )
}
fn valid_environment_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

#[derive(Debug)]
pub struct ProcessError {
    pub code: &'static str,
    pub stage: &'static str,
    pub message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
impl ProcessError {
    pub fn new(code: &'static str, stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            stage,
            message: message.into(),
            source: None,
        }
    }
    pub fn caused_by(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }
}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.stage, self.message)
    }
}
impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}
pub type ProcessResult<T> = Result<T, ProcessError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Cancellation,
    Deadline,
    StdoutLimit,
    StdinClosed,
    HostIo,
    PipeTimeout,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessExit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    Exited(ProcessExit),
    Stopped {
        reason: StopReason,
        forced: bool,
        exit: Option<ProcessExit>,
    },
}
#[derive(Debug, Default)]
pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub observed_bytes: u64,
}
impl CapturedOutput {
    pub fn truncated(&self) -> bool {
        self.observed_bytes > self.bytes.len() as u64
    }
}

/// Observes only the owned leader and its pipes. It does not certify that an
/// escaped descendant, a remote task or the SDK's native work has stopped.
#[derive(Debug, Default)]
pub struct ProcessCleanup {
    pub leader_reaped: bool,
    pub stdout_eof: bool,
    pub stderr_eof: bool,
    pub group_kill_attempted: bool,
    pub signal_failed: bool,
}
impl ProcessCleanup {
    pub fn locally_complete(&self) -> bool {
        self.leader_reaped && self.stdout_eof && self.stderr_eof && !self.signal_failed
    }
}
#[derive(Debug)]
pub struct ProcessReport {
    pub termination: Termination,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub stdin_written: usize,
    pub cleanup: ProcessCleanup,
    pub diagnostic: Option<ProcessError>,
}

/// Retain this value while pending_cleanup exists. Dropping a pending handle is
/// only best-effort; it is never evidence that cleanup succeeded.
#[must_use]
pub struct ProcessRun {
    pub report: ProcessReport,
    pub pending_cleanup: Option<Box<dyn PendingProcess>>,
}
impl ProcessRun {
    pub fn poll_cleanup(&mut self) -> ProcessResult<bool> {
        if let Some(pending) = &mut self.pending_cleanup
            && pending.poll_reaped()?
        {
            self.report.cleanup.leader_reaped = true;
            self.pending_cleanup.take();
        }
        Ok(self.pending_cleanup.is_none())
    }
    fn eligible(&self) -> bool {
        self.pending_cleanup.is_none()
            && self.report.diagnostic.is_none()
            && self.report.cleanup.locally_complete()
            && self.report.termination
                == Termination::Exited(ProcessExit {
                    code: Some(0),
                    signal: None,
                })
    }
}

const READY: u8 = 0;
const RUNNING: u8 = 1;
const CANCEL_BEFORE_START: u8 = 2;
const CANCEL_RUNNING: u8 = 3;
const FINISHED: u8 = 4;

/// A single-use admission gate. The atomic transition orders cancellation and
/// completion without holding a lock while running an external process.
#[derive(Clone, Default)]
pub struct ExecutionControl {
    state: Arc<AtomicU8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelDecision {
    Requested,
    AlreadyRequested,
    AlreadyFinished,
}
impl ExecutionControl {
    pub fn cancel(&self) -> CancelDecision {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let next = match state {
                READY => CANCEL_BEFORE_START,
                RUNNING => CANCEL_RUNNING,
                CANCEL_BEFORE_START | CANCEL_RUNNING => return CancelDecision::AlreadyRequested,
                _ => return CancelDecision::AlreadyFinished,
            };
            if self
                .state
                .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return CancelDecision::Requested;
            }
        }
    }
    fn begin(&self) -> ProcessResult<bool> {
        match self
            .state
            .compare_exchange(READY, RUNNING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(true),
            Err(CANCEL_BEFORE_START)
                if self
                    .state
                    .compare_exchange(
                        CANCEL_BEFORE_START,
                        FINISHED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok() =>
            {
                Ok(false)
            }
            _ => Err(ProcessError::new(
                "EXEC001",
                "admission",
                "execution control has already been used",
            )),
        }
    }
    fn finish(&self) -> bool {
        self.state.swap(FINISHED, Ordering::AcqRel) == CANCEL_RUNNING
    }
}
pub struct CancellationProbe<'a>(&'a AtomicU8);
impl CancellationProbe<'_> {
    pub fn requested(&self) -> bool {
        self.0.load(Ordering::Acquire) == CANCEL_RUNNING
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultAdmission {
    EligibleForValidation,
    RejectedByCancellation,
    Unavailable,
}
#[must_use]
pub struct ProcessExecution {
    pub admission: ResultAdmission,
    pub process: Option<ProcessRun>,
}

pub fn execute_process(
    runner: &impl ProcessRunner,
    request: &ProcessRequest,
    control: &ExecutionControl,
) -> ProcessResult<ProcessExecution> {
    if !control.begin()? {
        return Ok(ProcessExecution {
            admission: ResultAdmission::RejectedByCancellation,
            process: None,
        });
    }
    struct Finish<'a>(Option<&'a ExecutionControl>);
    impl Drop for Finish<'_> {
        fn drop(&mut self) {
            if let Some(control) = self.0.take() {
                control.finish();
            }
        }
    }
    let mut finish = Finish(Some(control));
    let result = runner.run(request, &CancellationProbe(&control.state));
    let cancellation_won = finish.0.take().expect("active execution guard").finish();
    let process = result?;
    let admission = if cancellation_won {
        ResultAdmission::RejectedByCancellation
    } else if process.eligible() {
        ResultAdmission::EligibleForValidation
    } else {
        ResultAdmission::Unavailable
    };
    Ok(ProcessExecution {
        admission,
        process: Some(process),
    })
}
