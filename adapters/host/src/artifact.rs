//! Controlled execution for the real reader/generator/packager port payloads.
use crate::inputs::{PreparationError, PreparedRun, RetainedRun};
use madb_app::{
    ErrorKind, LogicalPath, QueryError,
    inputs::FrozenInputs,
    pipeline::{
        ArtifactExecutor, ArtifactNodeRun, ArtifactRecipe, ArtifactResult, capture_artifact_result,
    },
    process::{ExecutionControl, ProcessError, ProcessExecution, ProcessPolicy, ProcessResult},
};
use std::path::{Path, PathBuf};

pub struct LocalArtifactExecutor {
    parent: PathBuf,
    name: LogicalPath,
    tool_root: PathBuf,
    policy: ProcessPolicy,
    used: bool,
}
impl LocalArtifactExecutor {
    pub fn new(
        parent: &Path,
        name: &LogicalPath,
        tool_root: &Path,
        policy: ProcessPolicy,
    ) -> Result<Self, PreparationError> {
        if !name.as_str().is_ascii() || name.as_str().contains('/') {
            return Err(PreparationError {
                error: QueryError::new(
                    ErrorKind::InvalidInput,
                    "PIPE001",
                    "run-name",
                    "run name must be one ASCII path component",
                ),
                retained_path: None,
            });
        }
        Ok(Self {
            parent: parent.to_owned(),
            name: name.clone(),
            tool_root: tool_root.to_owned(),
            policy,
            used: false,
        })
    }
}
impl<R: ArtifactRecipe> ArtifactExecutor<R> for LocalArtifactExecutor {
    type Run = LocalArtifactRun;
    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> Self::Run {
        let prepared = (|| {
            if self.used {
                return Err(PreparationError {
                    error: QueryError::new(
                        ErrorKind::Conflict,
                        "PIPE001",
                        "executor",
                        "artifact executor already used",
                    ),
                    retained_path: None,
                });
            }
            self.used = true;
            recipe
                .invocation()
                .validate()
                .map_err(|error| PreparationError {
                    error,
                    retained_path: None,
                })?;
            let expected = &recipe.invocation().inputs;
            if inputs.references().count() != expected.len()
                || !inputs.references().all(|r| expected.contains(r))
            {
                return Err(PreparationError {
                    error: QueryError::new(
                        ErrorKind::InvalidInput,
                        "PIPE001",
                        "inputs",
                        "fixed artifact inputs differ from bound invocation",
                    ),
                    retained_path: None,
                });
            }
            PreparedRun::create(&self.parent, &self.name, inputs)
        })();
        LocalArtifactRun {
            outcome: prepared.map(|prepared| {
                prepared.execute_validated(
                    &self.tool_root,
                    self.policy.clone(),
                    &recipe.invocation().process,
                    control,
                    |workspace, run, _| {
                        let result = capture_artifact_result(
                            workspace,
                            recipe,
                            &run.report.stdout.bytes,
                            result_limit,
                        )
                        .map_err(result_error)?;
                        verify(workspace, result.files()).map_err(result_error)?;
                        Ok(result)
                    },
                )
            }),
        }
    }
}
#[must_use]
pub struct LocalArtifactRun {
    outcome: Result<RetainedRun<ArtifactResult>, PreparationError>,
}
impl LocalArtifactRun {
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
    pub fn poll_cleanup(&mut self) -> ProcessResult<bool> {
        match &mut self.outcome {
            Ok(run) => run.poll_cleanup(),
            Err(_) => Ok(true),
        }
    }
}
impl ArtifactNodeRun for LocalArtifactRun {
    fn take_value(&mut self) -> Option<ArtifactResult> {
        self.outcome.as_mut().ok().and_then(RetainedRun::take_value)
    }
}
#[cfg(unix)]
fn verify(workspace: &crate::LocalWorkspace, files: &FrozenInputs) -> madb_app::Result<()> {
    crate::inputs::tree::verify_contents(workspace, files)
}
#[cfg(not(unix))]
fn verify(_: &crate::LocalWorkspace, _: &FrozenInputs) -> madb_app::Result<()> {
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST004",
        "platform",
        "artifact tree verification requires Unix",
    ))
}
fn result_error(error: QueryError) -> ProcessError {
    ProcessError::new(
        "EXEC019",
        "artifact-result",
        "artifact payload or file verification failed",
    )
    .caused_by(error)
}
