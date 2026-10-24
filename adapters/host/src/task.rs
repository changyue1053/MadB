//! Execute an explicitly bound single-task recipe, retaining fixed result bytes.
use crate::{
    LocalWorkspace,
    inputs::{PreparationError, PreparedRun, RetainedRun},
};
use madb_app::{
    LogicalPath, QueryError,
    inputs::{FrozenInputs, capture_inputs},
    ports::ReadWorkspace,
    process::{ExecutionControl, ProcessError, ProcessPolicy},
    task::{BuildRecipe, ValidatedTaskResult, capture_task_result_limited},
};
use std::path::Path;

/// Preparation performs no process execution. A recipe must expose a stable
/// invocation throughout preparation and execution; concrete bindings own it.
#[must_use]
pub struct PreparedTask<'a, R> {
    prepared: PreparedRun,
    recipe: &'a R,
}
impl<'a, R: BuildRecipe> PreparedTask<'a, R> {
    pub fn create<W: ReadWorkspace>(
        parent: &Path,
        name: &LogicalPath,
        source: &W,
        recipe: &'a R,
    ) -> Result<Self, PreparationError> {
        let before_allocation = |error| PreparationError {
            error,
            retained_path: None,
        };
        if !recipe.dependencies().is_empty() {
            return Err(before_allocation(QueryError::new(
                madb_app::ErrorKind::InvalidInput,
                "TASK001",
                "dependencies",
                "tasks with dependencies require graph execution",
            )));
        }
        recipe.invocation().validate().map_err(before_allocation)?;
        let snapshot = capture_inputs(source, recipe.invocation().inputs.clone())
            .map_err(before_allocation)?;
        Self::from_frozen(parent, name, snapshot, recipe)
    }
    /// Only the graph executor may supply already captured dependency inputs.
    /// This is deliberately not a second public bypass of dependency admission.
    pub(crate) fn from_frozen(
        parent: &Path,
        name: &LogicalPath,
        snapshot: FrozenInputs,
        recipe: &'a R,
    ) -> Result<Self, PreparationError> {
        recipe
            .invocation()
            .validate()
            .map_err(|error| PreparationError {
                error,
                retained_path: None,
            })?;
        let expected = &recipe.invocation().inputs;
        if snapshot.references().count() != expected.len()
            || !snapshot
                .references()
                .all(|reference| expected.contains(reference))
        {
            return Err(PreparationError {
                error: QueryError::new(
                    madb_app::ErrorKind::InvalidInput,
                    "TASK001",
                    "inputs",
                    "frozen input inventory must match the bound invocation",
                ),
                retained_path: None,
            });
        }
        Ok(Self {
            prepared: PreparedRun::create(parent, name, snapshot)?,
            recipe,
        })
    }
    pub fn path(&self) -> &Path {
        self.prepared.path()
    }
    /// A successful captured value is an observation, not a committed batch or
    /// an SDK certification. Disk contents may subsequently change; these bytes
    /// do not. No output is exposed when cancellation wins admission.
    pub fn execute(
        self,
        tool_root: &Path,
        policy: ProcessPolicy,
        control: &ExecutionControl,
    ) -> RetainedRun<ValidatedTaskResult> {
        self.execute_limited(tool_root, policy, control, 128 * 1024 * 1024)
    }
    pub(crate) fn execute_limited(
        self,
        tool_root: &Path,
        policy: ProcessPolicy,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> RetainedRun<ValidatedTaskResult> {
        self.prepared.execute_validated(
            tool_root,
            policy,
            &self.recipe.invocation().process,
            control,
            |workspace, run, _| {
                let result = capture_task_result_limited(
                    workspace,
                    self.recipe,
                    &run.report.stdout.bytes,
                    result_limit,
                )
                .map_err(result_error)?;
                verify_outputs(workspace, result.files()).map_err(result_error)?;
                Ok(result)
            },
        )
    }
}

#[cfg(unix)]
fn verify_outputs(workspace: &LocalWorkspace, files: &FrozenInputs) -> madb_app::Result<()> {
    crate::inputs::tree::verify_contents(workspace, files)
}
#[cfg(not(unix))]
fn verify_outputs(_: &LocalWorkspace, _: &FrozenInputs) -> madb_app::Result<()> {
    Err(QueryError::new(
        madb_app::ErrorKind::Host,
        "HOST004",
        "platform",
        "task output checks require Unix",
    ))
}
fn result_error(error: QueryError) -> ProcessError {
    ProcessError::new(
        "EXEC018",
        "task-result",
        "task payload or files failed validation",
    )
    .caused_by(error)
}
