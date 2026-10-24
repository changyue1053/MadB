use crate::contract::ResolvedSelector;
use crate::{Digest, Id};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceIdentity {
    pub input: Digest,
    pub tools: Digest,
    pub profile: Digest,
    pub candidate: Digest,
    pub lab_revision: String,
    pub case_catalog: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Freshness {
    Missing,
    Current,
    Stale(Vec<&'static str>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Applicability {
    Required,
    Optional,
    NotApplicable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Requirement {
    pub problem_id: Id,
    pub applicability: Applicability,
    pub reason: String,
    pub case_ids: Vec<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequirementInstance {
    pub selector: ResolvedSelector,
    pub requirement: Requirement,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStatus {
    Passed,
    Failed,
    Error,
    Skipped,
    Unsupported,
    Blocked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EvidenceLevel {
    Static,
    Generated,
    Compiled,
    Linked,
    HostRun,
    TargetRun,
    DeviceRun,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedOutcome {
    Success,
    Rejection { code: Id, stage: Id },
    IsolatedAbort { exit_code: i32, reason: Id },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActualOutcome {
    Success,
    Rejection { code: Id, stage: Id },
    IsolatedAbort { exit_code: i32, reason: Id },
    Unavailable,
    NotRun,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceObservation {
    pub name: Id,
    pub before: u64,
    pub after: u64,
    pub created: u64,
    pub released: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseResult {
    pub case_id: Id,
    pub case_digest: Digest,
    pub status: ExecutionStatus,
    pub expected: ExpectedOutcome,
    pub actual: ActualOutcome,
    pub level: EvidenceLevel,
    pub cleanup_complete: bool,
    pub attachment_count: usize,
    pub resources: Vec<ResourceObservation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredCase {
    pub id: Id,
    pub digest: Digest,
    pub expected: ExpectedOutcome,
    pub minimum_level: EvidenceLevel,
}
