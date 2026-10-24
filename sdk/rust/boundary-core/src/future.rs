use crate::{
    engine::Shared,
    model::{BoundaryError, MAX_WAITERS},
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// A bounded registration owned by the waiting future, including cancellation cleanup.
pub(crate) struct Wait<T, F: FnMut() -> Poll<Result<T, BoundaryError>>> {
    shared: Arc<Shared>,
    check: F,
    key: Option<u64>,
}
impl<T, F: FnMut() -> Poll<Result<T, BoundaryError>>> Wait<T, F> {
    pub(crate) fn new(shared: Arc<Shared>, check: F) -> Self {
        Self {
            shared,
            check,
            key: None,
        }
    }
}
impl<T, F: FnMut() -> Poll<Result<T, BoundaryError>> + Unpin> Future for Wait<T, F> {
    type Output = Result<T, BoundaryError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Register before checking the condition, preventing a completion/wake gap.
        // Waker clone/drop may execute arbitrary vtable code. Own it outside
        // Shared.state; only ordinary Arc bookkeeping occurs under the lock.
        let incoming = Arc::new(cx.waker().clone());
        let mut state = this.shared.lock();
        let key = match this.key {
            Some(key) => key,
            None => {
                if state.waiters.len() >= MAX_WAITERS {
                    return Poll::Ready(Err(BoundaryError::ResourceExhausted));
                }
                let key = state.next_waiter;
                state.next_waiter += 1;
                this.key = Some(key);
                key
            }
        };
        let replaced = state.waiters.insert(key, incoming);
        drop(state);
        drop(replaced);
        let result = (this.check)();
        if result.is_ready() {
            let removed = { this.shared.lock().waiters.remove(&key) };
            drop(removed);
        }
        result
    }
}
impl<T, F: FnMut() -> Poll<Result<T, BoundaryError>>> Drop for Wait<T, F> {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            let removed = { self.shared.lock().waiters.remove(&key) };
            drop(removed);
        }
    }
}
