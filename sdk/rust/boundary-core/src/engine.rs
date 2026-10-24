use crate::{future::Wait, model::*};
use std::{
    cell::Cell,
    collections::{BTreeMap, VecDeque},
    future::Future,
    sync::{Arc, Condvar, Mutex, MutexGuard, Weak},
    task::{Poll, Waker},
    thread,
};

pub trait Host: Send + Sync {
    fn transform(&self, value: Value) -> Result<Value, BoundaryError>;
}
pub trait Sink: Send + Sync {
    fn on_value(&self, value: Value) -> Result<(), BoundaryError>;
}
thread_local! { static CALLBACK_INSTANCE: Cell<usize> = const { Cell::new(0) }; }
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Open,
    Closing,
    Closed,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    State,
    Event,
}
struct SubState {
    stopping: bool,
    cleaning: bool,
    queued: usize,
    running: bool,
    sink: Option<Arc<dyn Sink>>,
    error: Option<BoundaryError>,
}
struct OpState {
    cancel: bool,
    finished: bool,
    consumed: bool,
    result: Option<Result<Value, BoundaryError>>,
    retained: usize,
}
struct Delivery {
    sub: Arc<Subscription>,
    value: Value,
}
struct Job {
    op: Arc<Operation>,
    host: Arc<dyn Host>,
    value: Value,
}
struct Cleanup {
    state: Arc<Mutex<SubState>>,
    sink: Arc<dyn Sink>,
}
enum Item {
    Delivery(Delivery),
    Job(Job),
    Cleanup(Cleanup),
}
pub(crate) struct State {
    life: Lifecycle,
    source_leases: usize,
    queue: VecDeque<Item>,
    events: usize,
    operations: Vec<Weak<Operation>>,
    subscriptions: Vec<Weak<Subscription>>,
    subscription_slots: Vec<Weak<Mutex<SubState>>>,
    stats: Statistics,
    pub(crate) waiters: BTreeMap<u64, Arc<Waker>>,
    pub(crate) next_waiter: u64,
}
pub(crate) struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}
impl Shared {
    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn notify(&self) {
        let wake: Vec<_> = self.lock().waiters.values().cloned().collect();
        self.ready.notify_all();
        for w in wake {
            w.wake_by_ref();
        }
    }
    fn check_open(state: &State) -> Result<(), BoundaryError> {
        match state.life {
            Lifecycle::Open => Ok(()),
            Lifecycle::Closing => Err(BoundaryError::Closing),
            Lifecycle::Closed => Err(BoundaryError::Closed),
        }
    }
    fn is_callback(&self) -> bool {
        CALLBACK_INSTANCE.with(|c| c.get() == self as *const Self as usize)
    }
}

/// One instance owns one worker. All mutable protocol state is guarded by Shared.state;
/// object state mutexes are acquired only after Shared.state, never in the opposite order.
pub struct Engine {
    shared: Arc<Shared>,
}
impl Engine {
    pub fn new(expected_profile: &str) -> Result<Self, BoundaryError> {
        if expected_profile != PROFILE_ID {
            return Err(BoundaryError::ProfileMismatch);
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                life: Lifecycle::Open,
                source_leases: 0,
                queue: VecDeque::new(),
                events: 0,
                operations: vec![],
                subscriptions: vec![],
                subscription_slots: vec![],
                stats: Statistics::default(),
                waiters: BTreeMap::new(),
                next_waiter: 1,
            }),
            ready: Condvar::new(),
        });
        let worker = shared.clone();
        thread::Builder::new()
            .name("madb-boundary".into())
            .spawn(move || run_worker(worker))
            .map_err(|_| BoundaryError::ResourceExhausted)?;
        Ok(Self { shared })
    }
    pub fn roundtrip(&self, value: Value) -> Result<Value, BoundaryError> {
        let s = self.shared.lock();
        Shared::check_open(&s)?;
        value.validate()?;
        Ok(value)
    }
    pub fn start_operation(
        &self,
        value: Value,
        host: Arc<dyn Host>,
    ) -> Result<Arc<Operation>, BoundaryError> {
        value.validate()?;
        let mut s = self.shared.lock();
        Shared::check_open(&s)?;
        s.operations.retain(|o| o.strong_count() > 0);
        if s.operations.len() >= MAX_OPERATIONS
            || s.stats.retained_bytes + value.payload_bytes() as u64 > MAX_BYTES as u64
        {
            return Err(BoundaryError::ResourceExhausted);
        }
        let op = Arc::new(Operation {
            shared: self.shared.clone(),
            state: Mutex::new(OpState {
                cancel: false,
                finished: false,
                consumed: false,
                result: None,
                retained: value.payload_bytes(),
            }),
        });
        s.operations.push(Arc::downgrade(&op));
        s.stats.operations_created += 1;
        s.stats.active_operations += 1;
        s.stats.retained_bytes += value.payload_bytes() as u64;
        s.stats.payloads_admitted += 1;
        s.queue.push_back(Item::Job(Job {
            op: op.clone(),
            host,
            value,
        }));
        drop(s);
        self.shared.notify();
        Ok(op)
    }
    pub fn observe_state(&self, sink: Arc<dyn Sink>) -> Result<Arc<Subscription>, BoundaryError> {
        self.observe(Kind::State, sink)
    }
    pub fn observe_events(&self, sink: Arc<dyn Sink>) -> Result<Arc<Subscription>, BoundaryError> {
        self.observe(Kind::Event, sink)
    }
    fn observe(&self, kind: Kind, sink: Arc<dyn Sink>) -> Result<Arc<Subscription>, BoundaryError> {
        let mut s = self.shared.lock();
        Shared::check_open(&s)?;
        s.subscriptions.retain(|x| x.strong_count() > 0);
        s.subscription_slots.retain(|x| x.strong_count() > 0);
        if s.subscription_slots.len() >= MAX_SUBSCRIPTIONS {
            return Err(BoundaryError::ResourceExhausted);
        }
        let sub = Arc::new(Subscription {
            shared: self.shared.clone(),
            kind,
            state: Arc::new(Mutex::new(SubState {
                stopping: false,
                cleaning: false,
                queued: 0,
                running: false,
                sink: Some(sink),
                error: None,
            })),
        });
        s.subscriptions.push(Arc::downgrade(&sub));
        s.subscription_slots.push(Arc::downgrade(&sub.state));
        s.source_leases += 1;
        s.stats.subscriptions_created += 1;
        s.stats.active_subscriptions += 1;
        Ok(sub)
    }
    pub fn publish_state(&self, value: Value) -> Result<(), BoundaryError> {
        self.publish(Kind::State, value)
    }
    pub fn try_publish_event(&self, value: Value) -> Result<(), BoundaryError> {
        self.publish(Kind::Event, value)
    }
    fn publish(&self, kind: Kind, value: Value) -> Result<(), BoundaryError> {
        value.validate()?;
        let mut s = self.shared.lock();
        Shared::check_open(&s)?;
        // Keep every upgraded reference until after unlocking, including rejected
        // kinds/stopped subscriptions: a final Arc drop runs resource cleanup.
        let registered: Vec<_> = s.subscriptions.iter().filter_map(Weak::upgrade).collect();
        let subscribers: Vec<_> = registered
            .iter()
            .filter(|x| x.kind == kind && !x.state.lock().unwrap().stopping)
            .collect();
        let size = value.payload_bytes();
        let mut replaced = 0usize;
        let mut new_items = 0usize;
        for sub in &subscribers {
            if kind == Kind::State {
                if let Some(Item::Delivery(d)) = s
                    .queue
                    .iter()
                    .find(|q| matches!(q,Item::Delivery(d) if Arc::ptr_eq(&d.sub,sub)))
                {
                    replaced += d.value.payload_bytes();
                    continue;
                }
            }
            new_items += 1;
        }
        let after = s.stats.retained_bytes as usize - replaced + size * subscribers.len();
        if after > MAX_BYTES || (kind == Kind::Event && s.events + new_items > MAX_EVENTS) {
            drop(s);
            drop(subscribers);
            drop(registered);
            return Err(BoundaryError::ResourceExhausted);
        }
        for sub in subscribers {
            if kind == Kind::State {
                if let Some(Item::Delivery(d)) = s
                    .queue
                    .iter_mut()
                    .find(|q| matches!(q,Item::Delivery(d) if Arc::ptr_eq(&d.sub,sub)))
                {
                    d.value = value.clone();
                    s.stats.payloads_released += 1;
                    s.stats.payloads_admitted += 1;
                    continue;
                }
            }
            sub.state.lock().unwrap().queued += 1;
            s.queue.push_back(Item::Delivery(Delivery {
                sub: sub.clone(),
                value: value.clone(),
            }));
            s.stats.payloads_admitted += 1;
            if kind == Kind::Event {
                s.events += 1;
            }
        }
        s.stats.retained_bytes = after as u64;
        drop(s);
        drop(registered);
        self.shared.notify();
        Ok(())
    }
    pub fn statistics(&self) -> Statistics {
        let s = self.shared.lock();
        let mut stats = s.stats.clone();
        stats.queued_items = s.queue.len() as u64;
        stats
    }
    pub fn request_close(&self) {
        request_close(&self.shared);
    }
    pub fn await_closed(&self) -> impl Future<Output = Result<(), BoundaryError>> + Send + '_ {
        Wait::new(self.shared.clone(), move || {
            let s = self.shared.lock();
            if self.shared.is_callback() {
                Poll::Ready(Err(BoundaryError::WouldDeadlock))
            } else if s.life == Lifecycle::Closed {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.request_close();
    }
}

pub struct Operation {
    shared: Arc<Shared>,
    state: Mutex<OpState>,
}
impl Operation {
    pub fn request_cancel(&self) {
        let mut s = self.shared.lock();
        let mut op = self.state.lock().unwrap();
        op.cancel = true;
        if op.finished && !op.consumed {
            if matches!(op.result, Some(Ok(_))) {
                s.stats.retained_bytes -= op.retained as u64;
                s.stats.payloads_released += 1;
                op.retained = 0;
            }
            op.result = Some(Err(BoundaryError::Cancelled));
        }
        drop(op);
        drop(s);
        self.shared.notify();
    }
    pub fn is_cancel_requested(&self) -> bool {
        let _s = self.shared.lock();
        self.state.lock().unwrap().cancel
    }
    /// Cancellation of this waiter removes only its waker. Work ownership stays with the operation.
    pub fn result(&self) -> impl Future<Output = Result<Value, BoundaryError>> + Send + '_ {
        Wait::new(self.shared.clone(), move || {
            let mut s = self.shared.lock();
            let mut op = self.state.lock().unwrap();
            if self.shared.is_callback() {
                return Poll::Ready(Err(BoundaryError::WouldDeadlock));
            }
            if op.consumed {
                return Poll::Ready(Err(BoundaryError::AlreadyConsumed));
            }
            if let Some(result) = op.result.take() {
                op.consumed = true;
                if result.is_ok() {
                    s.stats.retained_bytes -= op.retained as u64;
                    s.stats.payloads_released += 1;
                    op.retained = 0;
                }
                Poll::Ready(result)
            } else {
                Poll::Pending
            }
        })
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        let mut s = self.shared.lock();
        let op = self.state.lock().unwrap();
        if op.retained > 0 || matches!(op.result, Some(Ok(_))) {
            s.stats.retained_bytes -= op.retained as u64;
            s.stats.payloads_released += 1;
        }
        s.stats.operations_released += 1;
        drop(op);
        drop(s);
        self.shared.notify();
    }
}

pub struct Subscription {
    shared: Arc<Shared>,
    kind: Kind,
    state: Arc<Mutex<SubState>>,
}
impl Subscription {
    pub fn request_stop(&self) {
        let mut s = self.shared.lock();
        let mut sub = self.state.lock().unwrap();
        stop_sub(&mut s, &mut sub);
        queue_cleanup(&mut s, &self.state, &mut sub);
        drop(sub);
        drop(s);
        self.shared.notify();
    }
    pub fn await_stopped(&self) -> impl Future<Output = Result<(), BoundaryError>> + Send + '_ {
        Wait::new(self.shared.clone(), move || {
            let _s = self.shared.lock();
            let sub = self.state.lock().unwrap();
            if self.shared.is_callback() {
                Poll::Ready(Err(BoundaryError::WouldDeadlock))
            } else if sub.stopping
                && sub.queued == 0
                && !sub.running
                && !sub.cleaning
                && sub.sink.is_none()
            {
                Poll::Ready(sub.error.map_or(Ok(()), Err))
            } else {
                Poll::Pending
            }
        })
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        let mut s = self.shared.lock();
        let mut sub = self.state.lock().unwrap();
        stop_sub(&mut s, &mut sub);
        queue_cleanup(&mut s, &self.state, &mut sub);
        s.stats.subscriptions_released += 1;
        drop(sub);
        drop(s);
        self.shared.notify();
    }
}
fn stop_sub(s: &mut State, sub: &mut SubState) {
    if !sub.stopping {
        sub.stopping = true;
        s.stats.active_subscriptions -= 1;
    }
}
// The state lease is also the subscription capacity token. A caller may drop
// its handle while cleanup runs, but that slot is not reusable until cleanup ends.
fn queue_cleanup(s: &mut State, state: &Arc<Mutex<SubState>>, sub: &mut SubState) {
    if sub.stopping && sub.queued == 0 && !sub.running {
        if let Some(sink) = sub.sink.take() {
            sub.cleaning = true;
            s.queue.push_back(Item::Cleanup(Cleanup {
                state: state.clone(),
                sink,
            }));
        }
    }
}
fn run_cleanup(shared: &Shared, cleanup: Cleanup) {
    let Cleanup { state, sink } = cleanup;
    // Foreign destructors are user code: run on the owned worker, without the
    // protocol lock, and retain the source lease until destruction has returned.
    let result = in_callback(shared, || drop(sink));
    let mut s = shared.lock();
    let mut sub = state.lock().unwrap();
    if let Err(error) = result {
        sub.error = Some(error);
    }
    sub.cleaning = false;
    s.source_leases -= 1;
    drop(sub);
    drop(s);
    drop(state);
}
fn request_close(shared: &Arc<Shared>) {
    let mut s = shared.lock();
    if s.life != Lifecycle::Open {
        return;
    }
    s.life = Lifecycle::Closing;
    let subscriptions: Vec<_> = s.subscriptions.iter().filter_map(Weak::upgrade).collect();
    for sub in &subscriptions {
        let mut st = sub.state.lock().unwrap();
        stop_sub(&mut s, &mut st);
        queue_cleanup(&mut s, &sub.state, &mut st);
    }
    let operations: Vec<_> = s.operations.iter().filter_map(Weak::upgrade).collect();
    for operation in &operations {
        let mut op = operation.state.lock().unwrap();
        op.cancel = true;
        if op.finished && !op.consumed {
            if matches!(op.result, Some(Ok(_))) {
                s.stats.retained_bytes -= op.retained as u64;
                s.stats.payloads_released += 1;
                op.retained = 0;
            }
            op.result = Some(Err(BoundaryError::Cancelled));
        }
    }
    drop(s);
    drop(subscriptions);
    drop(operations);
    shared.notify();
}
fn run_worker(shared: Arc<Shared>) {
    loop {
        let mut s = shared.lock();
        // A final handle may have reached strong_count == 0 before its Drop
        // acquires this mutex. Source leases and retained payloads prevent close
        // from overtaking that deferred cleanup merely because Weak::upgrade failed.
        while s.queue.is_empty()
            && (s.life == Lifecycle::Open || s.source_leases > 0 || s.stats.retained_bytes > 0)
        {
            s = shared.ready.wait(s).unwrap_or_else(|e| e.into_inner());
        }
        let Some(item) = s.queue.pop_front() else {
            s.life = Lifecycle::Closed;
            s.stats.worker_exits += 1;
            drop(s);
            shared.notify();
            break;
        };
        drop(s);
        match item {
            Item::Delivery(d) => deliver(&shared, d),
            Item::Job(job) => run_job(&shared, job),
            Item::Cleanup(cleanup) => run_cleanup(&shared, cleanup),
        }
        shared.notify();
    }
}
fn in_callback<T>(shared: &Shared, f: impl FnOnce() -> T) -> Result<T, BoundaryError> {
    CALLBACK_INSTANCE.with(|cell| {
        let previous = cell.replace(shared as *const _ as usize);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .map_err(|_| BoundaryError::CallbackFailure);
        cell.set(previous);
        result
    })
}
fn deliver(shared: &Arc<Shared>, delivery: Delivery) {
    let Delivery { sub, value } = delivery;
    let bytes = value.payload_bytes();
    let mut s = shared.lock();
    let mut st = sub.state.lock().unwrap();
    st.queued -= 1;
    if sub.kind == Kind::Event {
        s.events -= 1;
    }
    let sink = if st.stopping {
        None
    } else {
        st.running = true;
        s.stats.callbacks_in_flight += 1;
        st.sink.clone()
    };
    drop(st);
    drop(s);
    let outcome = sink
        .as_ref()
        .map(|sink| in_callback(shared, || sink.on_value(value)).and_then(|x| x));
    // Drop the invoking source lease before publishing the drain condition.
    drop(sink);
    let mut s = shared.lock();
    let mut st = sub.state.lock().unwrap();
    if st.running {
        st.running = false;
        s.stats.callbacks_in_flight -= 1;
    }
    if let Some(Err(error)) = outcome {
        st.error = Some(error);
        stop_sub(&mut s, &mut st);
    }
    s.stats.retained_bytes -= bytes as u64;
    s.stats.payloads_released += 1;
    queue_cleanup(&mut s, &sub.state, &mut st);
    drop(st);
    drop(s);
    drop(sub);
}
fn run_job(shared: &Arc<Shared>, job: Job) {
    let Job { op, host, value } = job;
    let input_bytes = value.payload_bytes();
    let mut s = shared.lock();
    let cancelled = op.state.lock().unwrap().cancel;
    if !cancelled {
        s.stats.callbacks_in_flight += 1;
    }
    drop(s);
    let result = if cancelled {
        Err(BoundaryError::Cancelled)
    } else {
        in_callback(shared, || host.transform(value)).and_then(|r| r)
    };
    // Final Host release is foreign code too. Preserve worker reentry diagnostics
    // and contain a Rust destructor panic before publishing finished/drained work.
    let cleanup = in_callback(shared, || drop(host));
    let result = result.and_then(|value| cleanup.map(|()| value));
    let mut s = shared.lock();
    let mut st = op.state.lock().unwrap();
    if !cancelled {
        s.stats.callbacks_in_flight -= 1;
    }
    s.stats.active_operations -= 1;
    s.stats.retained_bytes -= input_bytes as u64;
    s.stats.payloads_released += 1;
    st.retained = 0;
    let result = if st.cancel {
        Err(BoundaryError::Cancelled)
    } else {
        result.and_then(|v| {
            v.validate()?;
            if s.stats.retained_bytes + v.payload_bytes() as u64 > MAX_BYTES as u64 {
                Err(BoundaryError::ResourceExhausted)
            } else {
                Ok(v)
            }
        })
    };
    if let Ok(value) = &result {
        st.retained = value.payload_bytes();
        s.stats.retained_bytes += st.retained as u64;
        s.stats.payloads_admitted += 1;
    }
    st.result = Some(result);
    st.finished = true;
    drop(st);
    drop(s);
    drop(op);
}
