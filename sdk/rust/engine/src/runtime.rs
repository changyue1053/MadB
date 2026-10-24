use super::*;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    sync::{Condvar, Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

type Callback = unsafe extern "C" fn(*mut c_void, u64, u32);

#[derive(Clone, Copy)]
struct CallbackContext(*mut c_void);
// SAFETY: C's registration contract keeps context alive through drain. A source
// lease serializes all calls for one registration, and all transfers synchronize
// through the registry mutex. The caller must permit the declared source threads.
unsafe impl Send for CallbackContext {}

#[derive(Clone, Copy)]
struct Target {
    callback: Callback,
    context: CallbackContext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Open,
    Closing,
    Closed,
}

struct NativeEngine {
    _initial_state: u32,
}
struct Engine {
    phase: Phase,
    operations: u64,
    children: BTreeSet<MadbHandle>,
    native: Option<Box<NativeEngine>>,
}
struct Subscription {
    phase: Phase,
    parent: MadbHandle,
    target: Option<Target>,
    source_leases: u64,
    next_sequence: Option<u64>,
}
enum Record {
    Engine(Engine),
    Subscription(Subscription),
}
impl Record {
    fn phase(&self) -> Phase {
        match self {
            Self::Engine(e) => e.phase,
            Self::Subscription(s) => s.phase,
        }
    }
}

struct Registry {
    next_token: Option<u64>,
    records: BTreeMap<MadbHandle, Record>,
    #[cfg(feature = "test-support")]
    counters: test_support::MadbTestCounts,
}
struct Shared {
    state: Mutex<Registry>,
    changed: Condvar,
}

// This registry belongs to one loaded SDK image; it is not a global business
// engine or executor. All type checks, admission and close decisions use its one
// mutex. No user callback runs while holding it. Native payload drops are trivial
// Rust-only drops, completed under the mutex before publishing Closed.
fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        state: Mutex::new(Registry {
            next_token: Some(1),
            records: BTreeMap::new(),
            #[cfg(feature = "test-support")]
            counters: Default::default(),
        }),
        changed: Condvar::new(),
    })
}
fn lock() -> Result<MutexGuard<'static, Registry>, MadbStatus> {
    shared().state.lock().map_err(|_| MADB_INTERNAL_FAILURE)
}
fn phase_status(phase: Phase) -> MadbStatus {
    match phase {
        Phase::Open => MADB_OK,
        Phase::Closing => MADB_CLOSING,
        Phase::Closed => MADB_CLOSED,
    }
}
impl Registry {
    fn allocate(&mut self) -> Result<MadbHandle, MadbStatus> {
        if self.records.len() >= MADB_MAX_HANDLES as usize {
            return Err(MADB_RESOURCE_EXHAUSTED);
        }
        let token = self.next_token.ok_or(MADB_RESOURCE_EXHAUSTED)?;
        self.next_token = token.checked_add(1);
        Ok(token)
    }
    fn engine_open(&mut self, handle: MadbHandle) -> Result<&mut Engine, MadbStatus> {
        let Some(Record::Engine(engine)) = self.records.get_mut(&handle) else {
            return Err(MADB_INVALID_HANDLE);
        };
        if engine.phase == Phase::Open {
            Ok(engine)
        } else {
            Err(phase_status(engine.phase))
        }
    }
    fn close_subscription(&mut self, handle: MadbHandle) {
        if let Some(Record::Subscription(s)) = self.records.get_mut(&handle) {
            if s.phase == Phase::Open {
                s.phase = Phase::Closing;
            }
            if s.phase == Phase::Closing && s.source_leases == 0 {
                s.target.take(); // Revoke the borrowed context before confirming drain.
                s.phase = Phase::Closed;
                #[cfg(feature = "test-support")]
                {
                    self.counters.subscriptions_released += 1;
                }
            }
        }
    }
    fn drain_engine(&mut self, handle: MadbHandle) {
        let ready = match self.records.get(&handle) {
            Some(Record::Engine(e)) if e.phase == Phase::Closing && e.operations == 0 => e.children.iter().all(|child| {
                matches!(self.records.get(child), Some(Record::Subscription(s)) if s.phase == Phase::Closed)
            }),
            _ => false,
        };
        if ready && let Some(Record::Engine(e)) = self.records.get_mut(&handle) {
            drop(e.native.take());
            e.phase = Phase::Closed;
            #[cfg(feature = "test-support")]
            {
                self.counters.engines_released += 1;
            }
        }
    }
    fn close(&mut self, handle: MadbHandle) -> MadbStatus {
        match self.records.get(&handle) {
            None => MADB_INVALID_HANDLE,
            Some(Record::Subscription(s)) => {
                let parent = s.parent;
                self.close_subscription(handle);
                self.drain_engine(parent);
                MADB_OK
            }
            Some(Record::Engine(e)) => {
                let children = e.children.iter().copied().collect::<Vec<_>>();
                if let Some(Record::Engine(e)) = self.records.get_mut(&handle)
                    && e.phase == Phase::Open
                {
                    e.phase = Phase::Closing;
                }
                for child in children {
                    self.close_subscription(child);
                }
                self.drain_engine(handle);
                MADB_OK
            }
        }
    }
}

thread_local! { static CALLBACKS: RefCell<Vec<(MadbHandle, MadbHandle)>> = const { RefCell::new(Vec::new()) }; }
fn in_callback(handle: MadbHandle) -> bool {
    CALLBACKS.with(|calls| {
        calls
            .borrow()
            .iter()
            .any(|(engine, subscription)| handle == *engine || handle == *subscription)
    })
}
struct CallbackScope;
impl CallbackScope {
    fn enter(engine: MadbHandle, subscription: MadbHandle) -> Self {
        CALLBACKS.with(|calls| calls.borrow_mut().push((engine, subscription)));
        Self
    }
}
impl Drop for CallbackScope {
    fn drop(&mut self) {
        CALLBACKS.with(|calls| {
            calls.borrow_mut().pop();
        });
    }
}

pub(super) fn create() -> Result<MadbHandle, MadbStatus> {
    let mut state = lock()?;
    let handle = state.allocate()?;
    state.records.insert(
        handle,
        Record::Engine(Engine {
            phase: Phase::Open,
            operations: 0,
            children: BTreeSet::new(),
            native: Some(Box::new(NativeEngine { _initial_state: 0 })),
        }),
    );
    #[cfg(feature = "test-support")]
    {
        state.counters.engines_created += 1;
    }
    Ok(handle)
}

pub(super) struct Operation {
    engine: MadbHandle,
}
pub(super) fn operation(engine: MadbHandle) -> Result<Operation, MadbStatus> {
    let mut state = lock()?;
    let e = state.engine_open(engine)?;
    e.operations = e.operations.checked_add(1).ok_or(MADB_RESOURCE_EXHAUSTED)?;
    Ok(Operation { engine })
}
impl Drop for Operation {
    fn drop(&mut self) {
        // Poison retains the registry and resources in quarantine; no successful
        // close is fabricated when the synchronization invariant was lost.
        if let Ok(mut state) = lock() {
            if let Some(Record::Engine(e)) = state.records.get_mut(&self.engine) {
                e.operations -= 1;
            }
            state.drain_engine(self.engine);
            shared().changed.notify_all();
        }
    }
}

struct SourceLease {
    subscription: MadbHandle,
    parent: MadbHandle,
    target: Target,
    sequence: u64,
}
impl SourceLease {
    fn acquire(subscription: MadbHandle) -> Result<Self, MadbStatus> {
        let mut state = lock()?;
        let Some(Record::Subscription(s)) = state.records.get_mut(&subscription) else {
            return Err(MADB_INVALID_HANDLE);
        };
        if s.phase != Phase::Open {
            return Err(phase_status(s.phase));
        }
        if s.source_leases != 0 {
            return Err(if in_callback(subscription) {
                MADB_WOULD_DEADLOCK
            } else {
                MADB_BUSY
            });
        }
        let sequence = s.next_sequence.ok_or(MADB_RESOURCE_EXHAUSTED)?;
        s.next_sequence = sequence.checked_add(1);
        let target = s.target.ok_or(MADB_INTERNAL_FAILURE)?;
        s.source_leases = 1;
        Ok(Self {
            subscription,
            parent: s.parent,
            target,
            sequence,
        })
    }
    unsafe fn deliver(&self, value: u32) {
        // The lease was obtained before taking callback/context out of the
        // source registry. It remains alive through both this pause and callback.
        #[cfg(feature = "test-support")]
        test_support::pause(2);
        let _scope = CallbackScope::enter(self.parent, self.subscription);
        #[cfg(feature = "test-support")]
        if let Ok(mut state) = lock() {
            state.counters.callbacks_started += 1;
        }
        // SAFETY: caller's registration promise plus this source lease keep the
        // target/context alive; no registry lock is held across foreign code.
        unsafe { (self.target.callback)(self.target.context.0, self.sequence, value) };
        #[cfg(feature = "test-support")]
        if let Ok(mut state) = lock() {
            state.counters.callbacks_finished += 1;
        }
    }
}
impl Drop for SourceLease {
    fn drop(&mut self) {
        if let Ok(mut state) = lock() {
            if let Some(Record::Subscription(s)) = state.records.get_mut(&self.subscription) {
                s.source_leases -= 1;
            }
            if matches!(state.records.get(&self.subscription), Some(Record::Subscription(s)) if s.phase == Phase::Closing)
            {
                state.close_subscription(self.subscription);
            }
            state.drain_engine(self.parent);
            shared().changed.notify_all();
        }
    }
}

struct Registration {
    engine: MadbHandle,
    subscription: MadbHandle,
    transferred: bool,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if !self.transferred
            && let Ok(mut state) = lock()
        {
            state.close_subscription(self.subscription);
            // A registration guard is dropped after the local SourceLease.
            // This untransferred token has never been exposed to the caller.
            if matches!(state.records.get(&self.subscription), Some(Record::Subscription(s)) if s.phase == Phase::Closed)
            {
                state.records.remove(&self.subscription);
                if let Some(Record::Engine(e)) = state.records.get_mut(&self.engine) {
                    e.children.remove(&self.subscription);
                }
            }
            state.drain_engine(self.engine);
            shared().changed.notify_all();
        }
    }
}

pub(super) unsafe fn observe(
    engine: MadbHandle,
    callback: Callback,
    context: *mut c_void,
) -> Result<MadbHandle, MadbStatus> {
    let _operation = operation(engine)?;
    let subscription = {
        let mut state = lock()?;
        if state.engine_open(engine)?.children.len() >= MADB_MAX_SUBSCRIPTIONS_PER_ENGINE as usize {
            return Err(MADB_RESOURCE_EXHAUSTED);
        }
        let token = state.allocate()?;
        state.records.insert(
            token,
            Record::Subscription(Subscription {
                phase: Phase::Open,
                parent: engine,
                target: Some(Target {
                    callback,
                    context: CallbackContext(context),
                }),
                source_leases: 0,
                next_sequence: Some(1),
            }),
        );
        state.engine_open(engine)?.children.insert(token);
        #[cfg(feature = "test-support")]
        {
            state.counters.subscriptions_created += 1;
        }
        token
    };
    let mut registration = Registration {
        engine,
        subscription,
        transferred: false,
    };
    #[cfg(feature = "test-support")]
    if test_support::take_fault(1) {
        return Err(MADB_INTERNAL_FAILURE);
    }
    let source = match SourceLease::acquire(subscription) {
        Ok(source) => source,
        Err(status) => {
            // An engine close can win between installation and initial-source
            // admission. Report the parent operation's Closing state; this
            // unpublished subscription may already have finished its own drain.
            lock()?.engine_open(engine)?;
            return Err(status);
        }
    };
    // SAFETY: this call inherits observe's registration contract.
    unsafe { source.deliver(0) };
    drop(source);
    let mut state = lock()?;
    // The registration operation prevents engine Closed while this check runs.
    state.engine_open(engine)?;
    registration.transferred = true;
    Ok(subscription)
}

pub(super) fn request_close(handle: MadbHandle) -> MadbStatus {
    match lock() {
        Ok(mut state) => {
            let status = state.close(handle);
            shared().changed.notify_all();
            status
        }
        Err(status) => status,
    }
}
pub(super) fn await_closed(handle: MadbHandle, timeout_ms: u32) -> MadbStatus {
    let mut state = match lock() {
        Ok(state) => state,
        Err(status) => return status,
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    loop {
        let Some(record) = state.records.get(&handle) else {
            return MADB_INVALID_HANDLE;
        };
        if in_callback(handle) {
            return MADB_WOULD_DEADLOCK;
        }
        match record.phase() {
            Phase::Closed => return MADB_OK,
            Phase::Open => return MADB_BUSY,
            Phase::Closing => {}
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return MADB_TIMEOUT;
        };
        match shared().changed.wait_timeout(state, remaining) {
            Ok((next, _)) => state = next,
            Err(_) => return MADB_INTERNAL_FAILURE,
        }
    }
}
pub(super) fn release_handle(handle: MadbHandle) -> MadbStatus {
    let mut state = match lock() {
        Ok(state) => state,
        Err(status) => return status,
    };
    match state.records.get(&handle) {
        None => return MADB_INVALID_HANDLE,
        Some(record) if record.phase() != Phase::Closed => return MADB_BUSY,
        _ => {}
    }
    if let Some(Record::Subscription(s)) = state.records.remove(&handle) {
        if let Some(Record::Engine(e)) = state.records.get_mut(&s.parent) {
            e.children.remove(&handle);
        }
        state.drain_engine(s.parent);
    }
    shared().changed.notify_all();
    MADB_OK
}

#[cfg(feature = "test-support")]
pub(super) unsafe fn emit(subscription: MadbHandle, value: u32) -> MadbStatus {
    let source = match SourceLease::acquire(subscription) {
        Ok(source) => source,
        Err(status) => return status,
    };
    // SAFETY: the source lease retains the exact registered target until return.
    unsafe { source.deliver(value) };
    MADB_OK
}
#[cfg(feature = "test-support")]
pub(super) fn snapshot() -> Result<test_support::MadbTestCounts, MadbStatus> {
    let state = lock()?;
    let mut result = state.counters;
    result.registry_entries = state.records.len() as u64;
    for record in state.records.values() {
        match record {
            Record::Engine(e) => result.active_operations += e.operations,
            Record::Subscription(s) => result.source_leases += s.source_leases,
        }
    }
    result.diagnostic_failures = test_support::diagnostic_failures();
    Ok(result)
}
#[cfg(feature = "test-support")]
pub(super) fn set_next_token(next: u64) -> MadbStatus {
    let mut state = match lock() {
        Ok(state) => state,
        Err(status) => return status,
    };
    if !state.records.is_empty() {
        return MADB_BUSY;
    }
    if next == 0 || state.next_token.is_none_or(|current| next < current) {
        return MADB_INVALID_ARGUMENT;
    }
    state.next_token = Some(next);
    MADB_OK
}
