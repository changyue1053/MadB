//! Pure binding of execution bundles and genuine metadata/generation/package ports.
mod artifact;
mod build;
mod dependencies;
mod files;
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

use madb_app::{Digest, ErrorKind, Id, QueryError, Result};
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
