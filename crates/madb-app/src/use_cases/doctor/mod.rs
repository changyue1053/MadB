//! Read-only preflight. Observations never become process or commit permits.
mod query;
mod result;
mod types;
pub use query::*;
pub use result::*;
pub use types::*;

mod validation;
