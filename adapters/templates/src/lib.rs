//! Pure, bounded template bytes and rendering. This adapter cannot obtain or
//! extract packages, run tools, or commit a user's workspace.
mod archive;
mod render;

pub use archive::{ArchiveEntry, CheckedArchive, CheckedInventory, parse_archive};
pub use render::{render_asset, render_bytes};

// Re-export only the inward-facing values used by this adapter's signatures;
// their definitions and validation remain in the model/plan owners.
pub use madb_model::{Digest, Id, LogicalPath};
pub use madb_plan::project::{
    FileIdentity, FileOwnership, IdentifierLanguage, JsonBinding, ParameterValue, Renderer,
    TemplateAsset,
};

/// Local adapter failures are translated by the application at the actual
/// stage; a frozen-input conflict must not be inferred from this enum alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetErrorKind {
    ArchiveIdentity,
    ArchiveFormat,
    Inventory,
    Render,
    DocumentValidation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetError {
    pub kind: AssetErrorKind,
    pub subject: String,
    pub message: String,
}

impl AssetError {
    fn new(kind: AssetErrorKind, subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            subject: subject.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}: {}", self.subject, self.message)
    }
}

impl std::error::Error for AssetError {}
