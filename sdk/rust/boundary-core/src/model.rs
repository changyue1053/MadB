use std::fmt;
pub const PROFILE_ID: &str = "madb-c06-bounded-v1";
pub const MAX_PAYLOAD: usize = 1024;
pub(crate) const MAX_BYTES: usize = 4096;
pub(crate) const MAX_EVENTS: usize = 8;
pub(crate) const MAX_OPERATIONS: usize = 8;
pub(crate) const MAX_SUBSCRIPTIONS: usize = 8;
pub(crate) const MAX_WAITERS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Working,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub id: String,
    pub sequence: Option<i64>,
    pub text: String,
    pub bytes: Vec<u8>,
    pub phase: Phase,
}
impl Value {
    pub fn payload_bytes(&self) -> usize {
        self.id
            .len()
            .saturating_add(self.text.len())
            .saturating_add(self.bytes.len())
    }
    pub fn validate(&self) -> Result<(), BoundaryError> {
        if self.payload_bytes() > MAX_PAYLOAD {
            Err(BoundaryError::ResourceExhausted)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryError {
    InvalidArgument,
    Closing,
    Closed,
    Cancelled,
    ResourceExhausted,
    ProfileMismatch,
    CallbackFailure,
    WouldDeadlock,
    AlreadyConsumed,
}
impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BoundaryError {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    pub queued_items: u64,
    pub retained_bytes: u64,
    pub callbacks_in_flight: u64,
    pub active_operations: u64,
    pub active_subscriptions: u64,
    pub payloads_admitted: u64,
    pub payloads_released: u64,
    pub operations_created: u64,
    pub operations_released: u64,
    pub subscriptions_created: u64,
    pub subscriptions_released: u64,
    pub worker_exits: u64,
}
