//! The UDL is the public ABI authority; this layer maps its types to bounded SDK semantics.
use madb_boundary_core as core;
use std::sync::Arc;
uniffi::include_scaffolding!("boundary");

#[derive(Clone, Copy, Debug)]
pub enum Phase {
    Ready,
    Working,
    Done,
}
#[derive(Clone, Debug)]
pub struct Value {
    pub id: String,
    pub sequence: Option<i64>,
    pub text: String,
    pub bytes: Vec<u8>,
    pub phase: Phase,
}
impl From<Value> for core::Value {
    fn from(v: Value) -> Self {
        Self {
            id: v.id,
            sequence: v.sequence,
            text: v.text,
            bytes: v.bytes,
            phase: match v.phase {
                Phase::Ready => core::Phase::Ready,
                Phase::Working => core::Phase::Working,
                Phase::Done => core::Phase::Done,
            },
        }
    }
}
impl From<core::Value> for Value {
    fn from(v: core::Value) -> Self {
        Self {
            id: v.id,
            sequence: v.sequence,
            text: v.text,
            bytes: v.bytes,
            phase: match v.phase {
                core::Phase::Ready => Phase::Ready,
                core::Phase::Working => Phase::Working,
                core::Phase::Done => Phase::Done,
            },
        }
    }
}
#[derive(Debug)]
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
impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BoundaryError {}
impl From<core::BoundaryError> for BoundaryError {
    fn from(e: core::BoundaryError) -> Self {
        match e {
            core::BoundaryError::InvalidArgument => Self::InvalidArgument,
            core::BoundaryError::Closing => Self::Closing,
            core::BoundaryError::Closed => Self::Closed,
            core::BoundaryError::Cancelled => Self::Cancelled,
            core::BoundaryError::ResourceExhausted => Self::ResourceExhausted,
            core::BoundaryError::ProfileMismatch => Self::ProfileMismatch,
            core::BoundaryError::CallbackFailure => Self::CallbackFailure,
            core::BoundaryError::WouldDeadlock => Self::WouldDeadlock,
            core::BoundaryError::AlreadyConsumed => Self::AlreadyConsumed,
        }
    }
}
impl From<BoundaryError> for core::BoundaryError {
    fn from(e: BoundaryError) -> Self {
        match e {
            BoundaryError::InvalidArgument => Self::InvalidArgument,
            BoundaryError::Closing => Self::Closing,
            BoundaryError::Closed => Self::Closed,
            BoundaryError::Cancelled => Self::Cancelled,
            BoundaryError::ResourceExhausted => Self::ResourceExhausted,
            BoundaryError::ProfileMismatch => Self::ProfileMismatch,
            BoundaryError::CallbackFailure => Self::CallbackFailure,
            BoundaryError::WouldDeadlock => Self::WouldDeadlock,
            BoundaryError::AlreadyConsumed => Self::AlreadyConsumed,
        }
    }
}
// Unexpected callback exceptions become a typed boundary failure; no unwind crosses the ABI.
impl From<uniffi::UnexpectedUniFFICallbackError> for BoundaryError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::CallbackFailure
    }
}
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
impl From<core::Statistics> for Statistics {
    fn from(s: core::Statistics) -> Self {
        Self {
            queued_items: s.queued_items,
            retained_bytes: s.retained_bytes,
            callbacks_in_flight: s.callbacks_in_flight,
            active_operations: s.active_operations,
            active_subscriptions: s.active_subscriptions,
            payloads_admitted: s.payloads_admitted,
            payloads_released: s.payloads_released,
            operations_created: s.operations_created,
            operations_released: s.operations_released,
            subscriptions_created: s.subscriptions_created,
            subscriptions_released: s.subscriptions_released,
            worker_exits: s.worker_exits,
        }
    }
}
pub trait Host: Send + Sync {
    fn transform(&self, value: Value) -> Result<Value, BoundaryError>;
}
pub trait Sink: Send + Sync {
    fn on_value(&self, value: Value) -> Result<(), BoundaryError>;
}
struct HostAdapter(Box<dyn Host>);
impl core::Host for HostAdapter {
    fn transform(&self, value: core::Value) -> Result<core::Value, core::BoundaryError> {
        self.0
            .transform(value.into())
            .map(Into::into)
            .map_err(Into::into)
    }
}
struct SinkAdapter(Box<dyn Sink>);
impl core::Sink for SinkAdapter {
    fn on_value(&self, value: core::Value) -> Result<(), core::BoundaryError> {
        self.0.on_value(value.into()).map_err(Into::into)
    }
}
pub fn profile_identity() -> String {
    option_env!("MADB_PROFILE_DIGEST")
        .unwrap_or(core::PROFILE_ID)
        .to_owned()
}
pub struct Engine(core::Engine);
impl Engine {
    pub fn new(expected_profile: String) -> Result<Self, BoundaryError> {
        if expected_profile != profile_identity() {
            return Err(BoundaryError::ProfileMismatch);
        }
        Ok(Self(core::Engine::new(core::PROFILE_ID)?))
    }
    pub fn roundtrip(&self, value: Value) -> Result<Value, BoundaryError> {
        self.0
            .roundtrip(value.into())
            .map(Into::into)
            .map_err(Into::into)
    }
    pub fn start_operation(
        &self,
        value: Value,
        host: Box<dyn Host>,
    ) -> Result<Arc<Operation>, BoundaryError> {
        Ok(Arc::new(Operation(self.0.start_operation(
            value.into(),
            Arc::new(HostAdapter(host)),
        )?)))
    }
    pub fn observe_state(&self, sink: Box<dyn Sink>) -> Result<Arc<Subscription>, BoundaryError> {
        Ok(Arc::new(Subscription(
            self.0.observe_state(Arc::new(SinkAdapter(sink)))?,
        )))
    }
    pub fn observe_events(&self, sink: Box<dyn Sink>) -> Result<Arc<Subscription>, BoundaryError> {
        Ok(Arc::new(Subscription(
            self.0.observe_events(Arc::new(SinkAdapter(sink)))?,
        )))
    }
    pub fn publish_state(&self, value: Value) -> Result<(), BoundaryError> {
        self.0.publish_state(value.into()).map_err(Into::into)
    }
    pub fn try_publish_event(&self, value: Value) -> Result<(), BoundaryError> {
        self.0.try_publish_event(value.into()).map_err(Into::into)
    }
    pub fn statistics(&self) -> Statistics {
        self.0.statistics().into()
    }
    pub fn request_close(&self) {
        self.0.request_close();
    }
    pub async fn await_closed(&self) -> Result<(), BoundaryError> {
        self.0.await_closed().await.map_err(Into::into)
    }
}
pub struct Operation(Arc<core::Operation>);
impl Operation {
    pub fn request_cancel(&self) {
        self.0.request_cancel();
    }
    pub fn is_cancel_requested(&self) -> bool {
        self.0.is_cancel_requested()
    }
    pub async fn result(&self) -> Result<Value, BoundaryError> {
        self.0.result().await.map(Into::into).map_err(Into::into)
    }
}
pub struct Subscription(Arc<core::Subscription>);
impl Subscription {
    pub fn request_stop(&self) {
        self.0.request_stop();
    }
    pub async fn await_stopped(&self) -> Result<(), BoundaryError> {
        self.0.await_stopped().await.map_err(Into::into)
    }
}

#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub use test_support::*;

impl Drop for Operation {
    fn drop(&mut self) {
        self.0.request_cancel();
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.0.request_stop();
    }
}

#[cfg(all(feature = "test-support", feature = "gobley-029"))]
#[path = "../../gobley/src/unsigned_fixture.rs"]
mod unsigned_fixture;
