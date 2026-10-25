use madb_boundary_core::*;
use std::{
    future::Future,
    pin::pin,
    sync::{mpsc, Arc, Condvar, Mutex},
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};
struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark()
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark()
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let mut f = pin!(future);
    let w = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut cx = Context::from_waker(&w);
    let limit = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        assert!(Instant::now() < limit, "future did not drain");
        thread::park_timeout(Duration::from_millis(20));
    }
}
fn pending<F: Future>(future: std::pin::Pin<&mut F>) {
    let w = Waker::noop();
    assert!(future.poll(&mut Context::from_waker(w)).is_pending());
}
fn value(size: usize) -> Value {
    Value {
        id: String::new(),
        sequence: None,
        text: String::new(),
        bytes: vec![42; size],
        phase: Phase::Ready,
    }
}
struct Gate {
    entered: Mutex<Option<mpsc::SyncSender<()>>>,
    released: Mutex<bool>,
    ready: Condvar,
}
impl Gate {
    fn new() -> (Arc<Self>, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (
            Arc::new(Self {
                entered: Mutex::new(Some(tx)),
                released: Mutex::new(false),
                ready: Condvar::new(),
            }),
            rx,
        )
    }
    fn block(&self) {
        if let Some(tx) = self.entered.lock().unwrap().take() {
            tx.send(()).unwrap();
        }
        let mut r = self.released.lock().unwrap();
        while !*r {
            r = self.ready.wait(r).unwrap();
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.ready.notify_all();
    }
}
impl Host for Gate {
    fn transform(&self, v: Value) -> Result<Value, BoundaryError> {
        self.block();
        Ok(v)
    }
}
impl Sink for Gate {
    fn on_value(&self, _: Value) -> Result<(), BoundaryError> {
        self.block();
        Ok(())
    }
}
struct Echo;
impl Host for Echo {
    fn transform(&self, v: Value) -> Result<Value, BoundaryError> {
        Ok(v)
    }
}
fn engine() -> Engine {
    Engine::new(PROFILE_ID).unwrap()
}
fn close(e: &Engine) {
    e.request_close();
    run(e.await_closed()).unwrap();
    let s = e.statistics();
    assert_eq!(
        (
            s.queued_items,
            s.retained_bytes,
            s.callbacks_in_flight,
            s.active_operations,
            s.active_subscriptions,
            s.worker_exits
        ),
        (0, 0, 0, 0, 0, 1)
    );
    assert_eq!(s.payloads_admitted, s.payloads_released);
}
#[test]
fn values_identity_errors_and_owned_result() {
    assert!(matches!(
        Engine::new("wrong"),
        Err(BoundaryError::ProfileMismatch)
    ));
    let e = engine();
    let v = Value {
        id: "m1".into(),
        sequence: Some(i64::MIN),
        text: "你好\0é".into(),
        bytes: vec![0, 127, 128, 255],
        phase: Phase::Done,
    };
    assert_eq!(e.roundtrip(v.clone()).unwrap(), v);
    assert_eq!(
        e.roundtrip(value(1025)),
        Err(BoundaryError::ResourceExhausted)
    );
    let op = e.start_operation(v.clone(), Arc::new(Echo)).unwrap();
    assert_eq!(run(op.result()).unwrap(), v);
    assert_eq!(run(op.result()), Err(BoundaryError::AlreadyConsumed));
    drop(op);
    assert_eq!(e.statistics().operations_released, 1);
    close(&e);
    assert_eq!(e.roundtrip(value(1)), Err(BoundaryError::Closed));
}
#[test]
fn cancelling_waiter_does_not_cancel_work() {
    let e = engine();
    let (g, entered) = Gate::new();
    let op = e.start_operation(value(32), g.clone()).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    {
        let mut waiter = pin!(op.result());
        pending(waiter.as_mut());
    }
    assert!(!op.is_cancel_requested());
    g.release();
    assert_eq!(run(op.result()).unwrap().bytes.len(), 32);
    close(&e);
}
#[test]
fn cancel_queued_and_running_discards_late_result() {
    let e = engine();
    let (g, entered) = Gate::new();
    let running = e.start_operation(value(32), g.clone()).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let queued = e.start_operation(value(16), Arc::new(Echo)).unwrap();
    running.request_cancel();
    queued.request_cancel();
    let mut result = pin!(running.result());
    pending(result.as_mut());
    g.release();
    assert_eq!(run(result), Err(BoundaryError::Cancelled));
    assert_eq!(run(queued.result()), Err(BoundaryError::Cancelled));
    close(&e);
}
#[test]
fn event_item_budget_and_stop_drains_queue() {
    let e = engine();
    let (g, entered) = Gate::new();
    let sub = e.observe_events(g.clone()).unwrap();
    e.try_publish_event(value(1)).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..8 {
        e.try_publish_event(value(1)).unwrap();
    }
    assert_eq!(
        e.try_publish_event(value(1)),
        Err(BoundaryError::ResourceExhausted)
    );
    let stats = e.statistics();
    assert_eq!((stats.queued_items, stats.retained_bytes), (8, 9));
    sub.request_stop();
    let mut stop = pin!(sub.await_stopped());
    pending(stop.as_mut());
    e.request_close();
    let mut closed = pin!(e.await_closed());
    pending(closed.as_mut());
    g.release();
    run(stop).unwrap();
    run(closed).unwrap();
    close(&e);
}
#[test]
fn byte_budget_includes_inflight_and_no_transfer_on_rejection() {
    let e = engine();
    let (g, entered) = Gate::new();
    let sub = e.observe_events(g.clone()).unwrap();
    e.try_publish_event(value(1024)).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..3 {
        e.try_publish_event(value(1024)).unwrap();
    }
    let before = e.statistics();
    assert_eq!(before.retained_bytes, 4096);
    assert_eq!(
        e.try_publish_event(value(1)),
        Err(BoundaryError::ResourceExhausted)
    );
    assert_eq!(e.statistics(), before);
    sub.request_stop();
    g.release();
    run(sub.await_stopped()).unwrap();
    close(&e);
}
struct Collect(mpsc::SyncSender<Value>);
impl Sink for Collect {
    fn on_value(&self, v: Value) -> Result<(), BoundaryError> {
        self.0.send(v).unwrap();
        Ok(())
    }
}
#[test]
fn state_replaces_owned_payload_and_delivers_latest() {
    let e = engine();
    let (g, entered) = Gate::new();
    let op = e.start_operation(value(1), g.clone()).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let sub = e.observe_state(Arc::new(Collect(tx))).unwrap();
    for size in [3, 7, 9] {
        e.publish_state(value(size)).unwrap();
    }
    assert_eq!(e.statistics().queued_items, 1);
    assert_eq!(e.statistics().retained_bytes, 10);
    g.release();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap().bytes.len(),
        9
    );
    run(op.result()).unwrap();
    sub.request_stop();
    run(sub.await_stopped()).unwrap();
    close(&e);
}
struct Reentry {
    sub: Mutex<Option<Arc<Subscription>>>,
    result: mpsc::SyncSender<Result<(), BoundaryError>>,
}
impl Sink for Reentry {
    fn on_value(&self, _: Value) -> Result<(), BoundaryError> {
        let sub = self.sub.lock().unwrap().clone().unwrap();
        sub.request_stop();
        self.result.send(run(sub.await_stopped())).unwrap();
        Ok(())
    }
}
#[test]
fn callback_can_request_stop_but_cannot_wait_for_itself() {
    let e = engine();
    let (tx, rx) = mpsc::sync_channel(1);
    let sink = Arc::new(Reentry {
        sub: Mutex::new(None),
        result: tx,
    });
    let sub = e.observe_events(sink.clone()).unwrap();
    *sink.sub.lock().unwrap() = Some(sub.clone());
    e.try_publish_event(value(1)).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(BoundaryError::WouldDeadlock)
    );
    run(sub.await_stopped()).unwrap();
    sink.sub.lock().unwrap().take();
    close(&e);
}
struct RequestCloseFromCallback {
    engine: std::sync::Weak<Engine>,
    result: mpsc::SyncSender<Result<(), BoundaryError>>,
}
impl Sink for RequestCloseFromCallback {
    fn on_value(&self, _: Value) -> Result<(), BoundaryError> {
        let engine = self.engine.upgrade().unwrap();
        engine.request_close();
        // Direct polling is diagnosed; after returning, the external owner can drain.
        self.result.send(run(engine.await_closed())).unwrap();
        Ok(())
    }
}
#[test]
fn callback_requests_close_and_returns_before_external_owner_drains() {
    let engine = Arc::new(engine());
    let (tx, rx) = mpsc::sync_channel(1);
    let subscription = engine
        .observe_events(Arc::new(RequestCloseFromCallback {
            engine: Arc::downgrade(&engine),
            result: tx,
        }))
        .unwrap();
    engine.try_publish_event(value(1)).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(BoundaryError::WouldDeadlock)
    );
    run(engine.await_closed()).unwrap();
    run(subscription.await_stopped()).unwrap();
    let statistics = engine.statistics();
    assert_eq!(statistics.worker_exits, 1);
    assert_eq!(statistics.callbacks_in_flight, 0);
    assert_eq!(statistics.retained_bytes, 0);
}
struct DropGate(Arc<Gate>);
impl Sink for DropGate {
    fn on_value(&self, _: Value) -> Result<(), BoundaryError> {
        Ok(())
    }
}
impl Drop for DropGate {
    fn drop(&mut self) {
        self.0.block();
    }
}
#[test]
fn stop_and_close_requests_return_before_foreign_cleanup_finishes() {
    for close_request in [false, true] {
        let e = Arc::new(engine());
        let (gate, entered) = Gate::new();
        let sub = e.observe_events(Arc::new(DropGate(gate.clone()))).unwrap();
        let request_engine = e.clone();
        let request_sub = sub.clone();
        let (returned, received) = mpsc::sync_channel(1);
        let caller = thread::spawn(move || {
            if close_request {
                request_engine.request_close();
            } else {
                request_sub.request_stop();
            }
            returned.send(()).unwrap();
        });
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let returned_before_cleanup = received.recv_timeout(Duration::from_secs(2)).is_ok();
        if !returned_before_cleanup {
            gate.release();
            caller.join().unwrap();
            panic!("request executed foreign destructor on caller; close={close_request}");
        }
        caller.join().unwrap();
        let mut stopped = Box::pin(sub.await_stopped());
        pending(stopped.as_mut());
        e.request_close();
        let mut closed = pin!(e.await_closed());
        pending(closed.as_mut());
        gate.release();
        run(stopped).unwrap();
        run(closed).unwrap();
        drop(sub);
        let stats = e.statistics();
        assert_eq!(stats.worker_exits, 1);
        assert_eq!(stats.queued_items, 0);
        assert_eq!(stats.retained_bytes, 0);
        assert_eq!(stats.subscriptions_created, stats.subscriptions_released);
    }
}
#[test]
fn cleanup_tickets_keep_subscription_capacity_until_destructor_returns() {
    let e = engine();
    let (gate, entered) = Gate::new();
    let first = e.observe_events(Arc::new(DropGate(gate.clone()))).unwrap();
    let (values, _receiver) = mpsc::sync_channel(1);
    let mut rest: Vec<_> = (0..7)
        .map(|_| e.observe_events(Arc::new(Collect(values.clone()))).unwrap())
        .collect();
    first.request_stop();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(first);
    for sub in &rest {
        sub.request_stop();
    }
    let last = rest.pop().unwrap();
    drop(rest);
    assert_eq!(e.statistics().queued_items, 7);
    assert!(matches!(
        e.observe_events(Arc::new(Collect(values.clone()))),
        Err(BoundaryError::ResourceExhausted)
    ));
    let mut stopped = Box::pin(last.await_stopped());
    pending(stopped.as_mut());
    gate.release();
    run(stopped).unwrap();
    // Awaiting the final FIFO cleanup also proves the earlier orphaned tickets drained.
    let replacement = e.observe_events(Arc::new(Collect(values))).unwrap();
    close(&e);
    drop(replacement);
    drop(last);
    let stats = e.statistics();
    assert_eq!(stats.queued_items, 0);
    assert_eq!(stats.active_subscriptions, 0);
    assert_eq!(stats.subscriptions_created, stats.subscriptions_released);
}
#[test]
fn bounded_waiters_release_registration_on_wait_cancel() {
    let e = engine();
    let mut waiters: Vec<_> = (0..32).map(|_| Box::pin(e.await_closed())).collect();
    for w in &mut waiters {
        pending(w.as_mut());
    }
    assert_eq!(run(e.await_closed()), Err(BoundaryError::ResourceExhausted));
    drop(waiters);
    close(&e);
}

#[test]
fn waker_clone_drop_and_wake_can_reenter_instance() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{RawWaker, RawWakerVTable};
    struct Reenter {
        engine: Arc<Engine>,
        clones: AtomicUsize,
        drops: AtomicUsize,
    }
    // Each raw waker owns one Arc. The vtable intentionally enters the public SDK
    // on every clone/drop, so holding Shared.state across either action deadlocks.
    unsafe fn clone(data: *const ()) -> RawWaker {
        let state = &*data.cast::<Reenter>();
        state.engine.statistics();
        state.clones.fetch_add(1, Ordering::SeqCst);
        Arc::increment_strong_count(data.cast::<Reenter>());
        RawWaker::new(data, &TABLE)
    }
    unsafe fn wake(data: *const ()) {
        let state = Arc::from_raw(data.cast::<Reenter>());
        state.engine.statistics();
    }
    unsafe fn wake_ref(data: *const ()) {
        (&*data.cast::<Reenter>()).engine.statistics();
    }
    unsafe fn release(data: *const ()) {
        let state = Arc::from_raw(data.cast::<Reenter>());
        state.engine.statistics();
        state.drops.fetch_add(1, Ordering::SeqCst);
    }
    static TABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_ref, release);
    let e = Arc::new(engine());
    let state = Arc::new(Reenter {
        engine: e.clone(),
        clones: AtomicUsize::new(0),
        drops: AtomicUsize::new(0),
    });
    let raw = RawWaker::new(Arc::into_raw(state.clone()).cast(), &TABLE);
    let waker = unsafe { Waker::from_raw(raw) };
    let mut cx = Context::from_waker(&waker);
    {
        let mut waiter = Box::pin(e.await_closed());
        assert!(waiter.as_mut().poll(&mut cx).is_pending());
        assert!(waiter.as_mut().poll(&mut cx).is_pending()); // replacement drop
    } // cancellation removes a registered waker
    let mut waiter = Box::pin(e.await_closed());
    assert!(waiter.as_mut().poll(&mut cx).is_pending());
    e.request_close(); // notification clones Arc<Waker>, then invokes it unlocked
    run(e.await_closed()).unwrap();
    assert!(waiter.as_mut().poll(&mut cx).is_ready()); // ready removal
    drop(waiter);
    assert!(state.clones.load(Ordering::SeqCst) >= 4);
    assert!(state.drops.load(Ordering::SeqCst) >= 4);
}

struct HostDropReentry {
    engine: std::sync::Weak<Engine>,
    result: mpsc::SyncSender<Result<(), BoundaryError>>,
    panic_on_drop: bool,
}
impl Host for HostDropReentry {
    fn transform(&self, value: Value) -> Result<Value, BoundaryError> {
        Ok(value)
    }
}
impl Drop for HostDropReentry {
    fn drop(&mut self) {
        let engine = self.engine.upgrade().unwrap();
        self.result.send(run(engine.await_closed())).unwrap();
        assert!(
            !self.panic_on_drop,
            "simulated foreign owner destructor failure"
        );
    }
}
#[test]
fn host_destructor_keeps_worker_reentry_guard_and_cannot_kill_worker() {
    for panic_on_drop in [false, true] {
        let engine = Arc::new(engine());
        let (sender, receiver) = mpsc::sync_channel(1);
        let operation = engine
            .start_operation(
                value(1),
                Arc::new(HostDropReentry {
                    engine: Arc::downgrade(&engine),
                    result: sender,
                    panic_on_drop,
                }),
            )
            .unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(BoundaryError::WouldDeadlock)
        );
        let result = run(operation.result());
        if panic_on_drop {
            assert_eq!(result, Err(BoundaryError::CallbackFailure));
        } else {
            assert_eq!(result, Ok(value(1)));
        }
        close(&engine);
        let statistics = engine.statistics();
        assert_eq!(statistics.worker_exits, 1);
        assert_eq!(statistics.active_operations, 0);
        assert_eq!(statistics.callbacks_in_flight, 0);
        assert_eq!(statistics.retained_bytes, 0);
    }
}
