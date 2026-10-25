//! Boundary values independent of files, compilers, serialization and runtime IO.
pub mod build;
pub mod contract;
pub mod evidence;
pub mod identity;
pub mod policy;

pub use identity::{Digest, Id, Location, LogicalPath, PublicId, SourceId, SourceKey, ValueError};

/// A semantic diagnostic always identifies the affected subject and sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub code: &'static str,
    pub subject: String,
    pub locations: Vec<Location>,
    pub message: String,
}
impl Finding {
    pub fn new(code: &'static str, subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            subject: subject.into(),
            locations: Vec::new(),
            message: message.into(),
        }
    }
    pub fn at(mut self, location: &Location) -> Self {
        self.locations.push(location.clone());
        self
    }
}
