//! Host capabilities for the one approved author-project recipe. A public
//! BuildAttempt or result is an observation, never authority to adopt a path.
#[cfg(unix)]
mod attempt;
#[cfg(unix)]
mod capture;
#[cfg(unix)]
mod fs;
#[cfg(unix)]
mod install;
#[cfg(unix)]
mod publish;
#[cfg(unix)]
mod records;
#[cfg(unix)]
pub use capture::LocalProjectBuild;
#[cfg(not(unix))]
mod unsupported;
#[cfg(unix)]
pub use records::origin_bytes;
#[cfg(not(unix))]
pub use unsupported::{LocalProjectBuild, origin_bytes};
#[cfg(all(test, unix))]
mod tests;

#[cfg(unix)]
use madb_app::{Digest, ErrorKind, LogicalPath, QueryError, Result, use_cases::project_build::*};
#[cfg(unix)]
use std::{collections::BTreeMap, fs::File};

#[cfg(unix)]
fn invalid(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
#[cfg(unix)]
fn conflict(subject: impl Into<String>, message: impl ToString) -> QueryError {
    QueryError::new(
        ErrorKind::Conflict,
        "STORE003",
        subject,
        message.to_string(),
    )
}
#[cfg(unix)]
fn host(subject: impl Into<String>, message: impl ToString) -> QueryError {
    QueryError::new(ErrorKind::Host, "HOST003", subject, message.to_string())
}
#[cfg(unix)]
fn logical(value: &str) -> Result<LogicalPath> {
    LogicalPath::new(value).map_err(|e| invalid(value, e.to_string()))
}
