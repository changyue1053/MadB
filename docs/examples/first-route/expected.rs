//! C02 declaration shape only; not bindgen output and not a runtime implementation.
#![allow(dead_code)]
use core::ffi::c_void;
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
#[repr(C)]
pub struct MadbError { pub domain: u32, pub code: i32 }
pub type MadbStateCallback = Option<unsafe extern "C" fn(*mut c_void, u64, u32)>;
unsafe extern "C" {
    pub fn madb_fixture_create(out_engine: *mut MadbHandle, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_fixture_checksum(engine: MadbHandle, data: *const u8, length_bytes: u64, out_sum: *mut u64, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_fixture_request_close(handle: MadbHandle, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_fixture_await_closed(handle: MadbHandle, timeout_ms: u32, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_fixture_release_handle(handle: MadbHandle, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_fixture_observe(engine: MadbHandle, callback: MadbStateCallback, context: *mut c_void, out_subscription: *mut MadbHandle, out_error: *mut MadbError) -> MadbStatus;
    pub fn madb_c_fixture_checksum(data: *const u8, length_bytes: u64, out_sum: *mut u64, out_error: *mut MadbError) -> MadbStatus;
}
const _: () = assert!(core::mem::size_of::<MadbError>() == 8);
const _: () = assert!(core::mem::align_of::<MadbError>() == 4);
const _: () = assert!(core::mem::offset_of!(MadbError, code) == 4);
const _: () = assert!(core::mem::size_of::<MadbHandle>() == 8);
