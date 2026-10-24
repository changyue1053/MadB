//! Opt-in fixture controls. Gates surround real runtime leases; these controls do
//! not duplicate the lifecycle state machine and are absent from default builds.
use super::*;
use std::{
    sync::{
        Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MadbTestCounts {
    pub engines_created: u64,
    pub engines_released: u64,
    pub subscriptions_created: u64,
    pub subscriptions_released: u64,
    pub active_operations: u64,
    pub source_leases: u64,
    pub callbacks_started: u64,
    pub callbacks_finished: u64,
    pub registry_entries: u64,
    pub diagnostic_failures: u64,
}
#[derive(Default)]
struct Gate {
    armed: bool,
    reached: bool,
}
#[derive(Default)]
struct Controls {
    gates: [Gate; 2],
    faults: [bool; 3],
}
struct Shared {
    controls: Mutex<Controls>,
    changed: Condvar,
}
fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        controls: Mutex::new(Controls::default()),
        changed: Condvar::new(),
    })
}
fn index(point: u32) -> Option<usize> {
    match point {
        1 | 2 => Some((point - 1) as usize),
        _ => None,
    }
}

pub(super) fn pause(point: u32) {
    let index = index(point).expect("internal gate index");
    let mut controls = shared()
        .controls
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !controls.gates[index].armed {
        return;
    }
    controls.gates[index].reached = true;
    shared().changed.notify_all();
    while controls.gates[index].armed {
        controls = shared()
            .changed
            .wait(controls)
            .unwrap_or_else(|poison| poison.into_inner());
    }
}
pub(super) fn take_fault(fault: u32) -> bool {
    let mut controls = shared()
        .controls
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let value = controls.faults[(fault - 1) as usize];
    controls.faults[(fault - 1) as usize] = false;
    value
}
static DIAGNOSTIC_FAILURES: AtomicU64 = AtomicU64::new(0);
pub(super) fn diagnostic_failures() -> u64 {
    DIAGNOSTIC_FAILURES.load(Ordering::Relaxed)
}
pub(super) fn diagnostic() {
    let fail = shared()
        .controls
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .faults[2];
    // A diagnostic sink fails outside the protocol transition. Its failure never
    // changes admission, result, lease return, or the business status.
    if fail && catch_unwind(|| panic!("injected diagnostic sink failure")).is_err() {
        DIAGNOSTIC_FAILURES.fetch_add(1, Ordering::Relaxed);
    }
}

/// # Safety
/// out_error points to a writable, aligned MadbError for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_arm(
    point: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        let Some(index) = index(point) else {
            return MADB_INVALID_ARGUMENT;
        };
        let mut controls = shared()
            .controls
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if controls.gates[index].armed {
            return MADB_BUSY;
        }
        controls.gates[index] = Gate {
            armed: true,
            reached: false,
        };
        MADB_OK
    })
}
/// # Safety
/// out_error is aligned and writable throughout this bounded wait.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_wait(
    point: u32,
    timeout_ms: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        let Some(index) = index(point) else {
            return MADB_INVALID_ARGUMENT;
        };
        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        let mut controls = shared()
            .controls
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        loop {
            if controls.gates[index].reached {
                return MADB_OK;
            }
            if !controls.gates[index].armed {
                return MADB_BUSY;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return MADB_TIMEOUT;
            };
            controls = shared()
                .changed
                .wait_timeout(controls, remaining)
                .unwrap_or_else(|poison| poison.into_inner())
                .0;
        }
    })
}
/// # Safety
/// out_error is an aligned writable object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_resume(
    point: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        let Some(index) = index(point) else {
            return MADB_INVALID_ARGUMENT;
        };
        let mut controls = shared()
            .controls
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !controls.gates[index].armed {
            return MADB_BUSY;
        }
        controls.gates[index].armed = false;
        shared().changed.notify_all();
        MADB_OK
    })
}
/// # Safety
/// Outputs are disjoint, writable, aligned objects. Counts are a fixture snapshot
/// rather than a production lifetime or synchronization interface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_snapshot(
    out_counts: *mut MadbTestCounts,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: independently checked caller-owned outputs.
        let (counts_valid, error_valid) = unsafe { (clear(out_counts), clear(out_error)) };
        if !counts_valid || !error_valid {
            return MADB_INVALID_ARGUMENT;
        }
        match runtime::snapshot() {
            Ok(counts) => {
                // SAFETY: checked output remains valid throughout the call.
                unsafe { out_counts.write(counts) };
                MADB_OK
            }
            Err(status) => status,
        }
    })
}
/// # Safety
/// The subscription's original callback/context promises remain in force.
/// out_error is an aligned writable object. This call completes synchronously.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_emit(
    subscription: MadbHandle,
    state: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        // SAFETY: uses the registered target under the runtime's source lease.
        unsafe { runtime::emit(subscription, state) }
    })
}
/// # Safety
/// out_error is an aligned writable object. Faults 1/2 are consumed once; fault 3
/// stays enabled until disabled and is isolated from all protocol transitions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_set_fault(
    fault: u32,
    enabled: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        if !(1..=3).contains(&fault) || enabled > 1 {
            return MADB_INVALID_ARGUMENT;
        }
        shared()
            .controls
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .faults[(fault - 1) as usize] = enabled == 1;
        MADB_OK
    })
}
/// # Safety
/// out_error is an aligned writable object. This only moves the counter forward
/// while the registry is empty; it cannot revive an exhausted token namespace.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_test_set_next_token(
    next: u64,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned error output.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        runtime::set_next_token(next)
    })
}
