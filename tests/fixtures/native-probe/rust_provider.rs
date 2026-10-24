//! P02 stateless probe, not the C05 engine implementation.
#![deny(unsafe_op_in_unsafe_fn)]

pub type MadbStatus = i32;
pub const MADB_OK: MadbStatus = 0;
pub const MADB_INVALID_ARGUMENT: MadbStatus = 1;

#[repr(C)]
pub struct MadbError {
    pub domain: u32,
    pub code: i32,
}

/// Layout witness only. No pointer ownership is transferred.
#[repr(C)]
pub struct MadbProbeLayout {
    pub tag: u8,
    pub count: u64,
    pub data: *const u8,
    pub length_bytes: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn madb_probe_rust_echo(value: u64) -> u64 {
    value
}

/// # Safety
/// Non-null outputs must point to aligned, writable, disjoint objects. A non-null
/// input at an accepted length (1..=65_536) must denote that many readable bytes,
/// disjoint from outputs, and stay alive without concurrent mutation until return.
/// At length zero or above the limit the input is not dereferenced.
/// Null and excessive lengths are rejected; arbitrary dangling pointers cannot
/// be validated by this boundary. This probe has no handle or async lifecycle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn madb_probe_rust_checksum(
    data: *const u8,
    length_bytes: u64,
    out_sum: *mut u64,
    out_error: *mut MadbError,
) -> MadbStatus {
    let sum_valid = !out_sum.is_null() && out_sum.is_aligned();
    let error_valid = !out_error.is_null() && out_error.is_aligned();
    // SAFETY: after null/alignment checks, validity and disjointness are caller obligations.
    unsafe {
        if sum_valid {
            out_sum.write(0);
        }
        if error_valid {
            out_error.write(MadbError { domain: 0, code: 0 });
        }
    }
    if !sum_valid || !error_valid || length_bytes > 65_536 {
        return MADB_INVALID_ARGUMENT;
    }
    if length_bytes == 0 {
        return MADB_OK; // Rust slices require a non-null pointer even at length zero.
    }
    if data.is_null() {
        return MADB_INVALID_ARGUMENT;
    }
    let Ok(length) = usize::try_from(length_bytes) else {
        return MADB_INVALID_ARGUMENT;
    };
    // SAFETY: input validity/liveness are caller obligations. The bound prevents
    // arithmetic overflow and this path allocates nothing and cannot panic.
    let bytes = unsafe { core::slice::from_raw_parts(data, length) };
    let sum = bytes.iter().map(|byte| u64::from(*byte)).sum();
    // SAFETY: checked output and same caller obligations as above.
    unsafe { out_sum.write(sum) };
    MADB_OK
}
