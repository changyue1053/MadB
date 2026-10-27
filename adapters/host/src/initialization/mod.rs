//! macOS initialization filesystem effects. Directory publication is an actual
//! no-replace syscall; these adapters never execute recipes or external tools.
#[cfg(target_os = "macos")]
mod evidence;
#[cfg(target_os = "macos")]
mod filesystem;
#[cfg(target_os = "macos")]
mod project;
#[cfg(target_os = "macos")]
mod staging;

#[cfg(target_os = "macos")]
pub use evidence::LocalInitEvidence;
#[cfg(target_os = "macos")]
pub use project::LocalProjectCreator;

use madb_app::{ErrorKind, QueryError};
use std::path::Path;

/// Explicit fault/barrier points. Hooks run outside the cancellation mutex and
/// are supplied by the caller; no environment variable enables fault injection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationPoint {
    StageCreated,
    FileWritten,
    BeforePublish,
    AfterPublish,
}

/// The path is observational only. All implementation operations remain bound
/// to retained directory descriptors even when a test replaces this spelling.
pub struct InitializationEvent<'a> {
    pub point: InitializationPoint,
    pub staging: &'a Path,
}

fn failure(kind: ErrorKind, subject: &str, message: impl std::fmt::Display) -> QueryError {
    QueryError::new(kind, "FILE001", subject, message.to_string())
}
fn host(subject: &str, message: impl std::fmt::Display) -> QueryError {
    failure(ErrorKind::Host, subject, message)
}
#[cfg(target_os = "macos")]
fn conflict(subject: &str, message: impl std::fmt::Display) -> QueryError {
    failure(ErrorKind::Conflict, subject, message)
}

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use super::*;
    use madb_app::use_cases::initialize::{
        AbsolutePath, CommitNewPort, FileIdentity, HostCommitObservation, InitEvidencePort,
        PreparedProject, SavedEvidenceObservation,
    };
    use madb_app::{Result, ports::FileReference, transaction::CommitControl};
    pub struct LocalProjectCreator;
    impl LocalProjectCreator {
        pub fn open(_: &AbsolutePath, _: CommitControl) -> Result<Self> {
            Err(unavailable())
        }
        pub fn cancellation_observed(&self) -> bool {
            false
        }
    }
    impl CommitNewPort for LocalProjectCreator {
        fn commit_new(&mut self, _: &PreparedProject) -> Result<HostCommitObservation> {
            Err(unavailable())
        }
    }
    pub struct LocalInitEvidence;
    impl LocalInitEvidence {
        pub fn create(_: &AbsolutePath) -> Result<Self> {
            Err(unavailable())
        }
        pub fn allocate(_: &AbsolutePath) -> Result<Self> {
            Err(unavailable())
        }
        pub fn root(&self) -> &AbsolutePath {
            unreachable!("unsupported constructors cannot succeed")
        }
        pub fn result_was_saved(&self) -> bool {
            false
        }
        pub fn fork_verified(&self) -> Result<Self> {
            Err(unavailable())
        }
        pub fn store_file(&mut self, _: &FileReference, _: &[u8]) -> Result<()> {
            Err(unavailable())
        }
    }
    impl InitEvidencePort for LocalInitEvidence {
        fn save_result(
            &mut self,
            _: &AbsolutePath,
            _: &[u8],
            _: &[FileIdentity],
        ) -> Result<SavedEvidenceObservation> {
            Err(unavailable())
        }
    }
    fn unavailable() -> QueryError {
        host(
            "initialization",
            "atomic no-replace initialization is not implemented on this host",
        )
    }
}
#[cfg(not(target_os = "macos"))]
pub use unsupported::{LocalInitEvidence, LocalProjectCreator};
