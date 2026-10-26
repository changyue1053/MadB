// Invoked by the single resource test sequence before terminal token exhaustion.
use super::*;

fn pull() -> (MadbDiagnosticEvent, u64, MadbDiagnosticStats) {
    let mut event = MadbDiagnosticEvent::default();
    let mut present = 99;
    let mut stats = MadbDiagnosticStats::default();
    let mut error = MadbError {
        domain: 99,
        code: 99,
    };
    // SAFETY: disjoint live stack objects with their declared alignment and size.
    assert_eq!(
        unsafe { madb_fixture_diagnostics_drain(&mut event, &mut present, &mut stats, &mut error) },
        MADB_OK
    );
    assert_eq!((error.domain, error.code), (0, 0));
    (event, present, stats)
}

fn configure(enabled: u32) {
    let mut error = MadbError::default();
    // SAFETY: live aligned output.
    assert_eq!(
        unsafe { madb_fixture_diagnostics_configure(enabled, &mut error) },
        MADB_OK
    );
}

fn identity_call() {
    let mut identity = [0; 64];
    // SAFETY: complete writable 64-byte output.
    assert_eq!(
        unsafe { madb_fixture_bootstrap(1, identity.as_mut_ptr(), 64) },
        MADB_OK
    );
}

pub(super) fn diagnostics_public_contract() {
    let (event, present, before) = pull();
    assert_eq!((event, present), (MadbDiagnosticEvent::default(), 0));
    assert_eq!(before.capture_enabled, 0);
    assert!(before.completed_calls > 0 && before.completed_callbacks > 0);
    configure(1);
    identity_call();
    let engine = create();
    let mut sum = 99;
    let mut error = MadbError::default();
    // SAFETY: input and outputs remain live and disjoint.
    assert_eq!(
        unsafe { madb_fixture_checksum(engine, [2_u8, 3].as_ptr(), 2, &mut sum, &mut error) },
        MADB_OK
    );
    assert_eq!(sum, 5);
    let mut context = Context::new(engine);
    let subscription = observe(engine, &mut context);
    close_release(subscription);
    context.alive.store(false, Ordering::SeqCst);
    close_release(engine);
    let mut events = Vec::new();
    for _ in 0..11 {
        let (event, present, _) = pull();
        assert_eq!(present, 1);
        events.push(event);
    }
    assert_eq!(
        events.iter().map(|e| e.symbol_id).collect::<Vec<_>>(),
        [1, 2, 3, 8, 7, 4, 5, 6, 4, 5, 6]
    );
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.record_id, index as u64 + 1);
        assert_eq!(event.phase, if event.symbol_id == 8 { 2 } else { 1 });
        assert_eq!(event.status_present, u64::from(event.symbol_id != 8));
        assert_eq!(event.status, 0);
    }
    assert_eq!(
        (events[0].resource_id, events[0].related_resource_id),
        (0, 0)
    );
    assert_eq!(
        (events[1].resource_id, events[1].related_resource_id),
        (engine, 0)
    );
    assert_eq!(
        (events[3].resource_id, events[3].related_resource_id),
        (subscription, engine)
    );
    assert_eq!(
        (events[4].resource_id, events[4].related_resource_id),
        (engine, subscription)
    );
    assert!(!context.bad.load(Ordering::SeqCst));
    let (event, present, after) = pull();
    assert_eq!((event, present), (MadbDiagnosticEvent::default(), 0));
    assert_eq!(after.completed_calls - before.completed_calls, 10);
    assert_eq!(after.completed_callbacks - before.completed_callbacks, 1);
    assert_eq!(after.failed_calls, before.failed_calls);
    assert_eq!(after.dropped_events, 0);
    rejected_outputs_preserve_queue();
    configure(0);
    identity_call();
    let (event, present, stats) = pull();
    assert_eq!((event, present), (MadbDiagnosticEvent::default(), 0));
    assert_eq!(stats.capture_enabled, 0);
}

fn rejected_outputs_preserve_queue() {
    identity_call();
    for slot in 0..4 {
        for misaligned in [false, true] {
            let mut event = MadbDiagnosticEvent {
                record_id: 99,
                ..Default::default()
            };
            let mut present = 99;
            let mut stats = MadbDiagnosticStats {
                completed_calls: 99,
                ..Default::default()
            };
            let mut error = MadbError {
                domain: 99,
                code: 99,
            };
            // Large aligned real storage: its +1 address is valid allocation but
            // deliberately misaligned. The ABI must reject it before any write.
            let mut storage = [u64::MAX; 10];
            let invalid = if misaligned {
                storage.as_mut_ptr().cast::<u8>().wrapping_add(1)
            } else {
                core::ptr::null_mut()
            };
            let event_out = if slot == 0 {
                invalid.cast()
            } else {
                &mut event
            };
            let present_out = if slot == 1 {
                invalid.cast()
            } else {
                &mut present
            };
            let stats_out = if slot == 2 {
                invalid.cast()
            } else {
                &mut stats
            };
            let error_out = if slot == 3 {
                invalid.cast()
            } else {
                &mut error
            };
            // SAFETY: every non-rejected pointer is a valid disjoint output;
            // invalid is either null or allocated and mechanically misaligned.
            assert_eq!(
                unsafe {
                    madb_fixture_diagnostics_drain(event_out, present_out, stats_out, error_out)
                },
                MADB_INVALID_ARGUMENT
            );
            if slot != 0 {
                assert_eq!(event, MadbDiagnosticEvent::default());
            }
            if slot != 1 {
                assert_eq!(present, 0);
            }
            if slot != 2 {
                assert_eq!(stats, MadbDiagnosticStats::default());
            }
            if slot != 3 {
                assert_eq!((error.domain, error.code), (0, 0));
            }
            assert_eq!(storage, [u64::MAX; 10]);
        }
    }
    let mut error = MadbError {
        domain: 99,
        code: 99,
    };
    // SAFETY: valid error object even though enabled value is rejected.
    assert_eq!(
        unsafe { madb_fixture_diagnostics_configure(2, &mut error) },
        MADB_INVALID_ARGUMENT
    );
    assert_eq!((error.domain, error.code), (0, 0));
    let (event, present, stats) = pull();
    assert_eq!((event.symbol_id, present, stats.capture_enabled), (1, 1, 1));
    assert_eq!(pull().1, 0);
}

#[cfg(feature = "test-support")]
pub(super) fn diagnostics_full_keeps_resource_protection() {
    configure(1);
    for _ in 0..70 {
        identity_call();
    }
    operation_close_barrier();
    source_close_barrier();
    reentrant_subscription_close();
    registration_panic_and_diagnostic_failures();
    configure(0);
    let mut last_id = 0;
    for _ in 0..64 {
        let (event, present, stats) = pull();
        assert_eq!(present, 1);
        assert!(event.record_id > last_id);
        last_id = event.record_id;
        assert!(stats.dropped_events >= 6);
        assert_eq!(stats.capture_enabled, 0);
    }
    assert_eq!(pull().1, 0);
    let counts = counts();
    assert_eq!(counts.registry_entries, 0);
    assert_eq!(counts.source_leases, 0);
    assert_eq!(counts.engines_created, counts.engines_released);
    assert_eq!(counts.subscriptions_created, counts.subscriptions_released);
}
