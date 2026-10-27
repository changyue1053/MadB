//! Observation values only. No value in this module grants execution authority.
use crate::use_cases::initialize::{Diagnostic, Recovery, ToolObservation, VerifyLevel};
use crate::{Digest, ErrorKind, Id, LogicalPath};
use madb_model::contract::ResolvedSelector;
pub use madb_plan::project::{ComponentIdentity, FileIdentity, FileReference};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub root: RootPath,
    pub project: LogicalPath,
    pub read_request: LogicalPath,
    pub snapshot: LogicalPath,
    pub verify: VerifyLevel,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectEncoding {
    Json,
    Toml,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub project: FileIdentity,
    pub project_format: ProjectEncoding,
    pub read_request: FileReference,
    pub lock: FileReference,
    pub profile: FileReference,
    pub snapshot: Option<FileReference>,
    pub selector: ResolvedSelector,
    pub recipe: Option<ComponentIdentity>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckKind {
    Configuration,
    Input,
    Recipe,
    Host,
    Tool,
    Sdk,
    Resource,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Passed,
    Failed,
    Error,
    Unsupported,
    Blocked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestKind {
    Project,
    Profile,
    Source,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Checked,
    Supported,
    Missing,
    Unsupported,
    NotChecked,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fact {
    Bytes {
        sha256: Digest,
        size: u64,
    },
    Digest {
        domain: DigestKind,
        sha256: Digest,
    },
    Tool(ToolObservation),
    Host {
        os: Id,
        arch: Id,
        version: Option<String>,
        build: Option<String>,
    },
    State(State),
}
impl Fact {
    pub fn bytes(bytes: &[u8]) -> Self {
        Self::Bytes {
            sha256: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Check {
    pub id: Id,
    pub kind: CheckKind,
    pub subject: String,
    pub status: Status,
    pub error_kind: Option<ErrorKind>,
    pub expected: Option<Fact>,
    pub observed: Option<Fact>,
    pub diagnostics: Vec<Diagnostic>,
    pub blocked_by: Vec<Id>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub tool: Option<ComponentIdentity>,
    pub request: Option<Request>,
    pub context: Option<Context>,
    pub truncated: bool,
    pub checks: Vec<Check>,
    pub recovery: Vec<Recovery>,
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: u8,
}

/// A doctor root is an existing directory; unlike init output it may be `/`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPath(String);
impl RootPath {
    pub fn new(value: &str) -> crate::Result<Self> {
        if !value.starts_with('/')
            || value.chars().count() > 4096
            || value.chars().any(char::is_control)
            || value.split('/').any(|v| matches!(v, "." | ".."))
        {
            return Err(crate::QueryError::new(
                ErrorKind::InvalidInput,
                "FILE001",
                "doctor root",
                "expected a bounded normalized absolute path",
            ));
        }
        let parts = value
            .split('/')
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>();
        Ok(Self(format!("/{}", parts.join("/"))))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
