//! Safe, synchronous checksum calls to the packaged C provider.
//!
//! [`Client::connect`] checks the linked library's interface and profile identities
//! before granting access to business calls. The stateless C provider keeps no
//! pointers or resources, so a client may be shared between threads. Calls borrow
//! their input for the duration of the call and accept at most 65536 bytes.
//!
//! Consume the complete candidate: generated bindings, generated identities, and
//! the adjacent native library are required at build time.
//!
//! ```no_run
//! use madb_c_client::Client;
//!
//! let client = Client::connect()?;
//! assert_eq!(client.checksum(&[1, 2, 255])?, 258);
//! # Ok::<(), madb_c_client::Error>(())
//! ```
#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

mod identity;
#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
mod raw;

/// An initialization or checksum failure. Native status/domain/code values remain
/// available without exposing unchecked pointers or bypassing identity checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The loaded provider's interface digest differs from the generated binding.
    AbiMismatch,
    /// The interface matches, but the loaded provider's profile digest differs.
    ProfileMismatch,
    /// The provider does not support this bootstrap version or operation.
    Unsupported,
    /// A native failure, or an input rejected by the same native argument limit.
    Native { status: i32, domain: u32, code: i32 },
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiMismatch => output.write_str("C provider interface digest mismatch"),
            Self::ProfileMismatch => output.write_str("C provider profile digest mismatch"),
            Self::Unsupported => {
                output.write_str("C provider operation or bootstrap version unsupported")
            }
            Self::Native {
                status,
                domain,
                code,
            } => write!(
                output,
                "C provider status {status}, domain {domain}, code {code}"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Access to the linked stateless C provider after a successful identity check.
///
/// Creation performs only the allocation-free bootstrap query. There is no
/// resource to close and no background work; checksum runs on the calling thread.
#[derive(Debug)]
pub struct Client {
    _verified: (),
}

impl Client {
    /// Verify ABI and profile identity before enabling any checksum calls.
    ///
    /// Interface mismatch is reported before profile mismatch. A bootstrap error
    /// is returned before either digest is interpreted.
    pub fn connect() -> Result<Self, Error> {
        let mut actual = [0_u8; 64];
        // SAFETY: this fixed bootstrap signature precedes business ABI validation.
        // The full, writable 64-byte output remains alive until the call returns.
        let status = unsafe { raw::madb_c_fixture_bootstrap(1, actual.as_mut_ptr(), 64) };
        check_status(status, 0, 0)?;
        if actual[..32] != identity::INTERFACE_DIGEST {
            return Err(Error::AbiMismatch);
        }
        if actual[32..] != identity::PROFILE_DIGEST {
            return Err(Error::ProfileMismatch);
        }
        Ok(Self { _verified: () })
    }

    /// Return the unsigned sum of bytes, with zero for an empty slice.
    ///
    /// Inputs longer than 65536 bytes return native `InvalidArgument` (status 1)
    /// without entering C. The provider does not retain the slice or outputs.
    pub fn checksum(&self, data: &[u8]) -> Result<u64, Error> {
        if data.len() > 65_536 {
            return Err(Error::Native {
                status: raw::MADB_INVALID_ARGUMENT,
                domain: 0,
                code: 0,
            });
        }
        let mut sum = 0;
        let mut error = raw::MadbError { domain: 0, code: 0 };
        // SAFETY: connect verified this business ABI and profile. The bounded
        // slice remains readable; stack outputs are aligned, writable, distinct,
        // and disjoint from the borrowed slice throughout this synchronous call.
        let status = unsafe {
            raw::madb_c_fixture_checksum(data.as_ptr(), data.len() as u64, &mut sum, &mut error)
        };
        check_status(status, error.domain, error.code)?;
        Ok(sum)
    }
}

fn check_status(status: i32, domain: u32, code: i32) -> Result<(), Error> {
    match status {
        raw::MADB_OK => Ok(()),
        raw::MADB_UNSUPPORTED => Err(Error::Unsupported),
        _ => Err(Error::Native {
            status,
            domain,
            code,
        }),
    }
}
