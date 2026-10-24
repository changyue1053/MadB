//! P03 only. The invoking foreign task owns these async calls and its teardown acknowledgment.
//! Engine close makes no claim about a separate backend-owned foreign coroutine.
use super::{BoundaryError, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[uniffi::export(callback_interface)]
#[async_trait::async_trait]
pub trait AsyncHost: Send + Sync {
    async fn transform(&self, value: Value) -> Result<Value, BoundaryError>;
}
#[uniffi::export]
pub async fn async_host_transform(
    value: Value,
    host: Box<dyn AsyncHost>,
) -> Result<Value, BoundaryError> {
    host.transform(value).await
}

#[derive(uniffi::Object, Default)]
pub struct TestLedger {
    created: AtomicU64,
    released: AtomicU64,
}
#[uniffi::export]
impl TestLedger {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn create_resource(self: Arc<Self>) -> Arc<AsyncResource> {
        let id = self.created.fetch_add(1, Ordering::SeqCst) + 1;
        Arc::new(AsyncResource { ledger: self, id })
    }
    pub fn created(&self) -> u64 {
        self.created.load(Ordering::SeqCst)
    }
    pub fn released(&self) -> u64 {
        self.released.load(Ordering::SeqCst)
    }
}
#[derive(uniffi::Object)]
pub struct AsyncResource {
    ledger: Arc<TestLedger>,
    id: u64,
}
#[uniffi::export]
impl AsyncResource {
    pub fn id(&self) -> u64 {
        self.id
    }
}
impl Drop for AsyncResource {
    fn drop(&mut self) {
        self.ledger.released.fetch_add(1, Ordering::SeqCst);
    }
}
#[cfg(not(feature = "gobley-029"))]
#[uniffi::export(callback_interface)]
#[async_trait::async_trait]
pub trait AsyncObjectHost: Send + Sync {
    async fn produce(&self, resource: Arc<AsyncResource>) -> Arc<AsyncResource>;
}
#[cfg(not(feature = "gobley-029"))]
#[uniffi::export]
pub async fn async_host_object(
    resource: Arc<AsyncResource>,
    host: Box<dyn AsyncObjectHost>,
) -> Result<Arc<AsyncResource>, BoundaryError> {
    Ok(host.produce(resource).await)
}
