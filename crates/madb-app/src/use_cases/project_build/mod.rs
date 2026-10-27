//! Finite author-project rebuild values and inward-facing ports. Wire reports
//! contain observations only; they never become a filesystem or process permit.
mod inputs;
mod ports;
mod types;
mod workflow;
pub use inputs::*;
pub use ports::*;
pub use types::*;
pub use workflow::*;

pub const RECIPE: &str = "madb.project.rust-c-values@1";
pub const DESCRIPTOR: &str = ".madb/project-build.json";
pub const ORIGIN: &str = ".madb/project-origin.json";
pub const MAX_WIRE_BYTES: usize = 1024 * 1024;
pub const MAX_SECTION_BYTES: usize = 128 * 1024;

pub(crate) fn invalid(subject: impl Into<String>, message: impl Into<String>) -> crate::QueryError {
    crate::QueryError::new(crate::ErrorKind::InvalidInput, "PLAN001", subject, message)
}
