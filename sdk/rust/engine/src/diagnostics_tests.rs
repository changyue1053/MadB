use super::*;

fn call(state: &Diagnostics, resource: u64, status: MadbStatus) {
    state.complete_call(state.begin(), 3, resource, 0, status);
}

#[test]
fn capture_off_keeps_only_necessary_counts() {
    let state = Diagnostics::new();
    assert!(state.begin().is_none());
    call(&state, 7, MADB_OK);
    call(&state, 7, MADB_CLOSED);
    state.complete_callback(state.begin(), 9, 7);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!(event, EMPTY);
    assert_eq!(present, 0);
    assert_eq!(
        stats,
        MadbDiagnosticStats {
            completed_calls: 2,
            failed_calls: 1,
            completed_callbacks: 1,
            dropped_events: 0,
            capture_enabled: 0,
        }
    );
}

#[test]
fn fixed_ring_drops_new_preserves_fifo_and_reuses_slots() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    for resource in 0..70 {
        call(&state, resource, MADB_OK);
    }
    for expected in 0..64 {
        let (event, present, stats) = state.drain().unwrap();
        assert_eq!(present, 1);
        assert_eq!(event.record_id, expected + 1);
        assert_eq!(event.resource_id, expected);
        assert_eq!(
            (
                event.symbol_id,
                event.phase,
                event.status_present,
                event.status
            ),
            (3, 1, 1, 0)
        );
        assert_eq!(stats.dropped_events, 6);
    }
    let (event, present, _) = state.drain().unwrap();
    assert_eq!((event, present), (EMPTY, 0));
    call(&state, 99, MADB_CLOSED);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!(present, 1);
    assert_eq!(
        (event.record_id, event.resource_id, event.status),
        (65, 99, i64::from(MADB_CLOSED))
    );
    assert_eq!(stats.failed_calls, 1);
}

#[test]
fn disable_preserves_records_and_skips_inflight_completion() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    call(&state, 1, MADB_OK);
    let started = state.begin();
    assert_eq!(state.configure(false), MADB_OK);
    state.complete_call(started, 3, 2, 0, MADB_OK);
    call(&state, 3, MADB_OK);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!((event.resource_id, present), (1, 1));
    assert_eq!(
        (
            stats.completed_calls,
            stats.dropped_events,
            stats.capture_enabled
        ),
        (3, 0, 0)
    );
    assert_eq!(state.drain().unwrap().1, 0);
    assert_eq!(state.configure(true), MADB_OK);
    assert_eq!(state.configure(true), MADB_OK);
    state.complete_callback(state.begin(), 5, 4);
    let (event, _, stats) = state.drain().unwrap();
    assert_eq!((event.record_id, event.symbol_id, event.phase), (2, 8, 2));
    assert_eq!((event.resource_id, event.related_resource_id), (5, 4));
    assert_eq!(
        (
            event.status_present,
            event.status,
            stats.completed_callbacks
        ),
        (0, 0, 1)
    );
}

#[test]
fn held_lock_drops_without_waiting_and_controls_report_busy() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    call(&state, 1, MADB_OK);
    let guard = state.ring.lock().unwrap();
    call(&state, 2, MADB_OK);
    assert_eq!(state.configure(false), MADB_BUSY);
    assert_eq!(state.drain(), Err(MADB_BUSY));
    assert_eq!(guard.len, 1);
    drop(guard);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!((event.resource_id, present), (1, 1));
    assert_eq!(
        (
            stats.completed_calls,
            stats.dropped_events,
            stats.capture_enabled
        ),
        (2, 1, 1)
    );
}

#[test]
fn locked_recheck_rejects_stale_enabled_hint_without_drop() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    let started = state.begin();
    // Model an old atomic hint observed concurrently with a completed disable.
    state.ring.lock().unwrap().enabled = false;
    state.complete_call(started, 3, 1, 0, MADB_OK);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!((event, present), (EMPTY, 0));
    assert_eq!(stats.dropped_events, 0);
}

#[test]
fn poisoned_diagnostics_never_panics_through_completion() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = state.ring.lock().unwrap();
        panic!("private diagnostic-state poison test");
    }));
    assert!(result.is_err());
    call(&state, 1, MADB_OK);
    state.complete_callback(state.begin(), 2, 1);
    assert_eq!(state.configure(false), MADB_INTERNAL_FAILURE);
    assert_eq!(state.drain(), Err(MADB_INTERNAL_FAILURE));
    assert_eq!(state.completed_calls.load(Ordering::Relaxed), 1);
    assert_eq!(state.completed_callbacks.load(Ordering::Relaxed), 1);
    assert_eq!(state.dropped_events.load(Ordering::Relaxed), 2);
}

#[test]
fn record_ids_exhaust_without_reuse_and_counters_wrap_separately() {
    let state = Diagnostics::new();
    assert_eq!(state.configure(true), MADB_OK);
    state.ring.lock().unwrap().next_id = Some(u64::MAX - 1);
    state.completed_calls.store(u64::MAX, Ordering::Relaxed);
    state.failed_calls.store(u64::MAX, Ordering::Relaxed);
    call(&state, 1, MADB_CLOSED);
    assert_eq!(state.completed_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.failed_calls.load(Ordering::Relaxed), 0);
    call(&state, 2, MADB_OK);
    call(&state, 3, MADB_OK);
    assert_eq!(state.drain().unwrap().0.record_id, u64::MAX - 1);
    assert_eq!(state.drain().unwrap().0.record_id, u64::MAX);
    assert_eq!(state.configure(false), MADB_OK);
    assert_eq!(state.configure(true), MADB_OK);
    call(&state, 4, MADB_OK);
    let (event, present, stats) = state.drain().unwrap();
    assert_eq!((event, present), (EMPTY, 0));
    assert_eq!(stats.dropped_events, 2);
}
