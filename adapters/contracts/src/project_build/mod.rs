//! Strict approved project-build formats. Public reports remain observations;
//! this adapter does not create Host receipts or perform filesystem work.
mod decode;
mod encode;
mod validate;
use madb_app::use_cases::project_build::*;
use madb_app::{Digest, ErrorKind, QueryError, Result};
use madb_protocol::{DigestDomain, DocumentKind, SchemaRegistry, Value};

pub struct ProjectBuildAdapter {
    registry: SchemaRegistry,
}
impl ProjectBuildAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry: SchemaRegistry::new().map_err(format_error)?,
        })
    }
    pub fn decode_report(&self, bytes: &[u8]) -> Result<ProjectReport> {
        let document = self
            .registry
            .decode(DocumentKind::ProjectBuildResult, bytes)
            .map_err(format_error)?;
        let report = decode::report(document.value())?;
        validate::report(self, &report)?;
        Ok(report)
    }
    pub fn descriptor_value(&self, descriptor: &Descriptor) -> Result<Value> {
        let value = encode::descriptor(descriptor);
        self.registry
            .validate(DocumentKind::ProjectBuild, value.clone())
            .map_err(format_error)?;
        validate::descriptor(descriptor)?;
        Ok(value)
    }
}
impl ProjectBuildFormat for ProjectBuildAdapter {
    fn decode_descriptor(&self, bytes: &[u8]) -> Result<Descriptor> {
        let document = self
            .registry
            .decode(DocumentKind::ProjectBuild, bytes)
            .map_err(format_error)?;
        let descriptor = decode::descriptor(document.value())?;
        validate::descriptor(&descriptor)?;
        Ok(descriptor)
    }
    fn encode_descriptor(&self, descriptor: &Descriptor) -> Result<Vec<u8>> {
        bytes(&self.descriptor_value(descriptor)?)
    }
    fn encode_report(&self, report: &ProjectReport) -> Result<Vec<u8>> {
        validate::report(self, report)?;
        let value = encode::report(report);
        self.registry
            .validate(DocumentKind::ProjectBuildResult, value.clone())
            .map_err(format_error)?;
        bytes(&value)
    }
    fn request_digest(&self, request: &RequestIdentity) -> Result<Digest> {
        if request.descriptor.path.as_str() != DESCRIPTOR {
            return Err(invalid(
                "descriptor",
                "request identifies another descriptor path",
            ));
        }
        digest(DigestDomain::ProjectBuildRequest, &encode::request(request))
    }
    fn input_digest(&self, descriptor: &FileIdentity, inputs: &InputInventory) -> Result<Digest> {
        validate::inputs(inputs, false)?;
        let captured = inputs
            .files
            .iter()
            .find(|f| f.source.as_str() == DESCRIPTOR)
            .ok_or_else(|| invalid("descriptor", "complete inputs omit the descriptor"))?;
        if descriptor.path.as_str() != DESCRIPTOR
            || captured.file.sha256 != descriptor.sha256
            || captured.file.size != descriptor.size
        {
            return Err(invalid(
                "descriptor",
                "request and captured descriptor identities differ",
            ));
        }
        digest(
            DigestDomain::ProjectBuildInput,
            &encode::input_projection(descriptor, inputs),
        )
    }
    fn output_digest(&self, tool: &ToolIdentity, output: &OutputProjection) -> Result<Digest> {
        validate::outputs(output)?;
        digest(
            DigestDomain::ProjectBuildOutput,
            &encode::output_projection(tool, output),
        )
    }
}
fn digest(domain: DigestDomain, value: &Value) -> Result<Digest> {
    Digest::new(madb_protocol::semantic_digest(domain, value).map_err(format_error)?)
        .map_err(|e| invalid("digest", e.to_string()))
}
fn bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = madb_protocol::canonical_json(value).map_err(format_error)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(invalid(
            "result",
            "project wire exceeds the unchanged 1 MiB decoder limit",
        ));
    }
    Ok(bytes)
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
