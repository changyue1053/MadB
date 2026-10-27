pub use crate::use_cases::initialize::{AbsolutePath, Diagnostic, FileIdentity};
use crate::{Digest, Id, LogicalPath, QueryError};
pub use madb_plan::project::FileReference;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Build,
    InstallTool,
}
impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::InstallTool => "install-tool",
        }
    }
    pub fn stages(self) -> &'static [StageId] {
        match self {
            Self::Build => &[
                StageId::Capture,
                StageId::Preflight,
                StageId::Prepare,
                StageId::Metadata,
                StageId::Generate,
                StageId::Build,
                StageId::Package,
                StageId::ConsumerLink,
                StageId::Publish,
            ],
            Self::InstallTool => &[StageId::Capture, StageId::Preflight, StageId::Install],
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolIdentity {
    pub id: Id,
    pub version: String,
    pub sha256: Digest,
    pub size: u64,
    pub source_revision: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateIdentity {
    pub id: Id,
    pub version: String,
    pub archive: FileIdentity,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub template: TemplateIdentity,
    pub namespace: String,
    pub profile: FileReference,
    pub sdk: FileReference,
    pub tool: ToolIdentity,
}
/// Complete request projection after the actual root and descriptor bytes were captured.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestIdentity {
    pub operation: Operation,
    pub root: AbsolutePath,
    pub descriptor: FileIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum MemberKind {
    File,
    Directory,
}
impl MemberKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryMember {
    pub path: LogicalPath,
    pub kind: MemberKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputDirectory {
    pub path: LogicalPath,
    pub present: bool,
    pub members: Vec<DirectoryMember>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputFile {
    pub source: LogicalPath,
    pub file: FileIdentity,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputInventory {
    pub files: Vec<InputFile>,
    pub directories: Vec<InputDirectory>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviousBuild {
    pub artifact_root: AbsolutePath,
    pub identity: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildAttempt {
    pub root: AbsolutePath,
    pub inputs: InputInventory,
    pub previous: Option<PreviousBuild>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateOutput {
    pub root: LogicalPath,
    pub manifest: FileIdentity,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputProjection {
    pub snapshot_digest: Digest,
    pub candidate: CandidateOutput,
    pub consumer: FileIdentity,
    pub outputs: Vec<FileIdentity>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOutput {
    pub identity: Digest,
    pub projection: OutputProjection,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledTool {
    pub path: AbsolutePath,
    pub sha256: Digest,
    pub size: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Durability {
    NotApplicable,
    Confirmed,
    Uncertain,
}
impl Durability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::Confirmed => "confirmed",
            Self::Uncertain => "uncertain",
        }
    }
}
/// Actual effect survives a later durability/evidence error. None means the
/// replacement fact remains unknown after read-only inspection.
pub struct PublicationObservation {
    pub published: Option<bool>,
    pub durability: Durability,
    pub error: Option<QueryError>,
}
pub struct InstallationObservation {
    pub installed: Option<InstalledTool>,
    pub error: Option<QueryError>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageId {
    Capture,
    Preflight,
    Prepare,
    Metadata,
    Generate,
    Build,
    Package,
    ConsumerLink,
    Publish,
    Install,
}
impl StageId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Preflight => "preflight",
            Self::Prepare => "prepare",
            Self::Metadata => "metadata",
            Self::Generate => "generate",
            Self::Build => "build",
            Self::Package => "package",
            Self::ConsumerLink => "consumer-link",
            Self::Publish => "publish",
            Self::Install => "install",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageStatus {
    Passed,
    Failed,
    Error,
    Blocked,
    Skipped,
    Unsupported,
}
impl StageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
            Self::Unsupported => "unsupported",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageObservation {
    pub id: StageId,
    pub status: StageStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub attachments: Vec<FileIdentity>,
    pub blocked_by: Vec<StageId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    Complete,
    Failed,
    Cancelled,
}
impl Termination {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    pub kind: String,
    pub message: String,
    pub command: Option<Vec<String>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectReport {
    pub operation: Operation,
    pub root: Option<AbsolutePath>,
    pub recipe: Option<Id>,
    pub request_digest: Option<Digest>,
    pub tool: Option<ToolIdentity>,
    pub artifact_root: Option<AbsolutePath>,
    pub inputs: InputInventory,
    pub stages: Vec<StageObservation>,
    pub result: Option<ProjectOutput>,
    pub installed_tool: Option<InstalledTool>,
    pub previous: Option<PreviousBuild>,
    pub published: Option<bool>,
    pub publication_durability: Durability,
    pub termination: Termination,
    pub exit_code: u8,
    pub evidence_root: Option<AbsolutePath>,
    pub logs: Vec<FileIdentity>,
    pub diagnostics: Vec<Diagnostic>,
    pub recovery: Vec<Recovery>,
}
impl ProjectReport {
    pub fn empty(operation: Operation) -> Self {
        Self {
            operation,
            root: None,
            recipe: None,
            request_digest: None,
            tool: None,
            artifact_root: None,
            inputs: InputInventory::default(),
            stages: Vec::new(),
            result: None,
            installed_tool: None,
            previous: None,
            published: Some(false),
            publication_durability: Durability::NotApplicable,
            termination: Termination::Failed,
            exit_code: 2,
            evidence_root: None,
            logs: Vec::new(),
            diagnostics: Vec::new(),
            recovery: Vec::new(),
        }
    }
}
