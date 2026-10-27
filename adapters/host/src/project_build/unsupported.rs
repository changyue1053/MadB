//! Keep the CLI type surface available without implying host support.
use madb_app::{
    ErrorKind, QueryError, Result, transaction::CommitControl, use_cases::project_build::*,
};

pub struct LocalProjectBuild {
    _private: (),
}
impl LocalProjectBuild {
    pub fn open(_: &AbsolutePath) -> Result<Self> {
        Err(unavailable())
    }
}
pub fn origin_bytes(_: &Descriptor, _: &[u8]) -> Result<Vec<u8>> {
    Err(unavailable())
}
impl ProjectWorkspace for LocalProjectBuild {
    fn root(&self) -> &AbsolutePath {
        unreachable!("unsupported open cannot succeed")
    }
    fn descriptor_bytes(&self) -> Result<Vec<u8>> {
        Err(unavailable())
    }
    fn check_origin(&mut self, _: &Descriptor, _: &[u8]) -> Result<()> {
        Err(unavailable())
    }
    fn capture(&mut self, _: &Descriptor, _: &[u8]) -> Result<CapturedInputs> {
        Err(unavailable())
    }
    fn begin_attempt(&mut self, _: &CapturedInputs) -> Result<BuildAttempt> {
        Err(unavailable())
    }
    fn created_attempt_root(&self) -> Option<&AbsolutePath> {
        None
    }
    fn revalidate(&self) -> Result<()> {
        Err(unavailable())
    }
    fn verify_outputs(&self, _: &BuildAttempt, _: &ProjectOutput) -> Result<()> {
        Err(unavailable())
    }
    fn publish(
        &mut self,
        _: &BuildAttempt,
        _: &ProjectOutput,
        _: &CommitControl,
    ) -> PublicationObservation {
        PublicationObservation {
            published: Some(false),
            durability: Durability::NotApplicable,
            error: Some(unavailable()),
        }
    }
    fn install_tool(&mut self, _: &Descriptor, _: &CommitControl) -> InstallationObservation {
        InstallationObservation {
            installed: None,
            error: Some(unavailable()),
        }
    }
}
fn unavailable() -> QueryError {
    QueryError::new(
        ErrorKind::Host,
        "HOST003",
        "project",
        "project rebuilding is not implemented on this host",
    )
}
