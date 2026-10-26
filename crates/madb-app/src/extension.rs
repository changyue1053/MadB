//! Byte-oriented dialogue port. Wire decoding and negotiation belong to the
//! contracts adapter; process ownership and IO belong to the host adapter.
use crate::process::{ProcessRequest, ProcessResult};
use std::time::Duration;

pub enum SessionStep {
    Send(Vec<u8>),
    Terminal,
}

/// One request per process session. A terminal frame is only a claim: the
/// process must exit cleanly and outputs must be captured before admission.
pub trait SessionDriver {
    fn validate_request(&self, request: &ProcessRequest) -> ProcessResult<()>;
    fn initial(&mut self) -> ProcessResult<Vec<u8>>;
    fn receive(&mut self, line: &[u8]) -> ProcessResult<SessionStep>;
    /// Called only after execution cancellation/deadline has barred admission.
    /// None means no request was started, so there is no wire cancel to send.
    fn cancel(&mut self) -> ProcessResult<Option<Vec<u8>>>;
    fn is_terminal(&self) -> bool;
    fn timeout(&self) -> Duration;
}
