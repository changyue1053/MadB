//! Explicit initialization wire/domain conversion. No filesystem, package
//! extraction, tool execution, or creation authority is introduced here.
mod decode;
mod diagnostic;
mod inputs;
mod report;
mod wire;
pub use inputs::*;
use madb_app::use_cases::initialize::{
    InitReport, InitRequestDigest, InitResultFormat, ResolvedRequest,
};
use madb_app::{Digest, ErrorKind, QueryError, Result};
use madb_plan::project::CheckedTemplate;
use madb_protocol::{DocumentKind, SchemaRegistry};

pub struct ProjectInitAdapter {
    registry: SchemaRegistry,
}
impl ProjectInitAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry: SchemaRegistry::new().map_err(format_error)?,
        })
    }
    pub fn decode_template(&self, bytes: &[u8]) -> Result<CheckedTemplate> {
        let document = self
            .registry
            .decode(DocumentKind::ProjectTemplate, bytes)
            .map_err(format_error)?;
        decode::template(document.value())
    }
    /// Decoding a report returns observations only, never a Host receipt or a
    /// CompletedInitialization. Cross-field checks do not prove execution.
    pub fn decode_result(&self, bytes: &[u8]) -> Result<InitReport> {
        let document = self
            .registry
            .decode(DocumentKind::InitResult, bytes)
            .map_err(format_error)?;
        report::decode_report(document.value())
    }
}
impl InitRequestDigest for ProjectInitAdapter {
    fn digest(&self, p: &ResolvedRequest) -> Result<Digest> {
        let projection = report::request_projection(p);
        Digest::new(
            madb_protocol::semantic_digest(madb_protocol::DigestDomain::InitRequest, &projection)
                .map_err(format_error)?,
        )
        .map_err(|e| invalid("request_digest", e.to_string()))
    }
}
impl InitResultFormat for ProjectInitAdapter {
    fn encode_result(&self, report: &InitReport) -> Result<Vec<u8>> {
        let value = report::encode_report(report);
        self.registry
            .validate(DocumentKind::InitResult, value.clone())
            .map_err(format_error)?;
        // The same observation-level checks apply to encoded and decoded values.
        report::decode_report(&value)?;
        let mut bytes = madb_protocol::canonical_json(&value).map_err(format_error)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
fn invalid(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
fn format_error(error: madb_protocol::FormatError) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        error.code,
        error.path,
        error.message,
    )
}
