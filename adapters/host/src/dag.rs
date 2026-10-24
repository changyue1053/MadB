//! Retained serial graph runs. Scheduling and result admission remain in app;
//! this adapter only owns private trees, concrete processes and their cleanup.
use crate::{
    inputs::{PreparationError, RetainedRun},
    task::PreparedTask,
};
use madb_app::{
    ErrorKind, LogicalPath, QueryError,
    dag::{GraphExecutor, GraphRecipe, NodeRun},
    inputs::FrozenInputs,
    process::{ExecutionControl, ProcessExecution, ProcessPolicy, ProcessResult},
    task::ValidatedTaskResult,
};
use std::path::{Path, PathBuf};

/// A fresh private graph directory under a caller-granted stable parent.
/// Dropping this owner never removes directories. Each node uses an index, so
/// a task's logical ID cannot select a filesystem location.
#[must_use]
pub struct LocalGraphExecutor {
    path: PathBuf,
    #[cfg(unix)]
    root: std::fs::File,
    tool_root: PathBuf,
    policy: ProcessPolicy,
    next_index: usize,
}
impl LocalGraphExecutor {
    #[cfg(unix)]
    pub fn new(
        parent: &Path,
        name: &LogicalPath,
        tool_root: &Path,
        policy: ProcessPolicy,
    ) -> Result<Self, PreparationError> {
        let (path, root) = crate::inputs::allocate_directory(parent, name)?;
        Ok(Self {
            path,
            root,
            tool_root: tool_root.to_owned(),
            policy,
            next_index: 0,
        })
    }
    #[cfg(not(unix))]
    pub fn new(
        _: &Path,
        _: &LogicalPath,
        _: &Path,
        _: ProcessPolicy,
    ) -> Result<Self, PreparationError> {
        Err(PreparationError {
            error: QueryError::new(
                ErrorKind::Host,
                "HOST004",
                "platform",
                "private graph directories require a Unix host",
            ),
            retained_path: None,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[cfg(unix)]
    fn verify(&self) -> madb_app::Result<()> {
        crate::inputs::verify_directory(&self.path, &self.root)
    }
    #[cfg(not(unix))]
    fn verify(&self) -> madb_app::Result<()> {
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "platform",
            "private graph directories require a Unix host",
        ))
    }
}

// Direct calls to this low-level port produce a node observation only.
// Whole-graph acceptance is exclusively decided by app::dag::execute_graph.
impl<R: GraphRecipe> GraphExecutor<R> for LocalGraphExecutor {
    type Run = LocalNodeRun;

    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> Self::Run {
        let prepared = (|| {
            self.verify().map_err(|error| PreparationError {
                error,
                retained_path: None,
            })?;
            let index = self.next_index;
            self.next_index = index.checked_add(1).ok_or_else(|| PreparationError {
                error: QueryError::new(
                    ErrorKind::Host,
                    "TASK001",
                    "node-index",
                    "node directory index exhausted",
                ),
                retained_path: None,
            })?;
            let name = LogicalPath::new(format!("node-{index:04}"))
                .expect("generated ASCII node directory name");
            let prepared = PreparedTask::from_frozen(&self.path, &name, inputs, recipe)?;
            self.verify().map_err(|error| PreparationError {
                error,
                retained_path: Some(prepared.path().to_owned()),
            })?;
            Ok(prepared)
        })();
        LocalNodeRun {
            outcome: prepared.map(|prepared| {
                prepared.execute_limited(
                    &self.tool_root,
                    self.policy.clone(),
                    control,
                    result_limit,
                )
            }),
        }
    }
}

/// The scheduler drains the captured value into its private result map before
/// returning this owner. Public access is limited to raw process observations,
/// preparation errors, retained paths and explicit cleanup polling.
#[must_use]
pub struct LocalNodeRun {
    outcome: Result<RetainedRun<ValidatedTaskResult>, PreparationError>,
}
impl NodeRun for LocalNodeRun {
    fn take_value(&mut self) -> Option<ValidatedTaskResult> {
        self.outcome.as_mut().ok().and_then(RetainedRun::take_value)
    }
}
impl LocalNodeRun {
    pub fn path(&self) -> Option<&Path> {
        match &self.outcome {
            Ok(run) => Some(run.path()),
            Err(error) => error.retained_path.as_deref(),
        }
    }
    pub fn execution(&self) -> Option<&ProcessResult<ProcessExecution>> {
        self.outcome.as_ref().ok().map(RetainedRun::execution)
    }
    pub fn preparation_error(&self) -> Option<&PreparationError> {
        self.outcome.as_ref().err()
    }
    pub fn poll_cleanup(&mut self) -> Option<ProcessResult<bool>> {
        self.outcome.as_mut().ok().map(RetainedRun::poll_cleanup)
    }
}
