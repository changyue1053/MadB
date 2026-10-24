include!("expected.rs");
const _: () = {
    let statuses: [MadbStatus; 12] = [
        MADB_OK,
        MADB_INVALID_ARGUMENT,
        MADB_INVALID_HANDLE,
        MADB_CLOSING,
        MADB_CLOSED,
        MADB_BUSY,
        MADB_TIMEOUT,
        MADB_WOULD_DEADLOCK,
        MADB_INTERNAL_FAILURE,
        MADB_RESOURCE_EXHAUSTED,
        MADB_DOMAIN_ERROR,
        MADB_UNSUPPORTED,
    ];
    let mut index = 0;
    while index < statuses.len() {
        assert!(statuses[index] == index as i32);
        index += 1;
    }
};
const _: unsafe extern "C" fn(*mut MadbHandle, *mut MadbError) -> MadbStatus = madb_fixture_create;
const _: unsafe extern "C" fn(MadbHandle, *const u8, u64, *mut u64, *mut MadbError) -> MadbStatus =
    madb_fixture_checksum;
const _: unsafe extern "C" fn(
    MadbHandle,
    MadbStateCallback,
    *mut core::ffi::c_void,
    *mut MadbHandle,
    *mut MadbError,
) -> MadbStatus = madb_fixture_observe;
const _: unsafe extern "C" fn(MadbHandle, *mut MadbError) -> MadbStatus =
    madb_fixture_request_close;
const _: unsafe extern "C" fn(MadbHandle, u32, *mut MadbError) -> MadbStatus =
    madb_fixture_await_closed;
const _: unsafe extern "C" fn(MadbHandle, *mut MadbError) -> MadbStatus =
    madb_fixture_release_handle;
const _: unsafe extern "C" fn(*const u8, u64, *mut u64, *mut MadbError) -> MadbStatus =
    madb_c_fixture_checksum;
