//! Schema-governed, bounded wire decoding. A valid document is not a certified SDK.
mod canonical;
mod decode;
mod schema;

pub use canonical::{DigestDomain, byte_digest, canonical_json, semantic_digest};
pub use decode::{MAX_DOCUMENT_BYTES, MAX_DOCUMENT_DEPTH, strict_json};
pub use schema::{Document, DocumentKind, SchemaRegistry};
pub use serde_json::Value;

/// A stable format diagnostic; semantic findings belong to the model/rules layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatError {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

impl FormatError {
    pub fn new(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{} at {}: {}", self.code, self.path, self.message)
    }
}
impl std::error::Error for FormatError {}
