//! One explicit task recipe and its captured result. Neither represents a
//! committed generation, a complete dependency closure or SDK certification.
use std::collections::BTreeSet;

use crate::{
    ErrorKind, Id, LogicalPath, QueryError, Result,
    inputs::{FrozenInputs, capture_inputs, validate_inventory, validate_paths},
    ports::{FileReference, ReadWorkspace},
    process::ProcessRequest,
};

/// Wire-independent invocation. Adapters bind policy/context and tool identity;
/// the host must call validate before creating a run directory or process.
pub struct TaskInvocation {
    pub id: Id,
    pub process: ProcessRequest,
    pub inputs: Vec<FileReference>,
    pub outputs: Vec<LogicalPath>,
}
impl TaskInvocation {
    pub fn validate(&self) -> Result<()> {
        if self.process.working_directory.as_str() != "work" {
            return Err(task_error(
                "TASK001",
                "working_directory",
                "task cwd must be work",
            ));
        }
        validate_inventory(&self.inputs)?;
        validate_paths(&self.outputs)
            .map_err(|error| task_error("TASK001", "outputs", error.to_string()))?;
        Ok(())
    }
}

/// References parsed from a checked result payload. Actual files remain
/// untrusted until the host applies confinement and this module captures bytes.
#[derive(Clone, Debug)]
pub struct TaskFileReferences {
    pub outputs: Vec<FileReference>,
    pub logs: Vec<FileReference>,
}

/// Adapters own wire parsing, schema and request/result identity checks. They
/// do not read paths or decide whether a process actually succeeded. The
/// invocation must stay unchanged from host preparation through execution and
/// result validation; implementations should own an immutable bound invocation.
pub trait BuildRecipe {
    fn invocation(&self) -> &TaskInvocation;
    /// Direct task identities. Single-task recipes have no dependencies.
    fn dependencies(&self) -> &[Id] {
        &[]
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<TaskFileReferences>;
}

/// Immutable captured bytes, not a promise that the originating work tree
/// remains unchanged. Outputs and logs retain distinct logical inventories.
#[derive(Clone, Debug)]
pub struct ValidatedTaskResult {
    files: FrozenInputs,
    payload: std::sync::Arc<[u8]>,
    outputs: BTreeSet<LogicalPath>,
    logs: BTreeSet<LogicalPath>,
}
impl ValidatedTaskResult {
    /// Exact validated stdout, retained for real build-evidence publication.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn outputs(&self) -> impl Iterator<Item = (&FileReference, &[u8])> {
        self.files
            .files()
            .filter(|(reference, _)| self.outputs.contains(&reference.path))
    }
    pub fn logs(&self) -> impl Iterator<Item = (&FileReference, &[u8])> {
        self.files
            .files()
            .filter(|(reference, _)| self.logs.contains(&reference.path))
    }
}

/// Validate the payload, require the exact planned output set plus nonempty
/// logs, and capture both under one file/byte/path budget. The workspace port
/// must enforce filesystem confinement; exact tree enumeration belongs to host.
pub fn capture_task_result<W: ReadWorkspace, R: BuildRecipe>(
    workspace: &W,
    recipe: &R,
    stdout: &[u8],
) -> Result<ValidatedTaskResult> {
    capture_task_result_limited(workspace, recipe, stdout, usize::MAX)
}

/// Apply the caller's remaining graph budget before any result file is read.
/// The ordinary per-task inventory limits still apply independently.
pub fn capture_task_result_limited<W: ReadWorkspace, R: BuildRecipe>(
    workspace: &W,
    recipe: &R,
    stdout: &[u8],
    limit: usize,
) -> Result<ValidatedTaskResult> {
    recipe.invocation().validate()?;
    let references = recipe.validate_result(stdout)?;
    let outputs: BTreeSet<_> = references.outputs.iter().map(|f| f.path.clone()).collect();
    let logs: BTreeSet<_> = references.logs.iter().map(|f| f.path.clone()).collect();
    let expected: BTreeSet<_> = recipe.invocation().outputs.iter().cloned().collect();
    if outputs != expected || outputs.len() != references.outputs.len() {
        return Err(task_error(
            "TASK002",
            "outputs",
            "result must contain exactly the declared outputs",
        ));
    }
    if logs.is_empty() || logs.len() != references.logs.len() || !outputs.is_disjoint(&logs) {
        return Err(task_error(
            "TASK002",
            "logs",
            "logs must be nonempty, unique and separate from outputs",
        ));
    }
    let references: Vec<_> = references
        .outputs
        .into_iter()
        .chain(references.logs)
        .collect();
    let (_, total_bytes) = validate_inventory(&references)?;
    if total_bytes > limit {
        return Err(task_error(
            "TASK003",
            "result",
            "result exceeds remaining graph byte budget",
        ));
    }
    let files = capture_inputs(workspace, references)?;
    Ok(ValidatedTaskResult {
        payload: stdout.into(),
        files,
        outputs,
        logs,
    })
}

fn task_error(code: &'static str, subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}
