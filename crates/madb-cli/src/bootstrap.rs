//! The only composition root; commands do not construct adapters themselves.
use madb_app::{
    Result,
    query::{QueryOutcome, QueryRequest},
};
use std::path::Path;

pub fn query(root: &Path, request: &QueryRequest) -> Result<QueryOutcome> {
    let workspace = madb_host::LocalWorkspace::open(root)?;
    let format = madb_contracts::query::SnapshotAdapter::new()?;
    madb_app::query::query(&workspace, &format, request)
}
