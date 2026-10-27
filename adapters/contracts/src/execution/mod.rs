//! Pure binding of execution bundles and genuine metadata/generation/package ports.
mod artifact;
pub(crate) use artifact::validate_sources;
mod build;
mod dependencies;
pub(crate) mod files;
mod package;
mod spec;

pub use artifact::BoundArtifactRecipe;
pub use build::{
    BuildArtifacts, assemble_build_files, build_environment_names, build_input_references,
    selected_task_documents, validate_build_generation,
};
pub use dependencies::{validate_dependency_target, validate_dependency_tool};
pub use files::source_manifest;
pub use package::validate_package_parents;
pub use spec::{
    DependencyFiles, ExecutionContext, ExecutionOperation, ExecutionRecipe, ExecutionSpec,
    ExecutionTask, ExecutionTool, ExecutionWatch, generation_task_digest, read_execution,
};

use madb_app::{Digest, ErrorKind, Id, Phase, QueryError, Result};

/// Decode and check feature support; callers bind these profile bytes through
/// their existing checked configuration before executing any task.
pub fn build_completion_policy(profile: &[u8]) -> Result<madb_model::build::BuildCompletionPolicy> {
    let adapter = crate::query::SnapshotAdapter::new()?;
    let document = adapter.decode(crate::DocumentKind::Profile, profile)?;
    crate::build_completion_policy(&document).map_err(crate::query::format_error)
}

/// Dependency discovery is an early helper path: reject forbidden phases in
/// the whole request without binding inputs or requiring its pending receipt.
pub fn validate_build_phase_admission(profile: &[u8], tasks: &[Vec<u8>]) -> Result<()> {
    let policy = build_completion_policy(profile)?;
    let adapter = crate::query::SnapshotAdapter::new()?;
    for bytes in tasks {
        let task = adapter.decode(crate::DocumentKind::BuildTask, bytes)?;
        if !policy.allows(build_phase(&task.value()["phase"])?) {
            return Err(invalid(
                "EXECUTION003",
                "phase",
                "selected build policy forbids this phase before dependency preparation",
            ));
        }
    }
    Ok(())
}

fn build_phase(value: &madb_protocol::Value) -> Result<Phase> {
    match value.as_str() {
        Some("metadata") => Ok(Phase::Metadata),
        Some("generate") => Ok(Phase::Generate),
        Some("compile") => Ok(Phase::Compile),
        Some("link") => Ok(Phase::Link),
        Some("run") => Ok(Phase::Run),
        _ => Err(invalid("EXECUTION003", "phase", "unknown build phase")),
    }
}
use madb_protocol::Value;

fn id(value: &Value) -> Result<Id> {
    Id::new(
        value
            .as_str()
            .ok_or_else(|| invalid("DTO001", "id", "expected id"))?,
    )
    .map_err(|e| invalid("DTO002", "id", e.to_string()))
}
fn digest(value: &Value) -> Result<Digest> {
    Digest::new(
        value
            .as_str()
            .ok_or_else(|| invalid("DTO001", "digest", "expected digest"))?,
    )
    .map_err(|e| invalid("DTO002", "digest", e.to_string()))
}
fn invalid(code: &'static str, subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}

pub(crate) use build::assemble_build_files_checked;
pub(crate) use dependencies::bind_tool_values;
pub(crate) use package::{PackageContext, validate_package_files};
