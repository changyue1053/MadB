//! Report observations are data, not execution authority. In particular there
//! is no conversion from these public observations to a CommittedProject.
use super::{AbsolutePath, VerifyLevel};
use crate::{Digest, Id};
use madb_model::Location;
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{
    CheckLevel, ComponentIdentity, FileIdentity, FileOwnership, FileReference, IdeEntry, Version,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolObservation {
    pub id: Id,
    pub version: String,
    pub sha256: Digest,
    pub host: Id,
    pub target: Id,
    pub source: String,
    pub arguments: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: Id,
    pub message: String,
    pub location: Option<Location>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cleanup {
    Complete,
    Failed,
    NotRun,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckObservation {
    pub id: Id,
    pub level: CheckLevel,
    pub target: Id,
    pub status: ExecutionStatus,
    pub requirement: Applicability,
    pub environment: Vec<ToolObservation>,
    pub command: Option<Vec<String>>,
    pub evidence_level: Option<EvidenceLevel>,
    pub cleanup: Cleanup,
    pub diagnostics: Vec<Diagnostic>,
    pub attachments: Vec<FileIdentity>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenObservation {
    pub requested: Option<Id>,
    pub status: ExecutionStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub attachments: Vec<FileIdentity>,
}
impl OpenObservation {
    pub fn skipped(requested: Option<Id>) -> Self {
        Self {
            requested,
            status: ExecutionStatus::Skipped,
            diagnostics: Vec::new(),
            attachments: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadSource {
    Bundled,
    Https { url: String },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadObservation {
    pub id: Id,
    pub version: String,
    pub source: DownloadSource,
    pub sha256: Digest,
    pub size: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedFileObservation {
    pub module: Id,
    pub path: crate::LogicalPath,
    pub ownership: FileOwnership,
    pub content: Option<(Digest, u64)>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanObservation {
    pub files: Vec<PlannedFileObservation>,
    pub task_roots: Vec<Id>,
    pub downloads: Vec<DownloadObservation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateObservation {
    pub id: Id,
    pub version: Version,
    pub manifest: FileReference,
    pub package_sha256: Digest,
    pub input_root: crate::LogicalPath,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisplayPath {
    Absolute(AbsolutePath),
    Redacted,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestObservation {
    pub targets: Vec<Id>,
    pub verify: VerifyLevel,
    pub open: Option<Id>,
    pub dry_run: bool,
    pub offline: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Creation {
    NotCreated,
    Created,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    Completed,
    Failed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryKind {
    Retry,
    Manual,
    Inspect,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    pub kind: RecoveryKind,
    pub message: String,
    pub arguments: Option<Vec<String>>,
}
/// Nullable fields preserve the actual stage reached. A report is never a
/// substitute for source binding, a Host commit receipt, or independent Lab proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitReport {
    pub request_digest: Option<Digest>,
    pub tool: ComponentIdentity,
    pub template: Option<TemplateObservation>,
    pub template_package: Option<FileIdentity>,
    pub evidence_root: Option<DisplayPath>,
    pub template_profile: Option<Id>,
    pub boundary_profiles: Vec<FileReference>,
    pub request: Option<RequestObservation>,
    pub output: Option<DisplayPath>,
    pub plan: Option<PlanObservation>,
    pub creation: Creation,
    pub termination: Termination,
    pub exit_code: u8,
    pub checks: Vec<CheckObservation>,
    pub ide_entries: Vec<IdeEntry>,
    pub open: OpenObservation,
    pub recovery: Vec<Recovery>,
    pub diagnostics: Vec<Diagnostic>,
    pub logs: Vec<FileIdentity>,
}
/// Values needed to report a fully bound successful attempt. Actual file bytes
/// are checked by the evidence port before a CompletedInitialization escapes.
#[derive(Clone, Debug)]
pub struct SuccessObservations {
    pub tool: ComponentIdentity,
    pub template: TemplateObservation,
    pub template_package: FileIdentity,
    pub evidence_root: AbsolutePath,
    pub boundary_profiles: Vec<FileReference>,
    pub checks: Vec<CheckObservation>,
    pub open: OpenObservation,
    pub diagnostics: Vec<Diagnostic>,
    pub logs: Vec<FileIdentity>,
}
