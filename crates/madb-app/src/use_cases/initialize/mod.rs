//! Initialization facts and result reduction. These APIs do not implement a Host
//! filesystem, publish a template, or turn a decoded report into commit authority.
mod files;
mod request;
mod result;
pub mod types;

pub use files::*;
pub use madb_plan::project::{ComponentIdentity, FileIdentity, FileOwnership, Version};
pub use request::*;
pub use result::*;
pub use types::*;

use crate::{ErrorKind, QueryError};
fn invalid(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
fn evidence(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::CheckFailed, "EVD001", subject, message)
}
