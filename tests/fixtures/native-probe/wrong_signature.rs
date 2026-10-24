#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
include!("c_provider.rs");
// Must fail with E0308: the published length is u64, not u32.
const _: unsafe extern "C" fn(*const u8, u32, *mut u64, *mut MadbError) -> MadbStatus =
    madb_c_fixture_checksum;
