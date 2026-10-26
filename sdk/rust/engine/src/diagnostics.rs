//! Image-local diagnostics. This state never owns native resources or callbacks.
use super::*;
use std::{
    sync::{
        Mutex, TryLockError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

const CAPACITY: usize = 64;
const EMPTY: MadbDiagnosticEvent = MadbDiagnosticEvent {
    record_id: 0,
    symbol_id: 0,
    phase: 0,
    resource_id: 0,
    related_resource_id: 0,
    duration_ns: 0,
    status: 0,
    status_present: 0,
};

struct Ring {
    events: [MadbDiagnosticEvent; CAPACITY],
    head: usize,
    len: usize,
    next_id: Option<u64>,
    enabled: bool,
}

// The ring lock protects configuration, FIFO and record IDs. The atomic enabled
// flag is only an early sampling hint: enqueue rechecks under this same lock.
// No registry lock or external code is acquired/called while holding it.
struct Diagnostics {
    ring: Mutex<Ring>,
    enabled: AtomicBool,
    completed_calls: AtomicU64,
    failed_calls: AtomicU64,
    completed_callbacks: AtomicU64,
    dropped_events: AtomicU64,
}
impl Diagnostics {
    const fn new() -> Self {
        Self {
            ring: Mutex::new(Ring {
                events: [EMPTY; CAPACITY],
                head: 0,
                len: 0,
                next_id: Some(1),
                enabled: false,
            }),
            enabled: AtomicBool::new(false),
            completed_calls: AtomicU64::new(0),
            failed_calls: AtomicU64::new(0),
            completed_callbacks: AtomicU64::new(0),
            dropped_events: AtomicU64::new(0),
        }
    }

    fn begin(&self) -> Option<Instant> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        // Even a clock failure belongs only to diagnostics, not the business call.
        match catch_unwind(Instant::now) {
            Ok(started) => Some(started),
            Err(_) => {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    fn configure(&self, enabled: bool) -> MadbStatus {
        let mut ring = match self.ring.try_lock() {
            Ok(ring) => ring,
            Err(TryLockError::WouldBlock) => return MADB_BUSY,
            Err(TryLockError::Poisoned(_)) => return MADB_INTERNAL_FAILURE,
        };
        ring.enabled = enabled;
        self.enabled.store(enabled, Ordering::Relaxed);
        MADB_OK
    }

    fn stats(&self, enabled: bool) -> MadbDiagnosticStats {
        // Separate atomic samples, never a coherent resource/correctness snapshot.
        MadbDiagnosticStats {
            completed_calls: self.completed_calls.load(Ordering::Relaxed),
            failed_calls: self.failed_calls.load(Ordering::Relaxed),
            completed_callbacks: self.completed_callbacks.load(Ordering::Relaxed),
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
            capture_enabled: u64::from(enabled),
        }
    }

    fn drain(&self) -> Result<(MadbDiagnosticEvent, u64, MadbDiagnosticStats), MadbStatus> {
        let mut ring = self.ring.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => MADB_BUSY,
            TryLockError::Poisoned(_) => MADB_INTERNAL_FAILURE,
        })?;
        let (event, present) = if ring.len == 0 {
            (EMPTY, 0)
        } else {
            let head = ring.head;
            let event = ring.events[head];
            ring.events[head] = EMPTY;
            ring.head = (head + 1) % CAPACITY;
            ring.len -= 1;
            (event, 1)
        };
        Ok((event, present, self.stats(ring.enabled)))
    }

    fn record(&self, started: Option<Instant>, mut event: MadbDiagnosticEvent) {
        let Some(started) = started else { return };
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        // Clock/record failures cannot replace the original status or callback.
        // Timing happens outside the lock; no allocations or callbacks occur inside.
        let attempted = catch_unwind(AssertUnwindSafe(|| {
            event.duration_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let Ok(mut ring) = self.ring.try_lock() else {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                return;
            };
            if !ring.enabled {
                return;
            }
            if ring.len == CAPACITY || ring.next_id.is_none() {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let id = ring.next_id.expect("checked record namespace");
            event.record_id = id;
            ring.next_id = id.checked_add(1);
            let tail = (ring.head + ring.len) % CAPACITY;
            ring.events[tail] = event;
            ring.len += 1;
        }));
        if attempted.is_err() {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn complete_call(
        &self,
        started: Option<Instant>,
        symbol: u64,
        resource: u64,
        related: u64,
        status: MadbStatus,
    ) {
        self.completed_calls.fetch_add(1, Ordering::Relaxed);
        if status != MADB_OK {
            self.failed_calls.fetch_add(1, Ordering::Relaxed);
        }
        self.record(
            started,
            MadbDiagnosticEvent {
                symbol_id: symbol,
                phase: 1,
                resource_id: resource,
                related_resource_id: related,
                status: i64::from(status),
                status_present: 1,
                ..EMPTY
            },
        );
    }

    fn complete_callback(&self, started: Option<Instant>, subscription: u64, parent: u64) {
        self.completed_callbacks.fetch_add(1, Ordering::Relaxed);
        self.record(
            started,
            MadbDiagnosticEvent {
                symbol_id: 8,
                phase: 2,
                resource_id: subscription,
                related_resource_id: parent,
                ..EMPTY
            },
        );
    }
}

// Const initialization avoids a first-call OnceLock wait on the diagnostics path.
static STATE: Diagnostics = Diagnostics::new();

pub(super) fn begin() -> Option<Instant> {
    STATE.begin()
}
pub(super) fn configure(enabled: bool) -> MadbStatus {
    STATE.configure(enabled)
}
pub(super) fn drain() -> Result<(MadbDiagnosticEvent, u64, MadbDiagnosticStats), MadbStatus> {
    STATE.drain()
}
pub(super) fn complete_call(
    started: Option<Instant>,
    symbol: u64,
    resource: u64,
    related: u64,
    status: MadbStatus,
) {
    STATE.complete_call(started, symbol, resource, related, status);
}
pub(super) fn complete_callback(started: Option<Instant>, subscription: u64, parent: u64) {
    STATE.complete_callback(started, subscription, parent);
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
