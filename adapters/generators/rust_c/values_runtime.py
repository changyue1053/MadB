"""Fixed Rust support emitted for the reviewed madb.native-values/1 recipe."""

BASE_TYPES = """pub type MadbValueStatus = i32;
pub type MadbValueCallKind = u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MadbValueError {
    pub domain: u32,
    pub code: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MadbValueUtf8_64 {
    pub byte_count: u32,
    pub bytes: [u8; 64],
}
"""

CALL_KINDS = {
    "MADB_VALUE_CALL_OK": 0,
    "MADB_VALUE_CALL_BOOTSTRAP_FAILED": 1,
    "MADB_VALUE_CALL_INTERFACE_MISMATCH": 2,
    "MADB_VALUE_CALL_PROFILE_MISMATCH": 3,
    "MADB_VALUE_CALL_OPERATION_FAILED": 4,
}
STATUSES = {
    "MADB_VALUE_STATUS_OK": 0,
    "MADB_VALUE_STATUS_INVALID_ARGUMENT": 1,
    "MADB_VALUE_STATUS_INTERNAL_FAILURE": 8,
    "MADB_VALUE_STATUS_RESOURCE_EXHAUSTED": 9,
    "MADB_VALUE_STATUS_DOMAIN_ERROR": 10,
    "MADB_VALUE_STATUS_UNSUPPORTED": 11,
}
BASE_NAMES = ["MadbValueStatus", "MadbValueCallKind", "MadbValueError", "MadbValueUtf8_64"]

RUNTIME = """mod madb_value_runtime {
    use super::{MadbValueError, MadbValueStatus, MadbValueUtf8_64};

    pub(super) type Failure = (MadbValueStatus, MadbValueError);

    pub(super) fn failure(status: MadbValueStatus) -> Failure {
        (status, MadbValueError { domain: 0, code: 0 })
    }

    struct AbortOnUnwind;

    impl Drop for AbortOnUnwind {
        fn drop(&mut self) {
            if std::thread::panicking() {
                std::process::abort();
            }
        }
    }

    pub(super) fn dispose_panic(payload: Box<dyn std::any::Any + Send>) {
        // A caught payload may itself panic when dropped. Release it inside
        // another catch, including its allocation and any fields during unwind.
        if let Err(secondary) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload))) {
            // Cleanup is bounded: release the secondary payload normally, but
            // abort if its destructor panics too. Never leak or unwind into C.
            let abort_on_unwind = AbortOnUnwind;
            drop(secondary);
            drop(abort_on_unwind);
        }
    }

    // Mechanical checks do not establish allocation validity or non-aliasing.
    // Those remain the documented raw caller preconditions.
    pub(super) fn usable<T>(pointer: *const T) -> bool {
        !pointer.is_null() && (pointer as usize) % std::mem::align_of::<T>() == 0
    }

    pub(super) fn text(value: &MadbValueUtf8_64) -> Result<&str, Failure> {
        let count = value.byte_count as usize;
        if count > value.bytes.len() {
            return Err(failure(1));
        }
        std::str::from_utf8(&value.bytes[..count]).map_err(|_| failure(1))
    }

    pub(super) fn owned_text(value: &str) -> Result<String, Failure> {
        let mut result = String::new();
        result.try_reserve_exact(value.len()).map_err(|_| failure(9))?;
        result.push_str(value);
        Ok(result)
    }

    pub(super) fn encoded_text(value: &str) -> MadbValueUtf8_64 {
        // Every output length is checked before any output field is encoded.
        let mut result = MadbValueUtf8_64 { byte_count: value.len() as u32, bytes: [0; 64] };
        result.bytes[..value.len()].copy_from_slice(value.as_bytes());
        result
    }
}
"""
