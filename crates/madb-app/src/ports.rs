use crate::{Digest, LogicalPath, Result, TaskGraph};
use madb_model::contract::Contract;

/// A reference to bytes, not a claim that a reader discovered every dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReference {
    pub path: LogicalPath,
    pub digest: Digest,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct RequestFiles {
    pub lock: FileReference,
    pub profile: FileReference,
}

/// Implementations may read bounded regular files, but cannot execute a tool or
/// repair/create a missing file. A missing leaf or parent returns None.
pub trait ReadWorkspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>>;
}

pub struct ConfigurationBytes<'a> {
    pub request: &'a [u8],
    pub project: &'a [u8],
    pub lock: &'a [u8],
    pub profile: &'a [u8],
}

/// Prepared is adapter-owned decoded state. Wire DTOs never enter app rules.
pub trait SnapshotFormat {
    type Prepared;
    fn request_files(&self, request: &[u8]) -> Result<RequestFiles>;
    fn prepare(
        &self,
        files: ConfigurationBytes<'_>,
    ) -> Result<(Self::Prepared, Vec<FileReference>)>;
    fn check(&self, prepared: &Self::Prepared, snapshot: &[u8]) -> Result<Contract>;
    fn task_graph(&self, prepared: &Self::Prepared, tasks: &[Vec<u8>]) -> Result<TaskGraph>;
}

/// The implementation owns process creation, bounded pipes and OS cleanup.
/// It must never return a successful process observation for incomplete cleanup.
pub trait ProcessRunner {
    fn run(
        &self,
        request: &crate::process::ProcessRequest,
        cancellation: &crate::process::CancellationProbe<'_>,
    ) -> crate::process::ProcessResult<crate::process::ProcessRun>;
}

/// Retained ownership after a cleanup deadline. Callers must keep this handle
/// until it reports reaped; dropping it is not a cleanup acknowledgement.
pub trait PendingProcess: Send {
    fn poll_reaped(&mut self) -> crate::process::ProcessResult<bool>;
}
