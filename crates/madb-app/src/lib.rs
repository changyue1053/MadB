//! Application use cases and inward-facing ports. No filesystem or wire DTOs.
pub mod dag;
mod dag_control;
pub mod dependencies;
pub mod explain;
pub mod extension;
pub mod input_observation;
pub mod inputs;
pub mod pipeline;
pub mod ports;
pub mod process;
pub mod query;
pub mod task;
pub mod transaction;

pub use madb_model::{Digest, Finding, Id, LogicalPath};
pub use madb_plan::{Phase, Task, TaskGraph};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidInput,
    CheckFailed,
    Conflict,
    Host,
}

#[derive(Debug)]
pub struct QueryError {
    pub kind: ErrorKind,
    pub findings: Vec<Finding>,
}
impl QueryError {
    pub fn new(
        kind: ErrorKind,
        code: &'static str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            findings: vec![Finding::new(code, subject, message)],
        }
    }
}
impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for finding in &self.findings {
            writeln!(
                f,
                "{} at {}: {}",
                finding.code, finding.subject, finding.message
            )?;
            for location in &finding.locations {
                writeln!(
                    f,
                    "  source {}:{}:{}",
                    location.path.as_str(),
                    location.line,
                    location.column
                )?;
            }
        }
        Ok(())
    }
}
impl std::error::Error for QueryError {}

pub type Result<T> = std::result::Result<T, QueryError>;
