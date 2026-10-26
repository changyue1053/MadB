use crate::{FormatError, MAX_DOCUMENT_BYTES, Value, canonical_json, strict_json};
use jsonschema::{Draft, PatternOptions, Registry, Validator};
use std::collections::BTreeMap;

macro_rules! schema {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/",
            $name,
            ".schema.json"
        ))
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DocumentKind {
    Project,
    ContractIr,
    Lock,
    Profile,
    ProfileV2,
    Composition,
    Case,
    Coverage,
    Report,
    ReadRequest,
    GenerateRequest,
    OutputSet,
    BuildTask,
    BuildEvidence,
    PackageRequest,
    Candidate,
    VerifyRequest,
    Execution,
    DiagnosticContext,
    ExplainResult,
}
impl DocumentKind {
    pub const ALL: [Self; 20] = [
        Self::Project,
        Self::ContractIr,
        Self::Lock,
        Self::Profile,
        Self::ProfileV2,
        Self::Composition,
        Self::Case,
        Self::Coverage,
        Self::Report,
        Self::ReadRequest,
        Self::GenerateRequest,
        Self::OutputSet,
        Self::BuildTask,
        Self::BuildEvidence,
        Self::PackageRequest,
        Self::Candidate,
        Self::VerifyRequest,
        Self::Execution,
        Self::DiagnosticContext,
        Self::ExplainResult,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::ContractIr => "contract-ir",
            Self::Lock => "lock",
            Self::Profile => "profile",
            Self::ProfileV2 => "profile-v2",
            Self::Composition => "composition",
            Self::Case => "case",
            Self::Coverage => "coverage",
            Self::Report => "report",
            Self::ReadRequest => "read-request",
            Self::GenerateRequest => "generate-request",
            Self::OutputSet => "output-set",
            Self::BuildTask => "build-task",
            Self::BuildEvidence => "build-evidence",
            Self::PackageRequest => "package-request",
            Self::Candidate => "candidate",
            Self::VerifyRequest => "verify-request",
            Self::Execution => "execution",
            Self::DiagnosticContext => "diagnostic-context",
            Self::ExplainResult => "explain-result",
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::Project => schema!("project"),
            Self::ContractIr => schema!("contract-ir"),
            Self::Lock => schema!("lock"),
            Self::Profile => schema!("profile"),
            Self::ProfileV2 => schema!("profile-v2"),
            Self::Composition => schema!("composition"),
            Self::Case => schema!("case"),
            Self::Coverage => schema!("coverage"),
            Self::Report => schema!("report"),
            Self::ReadRequest => schema!("read-request"),
            Self::GenerateRequest => schema!("generate-request"),
            Self::OutputSet => schema!("output-set"),
            Self::BuildTask => schema!("build-task"),
            Self::BuildEvidence => schema!("build-evidence"),
            Self::PackageRequest => schema!("package-request"),
            Self::Candidate => schema!("candidate"),
            Self::VerifyRequest => schema!("verify-request"),
            Self::Execution => schema!("execution"),
            Self::DiagnosticContext => schema!("diagnostic-context"),
            Self::ExplainResult => schema!("explain-result"),
        }
    }
    pub fn schema_id(self) -> String {
        match self {
            Self::ProfileV2 => "madb.profile/2".into(),
            _ => format!("madb.{}/1", self.name()),
        }
    }
}

/// Immutable validated wire data. Domain conversion must remain explicit.
#[derive(Clone, Debug)]
pub struct Document {
    kind: DocumentKind,
    value: Value,
}
impl Document {
    pub fn kind(&self) -> DocumentKind {
        self.kind
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn into_value(self) -> Value {
        self.value
    }
}

/// Only bundled schemas are used; untrusted documents cannot trigger retrieval.
pub struct SchemaRegistry {
    validators: BTreeMap<DocumentKind, Validator>,
}
impl SchemaRegistry {
    pub fn new() -> Result<Self, FormatError> {
        let common: Value = serde_json::from_str(schema!("common")).map_err(schema_error)?;
        let registry = Registry::new()
            .add("urn:madb:schema:common:1", common)
            .map_err(schema_error)?
            .prepare()
            .map_err(schema_error)?;
        let mut validators = BTreeMap::new();
        for kind in DocumentKind::ALL {
            let schema: Value = serde_json::from_str(kind.source()).map_err(schema_error)?;
            let validator = jsonschema::options()
                .with_draft(Draft::Draft202012)
                .with_registry(&registry)
                .offline()
                .with_pattern_options(PatternOptions::regex())
                .should_validate_formats(true)
                .build(&schema)
                .map_err(schema_error)?;
            validators.insert(kind, validator);
        }
        Ok(Self { validators })
    }
    /// Validate a trusted bundled auxiliary schema. Untrusted input supplies
    /// only payload bytes; offline resolution permits only the bundled common schema.
    pub fn validate_bundled(schema: &Value, bytes: &[u8]) -> Result<Value, FormatError> {
        let value = strict_json(bytes)?;
        let common: Value = serde_json::from_str(schema!("common")).map_err(schema_error)?;
        let registry = Registry::new()
            .add("urn:madb:schema:common:1", common)
            .map_err(schema_error)?
            .prepare()
            .map_err(schema_error)?;
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_registry(&registry)
            .offline()
            .with_pattern_options(PatternOptions::regex())
            .should_validate_formats(true)
            .build(schema)
            .map_err(schema_error)?;
        if let Some(error) = validator.iter_errors(&value).next() {
            return Err(FormatError::new(
                "FMT004",
                error.instance_path().to_string(),
                error.to_string(),
            ));
        }
        Ok(value)
    }

    pub fn decode(&self, kind: DocumentKind, bytes: &[u8]) -> Result<Document, FormatError> {
        self.validate(kind, strict_json(bytes)?)
    }
    pub fn validate(&self, kind: DocumentKind, value: Value) -> Result<Document, FormatError> {
        // Also cover callers supplying a Value without going through strict_json.
        if canonical_json(&value)?.len() > MAX_DOCUMENT_BYTES {
            return Err(FormatError::new(
                "FMT002",
                "",
                "document exceeds byte budget",
            ));
        }
        if value.get("schema").and_then(Value::as_str) != Some(kind.schema_id().as_str()) {
            return Err(FormatError::new(
                "FMT003",
                "/schema",
                "wrong or unsupported schema version",
            ));
        }
        if let Some(error) = self.validators[&kind].iter_errors(&value).next() {
            return Err(FormatError::new(
                "FMT004",
                error.instance_path().to_string(),
                error.to_string(),
            ));
        }
        Ok(Document { kind, value })
    }
    pub fn decode_project_toml(&self, text: &str) -> Result<Document, FormatError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(FormatError::new(
                "FMT002",
                "",
                "document exceeds byte budget",
            ));
        }
        let value: toml::Value =
            toml::from_str(text).map_err(|e| FormatError::new("FMT001", "", e.to_string()))?;
        let json = serde_json::to_value(value).map_err(schema_error)?;
        self.validate(DocumentKind::Project, json)
    }
}
fn schema_error(error: impl std::fmt::Display) -> FormatError {
    FormatError::new("FMT000", "", error.to_string())
}
