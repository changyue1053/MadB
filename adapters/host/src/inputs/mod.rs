//! Private copies of declared inputs. Retention is deliberate: process-group
//! observations alone cannot prove that escaped descendants stopped writing.
#[cfg(unix)]
mod tree;

use crate::process::LocalProcessRunner;
use madb_app::{
    ErrorKind, LogicalPath, QueryError,
    inputs::FrozenInputs,
    ports::ProcessRunner,
    process::{
        CancellationProbe, ExecutionControl, ProcessError, ProcessExecution, ProcessPolicy,
        ProcessRequest, ProcessResult, ProcessRun, execute_process,
    },
};
use std::path::{Path, PathBuf};

/// An error after allocation carries the path of the newly owned partial tree.
/// None means no directory was allocated; existing paths never become ours.
#[derive(Debug)]
pub struct PreparationError {
    pub error: QueryError,
    pub retained_path: Option<PathBuf>,
}
impl std::fmt::Display for PreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for PreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The caller grants a stable parent directory and owns its retention policy.
/// This value is deliberately not Clone. Drop closes descriptors, never deletes
/// files. Read-only modes deter mistakes; this is not a same-user sandbox.
#[must_use]
pub struct PreparedRun {
    path: PathBuf,
    #[cfg(unix)]
    root: std::fs::File,
    snapshot: FrozenInputs,
}
impl PreparedRun {
    pub fn create(
        parent: &Path,
        name: &LogicalPath,
        snapshot: FrozenInputs,
    ) -> Result<Self, PreparationError> {
        if !name.as_str().is_ascii() || name.as_str().contains('/') {
            return Err(PreparationError {
                error: preparation_error("name", "run name must be one ASCII path component"),
                retained_path: None,
            });
        }
        Self::allocate(parent, name, snapshot)
    }

    #[cfg(unix)]
    fn allocate(
        parent: &Path,
        name: &LogicalPath,
        snapshot: FrozenInputs,
    ) -> Result<Self, PreparationError> {
        let before_allocation = |error| PreparationError {
            error,
            retained_path: None,
        };
        let parent = std::fs::canonicalize(parent)
            .map_err(|e| before_allocation(preparation_error("parent", e)))?;
        let parent_directory = crate::LocalWorkspace::open(&parent).map_err(before_allocation)?;
        rustix::fs::mkdirat(&parent_directory.root, name.as_str(), tree::PRIVATE_DIR)
            .map_err(|e| before_allocation(preparation_error("name", e)))?;
        let path = parent.join(name.as_str());
        let after_allocation = |error| PreparationError {
            error,
            retained_path: Some(path.clone()),
        };
        let root = tree::open_directory(&parent_directory.root, name.as_str())
            .map_err(after_allocation)?;
        tree::materialize(&root, &snapshot).map_err(after_allocation)?;
        let prepared = Self {
            path,
            root,
            snapshot,
        };
        prepared.verify().map_err(|error| PreparationError {
            error,
            retained_path: Some(prepared.path.clone()),
        })?;
        Ok(prepared)
    }

    #[cfg(not(unix))]
    fn allocate(_: &Path, _: &LogicalPath, _: FrozenInputs) -> Result<Self, PreparationError> {
        Err(PreparationError {
            error: preparation_error("platform", "private input trees require a Unix host"),
            retained_path: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(unix)]
    fn verify(&self) -> madb_app::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let named =
            std::fs::symlink_metadata(&self.path).map_err(|e| tree::integrity_error("run", e))?;
        let owned = self
            .root
            .metadata()
            .map_err(|e| tree::integrity_error("run", e))?;
        if !named.is_dir() || named.dev() != owned.dev() || named.ino() != owned.ino() {
            return Err(tree::integrity_error("run", "run directory was replaced"));
        }
        tree::verify(&self.root, &self.snapshot)
    }

    #[cfg(not(unix))]
    fn verify(&self) -> madb_app::Result<()> {
        let _ = &self.snapshot;
        Err(preparation_error(
            "platform",
            "private input trees require a Unix host",
        ))
    }

    /// Arguments explicitly select ../inputs/... from work/. No argument
    /// rewriting or restriction on arbitrary tool filesystem/network access.
    /// Verification happens before final cancellation/result admission.
    pub fn execute(
        self,
        tool_root: &Path,
        policy: ProcessPolicy,
        request: &ProcessRequest,
        control: &ExecutionControl,
    ) -> RetainedRun {
        let execution = execute_process(
            &CheckedRunner {
                prepared: &self,
                tool_root,
                policy,
            },
            request,
            control,
        );
        RetainedRun {
            prepared: self,
            execution,
        }
    }
}

struct CheckedRunner<'a> {
    prepared: &'a PreparedRun,
    tool_root: &'a Path,
    policy: ProcessPolicy,
}
impl ProcessRunner for CheckedRunner<'_> {
    fn run(
        &self,
        request: &ProcessRequest,
        cancellation: &CancellationProbe<'_>,
    ) -> ProcessResult<ProcessRun> {
        if request.working_directory.as_str() != "work" {
            return Err(ProcessError::new(
                "EXEC002",
                "work-directory",
                "prepared runs must use work/",
            ));
        }
        self.prepared.verify().map_err(input_error)?;
        let runner =
            LocalProcessRunner::new(self.tool_root, self.prepared.path(), self.policy.clone())?;
        let mut run = runner.run(request, cancellation)?;
        // Preserve the child owner and observations even when validation fails.
        if let Err(error) = self.prepared.verify()
            && run.report.diagnostic.is_none()
        {
            run.report.diagnostic = Some(input_error(error));
        }
        Ok(run)
    }
}

/// A retained tree plus its execution outcome, including any pending child.
/// Neither a successful poll nor Drop authorizes filesystem removal.
#[must_use]
pub struct RetainedRun {
    prepared: PreparedRun,
    execution: ProcessResult<ProcessExecution>,
}
impl RetainedRun {
    pub fn path(&self) -> &Path {
        self.prepared.path()
    }
    pub fn execution(&self) -> &ProcessResult<ProcessExecution> {
        &self.execution
    }
    /// True means no pending owned leader, not that the tree is safe to delete.
    /// Late cleanup never upgrades result admission.
    pub fn poll_cleanup(&mut self) -> ProcessResult<bool> {
        if let Ok(execution) = &mut self.execution
            && let Some(process) = &mut execution.process
        {
            return process.poll_cleanup();
        }
        Ok(true)
    }
}

fn preparation_error(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "INPUT005", subject, error.to_string())
}
fn input_error(error: QueryError) -> ProcessError {
    ProcessError::new(
        "EXEC017",
        "frozen-inputs",
        "private input copy failed verification",
    )
    .caused_by(error)
}
