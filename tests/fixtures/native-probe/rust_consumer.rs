//! Independent Rust program: bindgen output + C library only.
#![deny(unsafe_op_in_unsafe_fn)]
#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
mod ffi {
    include!("c_provider.rs");
}
use ffi::*;
const _: () = {
    assert!(core::mem::size_of::<MadbError>() == 8);
    assert!(core::mem::align_of::<MadbError>() == 4);
    assert!(core::mem::offset_of!(MadbError, code) == 4);
    assert!(core::mem::size_of::<MadbProbeLayout>() == 32);
    assert!(core::mem::align_of::<MadbProbeLayout>() == 8);
    assert!(core::mem::offset_of!(MadbProbeLayout, count) == 8);
    assert!(core::mem::offset_of!(MadbProbeLayout, data) == 16);
    assert!(core::mem::offset_of!(MadbProbeLayout, length_bytes) == 24);
};

fn main() {
    let bytes = [255, 0, 1, 128];
    let mut sum = 99;
    let mut error = MadbError {
        domain: 99,
        code: 99,
    };
    // SAFETY: local, aligned, disjoint buffers live for every call. Null and
    // excessive-length cases must be rejected before the provider reads data.
    unsafe {
        assert_eq!(madb_probe_c_echo(u64::MAX), u64::MAX);
        assert_eq!(
            madb_probe_c_echo(9_007_199_254_740_993),
            9_007_199_254_740_993
        );
        assert_eq!(
            madb_c_fixture_checksum(bytes.as_ptr(), 4, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!((sum, error.domain, error.code), (384, 0, 0));
        assert_eq!(
            madb_c_fixture_checksum(core::ptr::null(), 0, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 0);
        assert_eq!(
            madb_c_fixture_checksum(bytes.as_ptr(), 0, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 0);
        sum = 99;
        error = MadbError {
            domain: 99,
            code: 99,
        };
        assert_eq!(
            madb_c_fixture_checksum(core::ptr::null(), 1, &mut sum, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!((sum, error.domain, error.code), (0, 0, 0));
        sum = 99;
        assert_eq!(
            madb_c_fixture_checksum(bytes.as_ptr(), u64::MAX, &mut sum, &mut error),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(sum, 0);
        assert_eq!(
            madb_c_fixture_checksum(bytes.as_ptr(), 1, core::ptr::null_mut(), &mut error),
            MADB_INVALID_ARGUMENT
        );
        sum = 99;
        assert_eq!(
            madb_c_fixture_checksum(bytes.as_ptr(), 1, &mut sum, core::ptr::null_mut()),
            MADB_INVALID_ARGUMENT
        );
        assert_eq!(sum, 0);
        let maximum = [255_u8; 65536];
        assert_eq!(
            madb_c_fixture_checksum(maximum.as_ptr(), 65536, &mut sum, &mut error),
            MADB_OK
        );
        assert_eq!(sum, 16_711_680);
    }
    println!("RUST_TO_C_OK cases=10");
}
