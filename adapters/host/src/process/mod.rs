//! Approved local tools only. Process groups are not an OS security sandbox.
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod child;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod pipes;
pub mod session;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod supervise;

use crate::LocalWorkspace;
use madb_app::{
    Digest, LogicalPath,
    ports::{ProcessRunner, ReadWorkspace},
    process::{
        CancellationProbe, ProcessError, ProcessPolicy, ProcessRequest, ProcessResult, ProcessRun,
    },
};
use std::path::{Path, PathBuf};

/// The caller owns stable installation and scratch directories for the entire
/// run. Validation does not defend against a hostile same-user filesystem writer.
/// The host must be the exclusive waiter for its children: no SIGCHLD auto-reap
/// or competing waitpid(-1). Observing ECHILD diagnoses, but cannot prevent, an
/// external waiter violating ownership between an observation and a signal.
pub struct LocalProcessRunner {
    tools: LocalWorkspace,
    workspace: LocalWorkspace,
    tool_root: PathBuf,
    work_root: PathBuf,
    policy: ProcessPolicy,
}
impl LocalProcessRunner {
    pub fn new(tool_root: &Path, work_root: &Path, policy: ProcessPolicy) -> ProcessResult<Self> {
        let tool_root =
            std::fs::canonicalize(tool_root).map_err(|e| io_error("configuration", e))?;
        let work_root =
            std::fs::canonicalize(work_root).map_err(|e| io_error("configuration", e))?;
        Ok(Self {
            tools: LocalWorkspace::open(&tool_root).map_err(|e| {
                ProcessError::new("EXEC010", "configuration", "tool directory is unavailable")
                    .caused_by(e)
            })?,
            workspace: LocalWorkspace::open(&work_root).map_err(|e| {
                ProcessError::new("EXEC010", "configuration", "work directory is unavailable")
                    .caused_by(e)
            })?,
            tool_root,
            work_root,
            policy,
        })
    }
    fn verify_tool(&self, request: &ProcessRequest) -> ProcessResult<()> {
        let bytes = self
            .tools
            .read(&request.tool.path, request.tool.size as usize)
            .map_err(|e| {
                ProcessError::new("EXEC011", "tool", "cannot read the bounded tool image")
                    .caused_by(e)
            })?
            .ok_or_else(|| ProcessError::new("EXEC011", "tool", "tool file is missing"))?;
        if bytes.len() as u64 != request.tool.size
            || Digest::of_bytes(&bytes) != request.tool.digest
        {
            return Err(ProcessError::new(
                "EXEC011",
                "tool",
                "tool image differs from its locked byte identity",
            ));
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl ProcessRunner for LocalProcessRunner {
    fn run(
        &self,
        request: &ProcessRequest,
        cancellation: &CancellationProbe<'_>,
    ) -> ProcessResult<ProcessRun> {
        use std::os::unix::fs::MetadataExt;
        use subprocess::{Exec, ExecExt, Redirection};
        self.policy.validate(request)?;
        if cancellation.requested() {
            return Ok(supervise::not_started());
        }
        self.verify_tool(request)?;
        let directory = open_directory(&self.workspace, &request.working_directory)?;
        let expected = directory
            .metadata()
            .map_err(|e| io_error("work-directory", e))?;
        let path = self.work_root.join(request.working_directory.as_str());
        let check_directory = || -> ProcessResult<()> {
            let current = open_directory(&self.workspace, &request.working_directory)?
                .metadata()
                .map_err(|e| io_error("work-directory", e))?;
            let named = std::fs::metadata(&path).map_err(|e| io_error("work-directory", e))?;
            if [current, named]
                .iter()
                .any(|m| m.dev() != expected.dev() || m.ino() != expected.ino())
            {
                return Err(ProcessError::new(
                    "EXEC012",
                    "work-directory",
                    "work directory changed during execution",
                ));
            }
            Ok(())
        };
        check_directory()?;
        if cancellation.requested() {
            return Ok(supervise::not_started());
        }
        // The pinned implementation uses execve, with no ENOEXEC shell fallback.
        // detached disables library Drop waiting; OwnedChild retains sole ownership.
        let child = Exec::cmd(self.tool_root.join(request.tool.path.as_str()))
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
        let mut run = supervise::run(child, request, &self.policy, cancellation);
        // A changed image/directory invalidates even a zero exit. host::inputs
        // checks declared copies; complete tool-bundle identity is later C04 work.
        if let Err(error) = check_directory().and_then(|()| self.verify_tool(request))
            && run.report.diagnostic.is_none()
        {
            run.report.diagnostic = Some(error);
        }
        Ok(run)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl ProcessRunner for LocalProcessRunner {
    fn run(&self, _: &ProcessRequest, _: &CancellationProbe<'_>) -> ProcessResult<ProcessRun> {
        Err(ProcessError::new(
            "EXEC010",
            "platform",
            "process supervision is implemented only on Linux/macOS",
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_directory(root: &LocalWorkspace, path: &LogicalPath) -> ProcessResult<std::fs::File> {
    use rustix::fs::{Mode, OFlags, openat};
    let mut directory = root
        .root
        .try_clone()
        .map_err(|e| io_error("work-directory", e))?;
    for part in path.as_str().split('/') {
        directory = std::fs::File::from(
            openat(
                &directory,
                part,
                OFlags::RDONLY
                    | OFlags::DIRECTORY
                    | OFlags::CLOEXEC
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|e| io_error("work-directory", e.into()))?,
        );
    }
    Ok(directory)
}
fn io_error(stage: &'static str, error: std::io::Error) -> ProcessError {
    ProcessError::new("EXEC010", stage, "host process operation failed").caused_by(error)
}
