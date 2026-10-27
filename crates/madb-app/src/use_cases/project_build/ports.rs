use super::*;
use crate::{Digest, Result, transaction::CommitControl};

pub trait ProjectBuildFormat {
    fn decode_descriptor(&self, bytes: &[u8]) -> Result<Descriptor>;
    fn encode_descriptor(&self, descriptor: &Descriptor) -> Result<Vec<u8>>;
    fn encode_report(&self, report: &ProjectReport) -> Result<Vec<u8>>;
    fn request_digest(&self, request: &RequestIdentity) -> Result<Digest>;
    fn input_digest(&self, descriptor: &FileIdentity, inputs: &InputInventory) -> Result<Digest>;
    fn output_digest(&self, tool: &ToolIdentity, output: &OutputProjection) -> Result<Digest>;
}

/// Host retains directory/file identities and index baseline privately. None of
/// these observational values can reopen, adopt or publish another output domain.
pub trait ProjectWorkspace {
    fn root(&self) -> &AbsolutePath;
    fn descriptor_bytes(&self) -> Result<Vec<u8>>;
    /// Validate the fixed origin record and original descriptor/archive bytes.
    /// This is also the complete source prerequisite for install-tool.
    fn check_origin(&mut self, descriptor: &Descriptor, original: &[u8]) -> Result<()>;
    fn capture(&mut self, descriptor: &Descriptor, original: &[u8]) -> Result<CapturedInputs>;
    fn begin_attempt(&mut self, captured: &CapturedInputs) -> Result<BuildAttempt>;
    /// A directory may exist even when subsequent copying or syncing failed.
    fn created_attempt_root(&self) -> Option<&AbsolutePath>;
    fn revalidate(&self) -> Result<()>;
    fn verify_outputs(&self, attempt: &BuildAttempt, output: &ProjectOutput) -> Result<()>;
    fn publish(
        &mut self,
        attempt: &BuildAttempt,
        output: &ProjectOutput,
        control: &CommitControl,
    ) -> PublicationObservation;
    /// Captures the actual running image independently; never accepts an arbitrary
    /// caller path as the installer source. Exact source/version/bytes must match.
    fn install_tool(
        &mut self,
        descriptor: &Descriptor,
        control: &CommitControl,
    ) -> InstallationObservation;
}

/// Implemented by the composition root using its existing C04 coordinator. The
/// app holds this port rather than importing CLI or constructing Host adapters.
pub trait ProjectBuildExecution {
    fn preflight(&mut self, operation: Operation, descriptor: &Descriptor) -> Result<()>;
    fn execute(
        &mut self,
        descriptor: &Descriptor,
        attempt: &BuildAttempt,
        snapshot: &Digest,
        report: &mut ExecutionProgress<'_>,
    ) -> BuildResult<ProjectOutput>;
}

#[derive(Debug)]
pub enum BuildFailure {
    Error(crate::QueryError),
    Cancelled,
}
impl From<crate::QueryError> for BuildFailure {
    fn from(error: crate::QueryError) -> Self {
        Self::Error(error)
    }
}
pub type BuildResult<T> = std::result::Result<T, BuildFailure>;
pub trait ProjectBuildControl {
    fn checkpoint(&self) -> BuildResult<()>;
    fn commit(&self) -> &CommitControl;
}
