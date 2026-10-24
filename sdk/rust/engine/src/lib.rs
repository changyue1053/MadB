//! First-route native owner. The generated header is derived from these exports.
//! No tool-host dependency, worker pool, consumer queue, or timer is used.
#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod runtime;
#[cfg(feature = "test-support")]
mod test_support;
#[cfg(test)]
mod tests;

mod identity {
    include!(concat!(env!("OUT_DIR"), "/identity.rs"));
}

pub type MadbStatus = i32;
pub type MadbHandle = u64;
pub const MADB_OK: MadbStatus = 0;
pub const MADB_INVALID_ARGUMENT: MadbStatus = 1;
pub const MADB_INVALID_HANDLE: MadbStatus = 2;
pub const MADB_CLOSING: MadbStatus = 3;
pub const MADB_CLOSED: MadbStatus = 4;
pub const MADB_BUSY: MadbStatus = 5;
pub const MADB_TIMEOUT: MadbStatus = 6;
pub const MADB_WOULD_DEADLOCK: MadbStatus = 7;
pub const MADB_INTERNAL_FAILURE: MadbStatus = 8;
pub const MADB_RESOURCE_EXHAUSTED: MadbStatus = 9;
pub const MADB_DOMAIN_ERROR: MadbStatus = 10;
pub const MADB_UNSUPPORTED: MadbStatus = 11;
/// Maximum registered engine and subscription tokens per loaded SDK instance.
pub const MADB_MAX_HANDLES: u32 = 4096;
/// Closed subscriptions retain a slot until their caller releases the token.
pub const MADB_MAX_SUBSCRIPTIONS_PER_ENGINE: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MadbError {
    pub domain: u32,
    pub code: i32,
}

/// The caller retains context until successful close wait, including source leases
/// taken before callback entry. Foreign exceptions and panics must not unwind here.
pub type MadbStateCallback = Option<unsafe extern "C" fn(*mut c_void, u64, u32)>;

fn boundary(operation: impl FnOnce() -> MadbStatus) -> MadbStatus {
    let status = catch_unwind(AssertUnwindSafe(operation)).unwrap_or(MADB_INTERNAL_FAILURE);
    #[cfg(feature = "test-support")]
    test_support::diagnostic();
    status
}

/// Validity, writable allocation size, and disjointness remain caller obligations.
/// This checks only the mechanically checkable null/alignment prerequisites.
unsafe fn clear<T: Default>(output: *mut T) -> bool {
    if output.is_null() || !output.is_aligned() {
        false
    } else {
        // SAFETY: allocation validity and disjointness are the ABI caller contract.
        unsafe { output.write(T::default()) };
        true
    }
}

/// # Safety
/// At capacity 64, out_identity must denote 64 writable bytes. No write occurs for
/// another capacity. Version mismatch clears a valid output before Unsupported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_bootstrap(
    requested_version: u32,
    out_identity: *mut u8,
    capacity: u64,
) -> MadbStatus {
    boundary(|| {
        if capacity != 64 || out_identity.is_null() {
            return MADB_INVALID_ARGUMENT;
        }
        // SAFETY: caller provides the complete writable bootstrap output.
        unsafe { out_identity.write_bytes(0, 64) };
        if requested_version != 1 {
            return MADB_UNSUPPORTED;
        }
        // SAFETY: the two non-overlapping slices fit the validated output size.
        unsafe {
            core::ptr::copy_nonoverlapping(identity::INTERFACE.as_ptr(), out_identity, 32);
            core::ptr::copy_nonoverlapping(identity::PROFILE.as_ptr(), out_identity.add(32), 32);
        }
        MADB_OK
    })
}

/// # Safety
/// Outputs are aligned, writable, disjoint objects. Valid outputs are zeroed even
/// when another output is null. Arbitrary invalid addresses cannot be checked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_create(
    out_engine: *mut MadbHandle,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller obligations above, checked independently before use.
        let (handle_valid, error_valid) = unsafe { (clear(out_engine), clear(out_error)) };
        if !handle_valid || !error_valid {
            return MADB_INVALID_ARGUMENT;
        }
        match runtime::create() {
            Ok(handle) => {
                // SAFETY: output checked above and remains caller-owned.
                unsafe { out_engine.write(handle) };
                MADB_OK
            }
            Err(status) => status,
        }
    })
}

/// # Safety
/// Nonzero accepted length requires that many readable bytes through return,
/// disjoint from aligned writable outputs. Inputs above 65536 are never read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_checksum(
    engine: MadbHandle,
    data: *const u8,
    length_bytes: u64,
    out_sum: *mut u64,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller owns valid outputs throughout the call.
        let (sum_valid, error_valid) = unsafe { (clear(out_sum), clear(out_error)) };
        if !sum_valid
            || !error_valid
            || length_bytes > 65_536
            || (length_bytes != 0 && data.is_null())
        {
            return MADB_INVALID_ARGUMENT;
        }
        let Ok(length) = usize::try_from(length_bytes) else {
            return MADB_INVALID_ARGUMENT;
        };
        let _permit = match runtime::operation(engine) {
            Ok(permit) => permit,
            Err(status) => return status,
        };
        #[cfg(feature = "test-support")]
        {
            test_support::pause(1);
            assert!(
                !test_support::take_fault(2),
                "injected checksum panic after admission"
            );
        }
        let sum = if length == 0 {
            0
        } else {
            // SAFETY: bounded, non-null input with caller-guaranteed liveness.
            unsafe { core::slice::from_raw_parts(data, length) }
                .iter()
                .map(|b| u64::from(*b))
                .sum()
        };
        // SAFETY: output checked above and remains valid until return.
        unsafe { out_sum.write(sum) };
        MADB_OK
    })
}

/// # Safety
/// out_error is an aligned writable object, and remains valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_request_close(
    handle: MadbHandle,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned output; null/alignment are checked by clear.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        runtime::request_close(handle)
    })
}

/// # Safety
/// out_error is an aligned writable object, and remains valid through the wait.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_await_closed(
    handle: MadbHandle,
    timeout_ms: u32,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned output; clear checks null/alignment.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        runtime::await_closed(handle, timeout_ms)
    })
}

/// # Safety
/// out_error is an aligned writable object, and remains valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_release_handle(
    handle: MadbHandle,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: caller-owned output; clear checks null/alignment.
        if !unsafe { clear(out_error) } {
            return MADB_INVALID_ARGUMENT;
        }
        runtime::release_handle(handle)
    })
}

/// # Safety
/// Outputs must be aligned, writable and disjoint. Callback/context must remain
/// callable/alive through successful subscription or parent-engine close wait.
/// Initial callback (sequence 1, state 0) is synchronous and may request close.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_fixture_observe(
    engine: MadbHandle,
    callback: MadbStateCallback,
    context: *mut c_void,
    out_subscription: *mut MadbHandle,
    out_error: *mut MadbError,
) -> MadbStatus {
    boundary(|| {
        // SAFETY: both caller-owned outputs are checked independently.
        let (subscription_valid, error_valid) =
            unsafe { (clear(out_subscription), clear(out_error)) };
        if !subscription_valid || !error_valid || callback.is_none() {
            return MADB_INVALID_ARGUMENT;
        }
        // SAFETY: callback/context validity through drain is required above.
        match unsafe { runtime::observe(engine, callback.expect("checked callback"), context) } {
            Ok(handle) => {
                // SAFETY: checked, caller-owned output.
                unsafe { out_subscription.write(handle) };
                MADB_OK
            }
            Err(status) => status,
        }
    })
}
