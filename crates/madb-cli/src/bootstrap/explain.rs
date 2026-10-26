//! Compose the read-only query without opening any workspace for code lookup.
use madb_app::{
    ErrorKind, Id, LogicalPath, QueryError, Result,
    explain::{ExplainFormat, ExplainRequest, ExplainSuccess},
    ports::ReadWorkspace,
};
use madb_contracts::explain::ExplainAdapter;
use std::path::Path;

pub struct ExplanationService {
    format: ExplainAdapter,
}
impl ExplanationService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            format: ExplainAdapter::new()?,
        })
    }

    pub fn query(&self, root: &Path, request: &ExplainRequest) -> Result<ExplainSuccess> {
        // Unknown codes must fail before a root/context is inspected, including
        // when the caller supplied an inaccessible root or context path.
        if request.context.is_none() || self.format.lookup(&request.code)?.is_none() {
            return madb_app::explain::explain(&NoWorkspace, &self.format, request);
        }
        let workspace = madb_host::DiagnosticWorkspace::open(root).map_err(|cause| {
            let (kind, code) = match cause.kind {
                ErrorKind::Host => (ErrorKind::Host, "EXPLAIN003"),
                ErrorKind::Conflict => (ErrorKind::Conflict, "EXPLAIN004"),
                _ => (ErrorKind::InvalidInput, "EXPLAIN002"),
            };
            QueryError::new(kind, code, "root", cause.to_string())
        })?;
        madb_app::explain::explain(&workspace, &self.format, request)
    }

    pub fn encode(&self, code: Option<&Id>, result: &Result<ExplainSuccess>) -> Result<Vec<u8>> {
        self.format
            .encode_result(env!("CARGO_PKG_VERSION"), code, result)
    }
}

struct NoWorkspace;
impl ReadWorkspace for NoWorkspace {
    fn read(&self, _: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
        Err(QueryError::new(
            ErrorKind::Host,
            "EXPLAIN003",
            "query",
            "code-only explanation attempted a workspace read",
        ))
    }
}
