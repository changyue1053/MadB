use super::*;
#[cfg(feature = "test-support")]
use crate::test_support::*;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

struct Context {
    alive: AtomicBool,
    calls: AtomicU64,
    sequence: AtomicU64,
    state: AtomicU64,
    bad: AtomicBool,
    engine: u64,
    #[cfg(feature = "test-support")]
    subscription: AtomicU64,
    close_in_callback: bool,
    query_status: AtomicI32,
    wait_status: AtomicI32,
    #[cfg(feature = "test-support")]
    recursive_status: AtomicI32,
}
impl Context {
    fn new(engine: u64) -> Self {
        Self {
            alive: AtomicBool::new(true),
            calls: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            state: AtomicU64::new(0),
            bad: AtomicBool::new(false),
            engine,
            #[cfg(feature = "test-support")]
            subscription: AtomicU64::new(0),
            close_in_callback: false,
            query_status: AtomicI32::new(-1),
            wait_status: AtomicI32::new(-1),
            #[cfg(feature = "test-support")]
            recursive_status: AtomicI32::new(-1),
        }
    }
}
unsafe extern "C" fn callback(context: *mut c_void, sequence: u64, state: u32) {
    // SAFETY: the test keeps this allocation alive until its successful close.
    let context = unsafe { &*context.cast::<Context>() };
    if !context.alive.load(Ordering::SeqCst) {
        context.bad.store(true, Ordering::SeqCst);
    }
    let previous = context.sequence.swap(sequence, Ordering::SeqCst);
    if sequence != previous + 1 {
        context.bad.store(true, Ordering::SeqCst);
    }
    context.calls.fetch_add(1, Ordering::SeqCst);
    context.state.store(u64::from(state), Ordering::SeqCst);
    if context.close_in_callback {
        let mut error = MadbError::default();
        let mut sum = 99;
        // SAFETY: local outputs and empty input satisfy the published contract.
        context.query_status.store(
            unsafe {
                madb_fixture_checksum(context.engine, core::ptr::null(), 0, &mut sum, &mut error)
            },
            Ordering::SeqCst,
        );
        // SAFETY: valid engine and local output; request close is nonblocking.
        unsafe {
            madb_fixture_request_close(context.engine, &mut error);
        }
        // SAFETY: runtime must reject self-wait before blocking.
        context.wait_status.store(
            unsafe { madb_fixture_await_closed(context.engine, 10_000, &mut error) },
            Ordering::SeqCst,
        );
    }
    #[cfg(feature = "test-support")]
    if context.subscription.load(Ordering::SeqCst) != 0 {
        let subscription = context.subscription.load(Ordering::SeqCst);
        let mut error = MadbError::default();
        // SAFETY: reentrant attempt must be rejected, without calling recursively.
        context.recursive_status.store(
            unsafe { madb_fixture_test_emit(subscription, state, &mut error) },
            Ordering::SeqCst,
        );
        // SAFETY: nonblocking close and rejected self-wait use valid handles.
        unsafe {
            madb_fixture_request_close(subscription, &mut error);
        }
        context.wait_status.store(
            unsafe { madb_fixture_await_closed(subscription, 10_000, &mut error) },
            Ordering::SeqCst,
        );
    }
}
fn create() -> u64 {
    let mut handle = 0;
    let mut error = MadbError::default();
    // SAFETY: local disjoint outputs.
    assert_eq!(
        unsafe { madb_fixture_create(&mut handle, &mut error) },
        MADB_OK
    );
    assert_ne!(handle, 0);
    handle
}
fn close_release(handle: u64) {
    let mut error = MadbError::default();
    // SAFETY: local output, valid handle from the test.
    unsafe {
        assert_eq!(madb_fixture_request_close(handle, &mut error), MADB_OK);
        assert_eq!(madb_fixture_await_closed(handle, 1000, &mut error), MADB_OK);
        assert_eq!(madb_fixture_release_handle(handle, &mut error), MADB_OK);
    }
}
fn observe(engine: u64, context: &mut Context) -> u64 {
    let mut handle = 0;
    let mut error = MadbError::default();
    // SAFETY: context lives until this test closes and waits on the registration.
    assert_eq!(
        unsafe {
            madb_fixture_observe(
                engine,
                Some(callback),
                (context as *mut Context).cast(),
                &mut handle,
                &mut error,
            )
        },
        MADB_OK
    );
    assert_eq!(context.calls.load(Ordering::SeqCst), 1);
    assert_eq!(context.state.load(Ordering::SeqCst), 0);
    handle
}

// One process-wide sequence is intentional: token exhaustion is terminal and
// runs last. Lab uses independent C/Rust consumers and its own expected values.
#[test]
fn first_route_protocol() {
    bootstrap_and_values();
    synchronous_registration_close();
    #[cfg(feature = "diagnostics")]
    diagnostics_public_contract();
    capacity_limits();
    #[cfg(feature = "test-support")]
    {
        operation_close_barrier();
        source_close_barrier();
        reentrant_subscription_close();
        registration_panic_and_diagnostic_failures();
        #[cfg(feature = "diagnostics")]
        diagnostics_full_keeps_resource_protection();
        token_exhaustion();
    }
}
fn bootstrap_and_values() {
    let mut bytes = [9_u8; 64];
    let mut error = MadbError {
        domain: 99,
        code: 99,
    };
    // SAFETY: local arrays and outputs satisfy the declared capacities/layouts.
    unsafe {
        assert_eq!(madb_fixture_bootstrap(1, bytes.as_mut_ptr(), 64), MADB_OK);
        assert_eq!(&bytes[..32], &identity::INTERFACE);
        assert_eq!(&bytes[32..], &identity::PROFILE);
        assert_eq!(
            madb_fixture_bootstrap(2, bytes.as_mut_ptr(), 64),
            MADB_UNSUPPORTED
        );
        assert_eq!(bytes, [0; 64]);
        bytes.fill(9);
        assert_eq!(
            madb_fixture_bootstrap(1, bytes.as_mut_ptr(), 63),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(bytes, [9; 64]);
        assert_eq!(
            madb_fixture_create(core::ptr::null_mut(), &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!((error.domain, error.code), (0, 0));
    }
    let engine = create();
    let mut context = Context::new(engine);
    let subscription = observe(engine, &mut context);
    let mut sum = 99;
    let data = [255_u8, 0, 1, 128];
    // SAFETY: data and local outputs live across each synchronous call.
    unsafe {
        assert_eq!(
            madb_fixture_checksum(engine, data.as_ptr(), 4, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 384);
        assert_eq!(
            madb_fixture_checksum(engine, core::ptr::null(), 0, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 0);
        sum = 99;
        assert_eq!(
            madb_fixture_checksum(engine, core::ptr::null(), 1, &mut sum, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(sum, 0);
        assert_eq!(
            madb_fixture_checksum(engine, data.as_ptr(), u64::MAX, &mut sum, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(
            madb_fixture_checksum(subscription, data.as_ptr(), 4, &mut sum, &mut error),
            MADB_INVALID_HANDLE
        );
        assert_eq!(madb_fixture_await_closed(engine, 0, &mut error), MADB_BUSY);
        assert_eq!(madb_fixture_release_handle(engine, &mut error), MADB_BUSY);
    }
    close_release(engine);
    context.alive.store(false, Ordering::SeqCst);
    // The child token belongs to its caller even after the engine is released.
    unsafe {
        assert_eq!(
            madb_fixture_await_closed(subscription, 0, &mut error),
            MADB_OK
        );
        assert_eq!(
            madb_fixture_release_handle(subscription, &mut error),
            MADB_OK
        );
        assert_eq!(
            madb_fixture_release_handle(subscription, &mut error),
            MADB_INVALID_HANDLE
        );
        assert_eq!(
            madb_fixture_checksum(engine, data.as_ptr(), 4, &mut sum, &mut error),
            MADB_INVALID_HANDLE
        );
    }
    assert!(!context.bad.load(Ordering::SeqCst));
    let later = create();
    assert!(later > subscription);
    close_release(later);
}
fn synchronous_registration_close() {
    let engine = create();
    let mut context = Context::new(engine);
    context.close_in_callback = true;
    let mut subscription = 99;
    let mut error = MadbError::default();
    // SAFETY: context and outputs stay live through failure rollback and drain.
    unsafe {
        assert_eq!(
            madb_fixture_observe(
                engine,
                Some(callback),
                (&mut context as *mut Context).cast(),
                &mut subscription,
                &mut error
            ),
            MADB_CLOSING
        );
        assert_eq!(subscription, 0);
        assert_eq!(context.query_status.load(Ordering::SeqCst), MADB_OK);
        assert_eq!(
            context.wait_status.load(Ordering::SeqCst),
            MADB_WOULD_DEADLOCK
        );
        assert_eq!(madb_fixture_await_closed(engine, 0, &mut error), MADB_OK);
        assert_eq!(madb_fixture_release_handle(engine, &mut error), MADB_OK);
    }
}

#[cfg(feature = "test-support")]
fn counts() -> MadbTestCounts {
    let mut counts = MadbTestCounts::default();
    let mut error = MadbError::default();
    // SAFETY: disjoint local outputs.
    assert_eq!(
        unsafe { madb_fixture_test_snapshot(&mut counts, &mut error) },
        MADB_OK
    );
    counts
}
#[cfg(feature = "test-support")]
fn operation_close_barrier() {
    let before = counts();
    let engine = create();
    let mut error = MadbError::default();
    // SAFETY: local output, documented gate.
    assert_eq!(unsafe { madb_fixture_test_arm(1, &mut error) }, MADB_OK);
    let worker = std::thread::spawn(move || {
        let mut error = MadbError::default();
        let mut sum = 99;
        // SAFETY: worker-local input/output remains alive through resume.
        let status = unsafe {
            madb_fixture_checksum(engine, [1_u8, 2, 3].as_ptr(), 3, &mut sum, &mut error)
        };
        (status, sum)
    });
    unsafe {
        assert_eq!(madb_fixture_test_wait(1, 1000, &mut error), MADB_OK);
        assert_eq!(counts().active_operations, 1);
        let close_threads = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    let mut error = MadbError::default();
                    madb_fixture_request_close(engine, &mut error)
                })
            })
            .collect::<Vec<_>>();
        for closer in close_threads {
            assert_eq!(closer.join().unwrap(), MADB_OK);
        }
        assert_eq!(madb_fixture_request_close(engine, &mut error), MADB_OK);
        assert_eq!(madb_fixture_request_close(engine, &mut error), MADB_OK);
        assert_eq!(
            madb_fixture_await_closed(engine, 0, &mut error),
            MADB_TIMEOUT
        );
        assert_eq!(madb_fixture_release_handle(engine, &mut error), MADB_BUSY);
        let mut sum = 99;
        assert_eq!(
            madb_fixture_checksum(engine, core::ptr::null(), 0, &mut sum, &mut error),
            MADB_CLOSING
        );
        assert_eq!(sum, 0);
        assert_eq!(counts().engines_released, before.engines_released);
        assert_eq!(madb_fixture_test_resume(1, &mut error), MADB_OK);
        assert_eq!(worker.join().unwrap(), (MADB_OK, 6));
        assert_eq!(madb_fixture_await_closed(engine, 1000, &mut error), MADB_OK);
        assert_eq!(counts().engines_released, before.engines_released + 1);
        assert_eq!(
            madb_fixture_checksum(engine, core::ptr::null(), 0, &mut sum, &mut error),
            MADB_CLOSED
        );
        assert_eq!(madb_fixture_release_handle(engine, &mut error), MADB_OK);
    }
}
#[cfg(feature = "test-support")]
fn source_close_barrier() {
    let before = counts();
    let engine = create();
    let mut context = Context::new(engine);
    let subscription = observe(engine, &mut context);
    let mut error = MadbError::default();
    unsafe {
        assert_eq!(madb_fixture_test_arm(2, &mut error), MADB_OK);
    }
    let worker = std::thread::spawn(move || {
        let mut error = MadbError::default();
        // SAFETY: registration context remains live on the parent test stack.
        unsafe { madb_fixture_test_emit(subscription, 77, &mut error) }
    });
    unsafe {
        assert_eq!(madb_fixture_test_wait(2, 1000, &mut error), MADB_OK);
        assert_eq!(counts().source_leases, 1);
        assert_eq!(context.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            madb_fixture_test_emit(subscription, 88, &mut error),
            MADB_BUSY
        );
        assert_eq!(
            madb_fixture_request_close(subscription, &mut error),
            MADB_OK
        );
        assert_eq!(madb_fixture_request_close(engine, &mut error), MADB_OK);
        assert_eq!(
            madb_fixture_await_closed(subscription, 0, &mut error),
            MADB_TIMEOUT
        );
        assert_eq!(
            madb_fixture_await_closed(engine, 0, &mut error),
            MADB_TIMEOUT
        );
        assert_eq!(
            madb_fixture_release_handle(subscription, &mut error),
            MADB_BUSY
        );
        assert_eq!(
            counts().subscriptions_released,
            before.subscriptions_released
        );
        assert_eq!(madb_fixture_test_resume(2, &mut error), MADB_OK);
        assert_eq!(worker.join().unwrap(), MADB_OK);
        assert_eq!(
            madb_fixture_await_closed(subscription, 1000, &mut error),
            MADB_OK
        );
        assert_eq!(madb_fixture_await_closed(engine, 1000, &mut error), MADB_OK);
        context.alive.store(false, Ordering::SeqCst);
        assert_eq!(
            madb_fixture_test_emit(subscription, 99, &mut error),
            MADB_CLOSED
        );
        assert_eq!(context.calls.load(Ordering::SeqCst), 2);
        assert_eq!(context.state.load(Ordering::SeqCst), 77);
        assert!(!context.bad.load(Ordering::SeqCst));
        assert_eq!(
            counts().subscriptions_released,
            before.subscriptions_released + 1
        );
        assert_eq!(madb_fixture_release_handle(engine, &mut error), MADB_OK);
        assert_eq!(
            madb_fixture_release_handle(subscription, &mut error),
            MADB_OK
        );
    }
}
#[cfg(feature = "test-support")]
fn reentrant_subscription_close() {
    let engine = create();
    let mut context = Context::new(engine);
    let subscription = observe(engine, &mut context);
    context.subscription.store(subscription, Ordering::SeqCst);
    let mut error = MadbError::default();
    unsafe {
        assert_eq!(madb_fixture_test_emit(subscription, 3, &mut error), MADB_OK);
        assert_eq!(
            context.recursive_status.load(Ordering::SeqCst),
            MADB_WOULD_DEADLOCK
        );
        assert_eq!(
            context.wait_status.load(Ordering::SeqCst),
            MADB_WOULD_DEADLOCK
        );
        assert_eq!(
            madb_fixture_await_closed(subscription, 0, &mut error),
            MADB_OK
        );
        assert_eq!(
            madb_fixture_release_handle(subscription, &mut error),
            MADB_OK
        );
    }
    close_release(engine);
}
#[cfg(feature = "test-support")]
fn registration_panic_and_diagnostic_failures() {
    let engine = create();
    let mut context = Context::new(engine);
    let mut subscription = 99;
    let mut error = MadbError::default();
    unsafe {
        assert_eq!(madb_fixture_test_set_fault(1, 1, &mut error), MADB_OK);
        assert_eq!(
            madb_fixture_observe(
                engine,
                Some(callback),
                (&mut context as *mut Context).cast(),
                &mut subscription,
                &mut error
            ),
            MADB_INTERNAL_FAILURE
        );
        assert_eq!(subscription, 0);
        assert_eq!(context.calls.load(Ordering::SeqCst), 0);
        assert_eq!(counts().registry_entries, 1);
        assert_eq!(madb_fixture_test_set_fault(2, 1, &mut error), MADB_OK);
        let mut sum = 99;
        assert_eq!(
            madb_fixture_checksum(engine, core::ptr::null(), 0, &mut sum, &mut error),
            MADB_INTERNAL_FAILURE
        );
        assert_eq!(sum, 0);
        assert_eq!(counts().active_operations, 0);
        assert_eq!(madb_fixture_test_set_fault(3, 1, &mut error), MADB_OK);
        assert_eq!(
            madb_fixture_checksum(engine, [5_u8].as_ptr(), 1, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 5);
        close_release(engine);
        assert_eq!(madb_fixture_test_set_fault(3, 0, &mut error), MADB_OK);
        let final_counts = counts();
        assert!(final_counts.diagnostic_failures >= 4);
        assert_eq!(final_counts.engines_created, final_counts.engines_released);
        assert_eq!(
            final_counts.subscriptions_created,
            final_counts.subscriptions_released
        );
        assert_eq!(final_counts.registry_entries, 0);
        assert_eq!(final_counts.source_leases, 0);
    }
}
#[cfg(feature = "test-support")]
fn token_exhaustion() {
    let mut error = MadbError::default();
    unsafe {
        assert_eq!(
            madb_fixture_test_set_next_token(1, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(
            madb_fixture_test_set_next_token(u64::MAX, &mut error),
            MADB_OK
        );
        let engine = create();
        assert_eq!(engine, u64::MAX);
        close_release(engine);
        let mut failed = 99;
        assert_eq!(
            madb_fixture_create(&mut failed, &mut error),
            MADB_RESOURCE_EXHAUSTED
        );
        assert_eq!(failed, 0);
        assert_eq!(
            madb_fixture_test_set_next_token(1, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(counts().registry_entries, 0);
    }
}

unsafe extern "C" fn empty_callback(_: *mut c_void, _: u64, _: u32) {}
fn capacity_limits() {
    let mut error = MadbError::default();
    let engines = (0..MADB_MAX_HANDLES).map(|_| create()).collect::<Vec<_>>();
    let mut excess = 99;
    // SAFETY: local outputs, explicitly bounded admission test.
    unsafe {
        assert_eq!(
            madb_fixture_create(&mut excess, &mut error),
            MADB_RESOURCE_EXHAUSTED
        );
        assert_eq!(excess, 0);
    }
    for engine in engines {
        close_release(engine);
    }
    let engine = create();
    let mut subscriptions = Vec::new();
    for _ in 0..MADB_MAX_SUBSCRIPTIONS_PER_ENGINE {
        let mut subscription = 0;
        // SAFETY: null context is permitted and the callback never reads it.
        unsafe {
            assert_eq!(
                madb_fixture_observe(
                    engine,
                    Some(empty_callback),
                    core::ptr::null_mut(),
                    &mut subscription,
                    &mut error
                ),
                MADB_OK
            );
        }
        subscriptions.push(subscription);
    }
    excess = 99;
    unsafe {
        assert_eq!(
            madb_fixture_observe(
                engine,
                Some(empty_callback),
                core::ptr::null_mut(),
                &mut excess,
                &mut error
            ),
            MADB_RESOURCE_EXHAUSTED
        );
        assert_eq!(excess, 0);
        // Closing one registration retains the caller's registered token slot.
        assert_eq!(
            madb_fixture_request_close(subscriptions[0], &mut error),
            MADB_OK
        );
        assert_eq!(
            madb_fixture_observe(
                engine,
                Some(empty_callback),
                core::ptr::null_mut(),
                &mut excess,
                &mut error
            ),
            MADB_RESOURCE_EXHAUSTED
        );
        assert_eq!(
            madb_fixture_release_handle(subscriptions.remove(0), &mut error),
            MADB_OK
        );
        assert_eq!(
            madb_fixture_observe(
                engine,
                Some(empty_callback),
                core::ptr::null_mut(),
                &mut excess,
                &mut error
            ),
            MADB_OK
        );
        subscriptions.push(excess);
    }
    // Control and cleanup still operate when subscription capacity is full.
    close_release(engine);
    for subscription in subscriptions {
        unsafe {
            assert_eq!(
                madb_fixture_release_handle(subscription, &mut error),
                MADB_OK
            );
        }
    }
}

#[cfg(feature = "diagnostics")]
#[path = "diagnostics_abi_tests.rs"]
mod diagnostics_abi_tests;
#[cfg(all(feature = "diagnostics", feature = "test-support"))]
use diagnostics_abi_tests::diagnostics_full_keeps_resource_protection;
#[cfg(feature = "diagnostics")]
use diagnostics_abi_tests::diagnostics_public_contract;
