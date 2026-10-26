//! Pure contract checks. No schema decoding, compiler invocation or certification IO.
mod contract;
mod evidence;
mod identity;
mod observations;
mod ownership;
mod policies;

pub use contract::{validate_constant, validate_contract};
pub use evidence::{evidence_freshness, instantiate_requirements, validate_required_results};
pub use identity::validate_identities;
pub use ownership::{OwnerSetError, compare_unique_owners};
pub use policies::resolve_policies;
