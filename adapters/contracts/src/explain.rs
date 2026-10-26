//! Pure, bundled explanation catalog and schema-to-domain adapter. No filesystem,
//! compiler invocation, network, candidate certification or automatic repair.
use crate::{DocumentKind, FormatError, SchemaRegistry};
use madb_app::explain::{
    BoundSource, DiagnosticContext, ExplainFormat, ExplainSuccess, Explanation, LocatedSource,
    PositionEncoding, Repair, RepairKind,
};
use madb_app::ports::FileReference;
use madb_app::{Digest, ErrorKind, Id, LogicalPath, QueryError, Result};
use madb_model::{Location, contract::Precision};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const CATALOG: &[u8] = include_bytes!("../../generators/diagnostic-explanations.json");
pub struct ExplainAdapter {
    registry: SchemaRegistry,
    entries: BTreeMap<Id, Explanation>,
}
impl ExplainAdapter {
    pub fn new() -> Result<Self> {
        let registry = SchemaRegistry::new().map_err(format_error)?;
        let catalog = madb_protocol::strict_json(CATALOG).map_err(format_error)?;
        if catalog["version"] != 1 {
            return Err(invalid("catalog", "unsupported bundled catalog version"));
        }
        let mut entries = BTreeMap::new();
        let items = catalog["entries"]
            .as_array()
            .ok_or_else(|| invalid("catalog", "missing bundled entries"))?;
        for item in items {
            let code = id(&item["code"])?;
            let explanation = explanation(item)?;
            if entries.insert(code, explanation).is_some() {
                return Err(invalid("catalog", "duplicate bundled explanation"));
            }
        }
        let mut expected = Vec::new();
        for (prefix, count) in [
            ("NATIVE", 6),
            ("SEM", 9),
            ("POL", 10),
            ("CAP", 4),
            ("TYPE", 9),
            ("ID", 6),
            ("NUM", 1),
            ("EXPLAIN", 4),
        ] {
            for number in 1..=count {
                expected.push(format!("{prefix}{number:03}"));
            }
        }
        expected.extend([
            "ARKTS_DECL_UNSUPPORTED_MODIFIERS".into(),
            "ARKTS_DECL_CALLBACK_RESULT".into(),
        ]);
        expected.sort();
        if entries
            .keys()
            .map(|code| code.as_str())
            .ne(expected.iter().map(String::as_str))
        {
            return Err(invalid(
                "catalog",
                "bundled catalog differs from the adopted finite code set",
            ));
        }
        let result = Self { registry, entries };
        // Validate every bundled entry against the actual result schema, not only
        // the selected one. A malformed bundled explanation is never published.
        for (code, value) in &result.entries {
            result.encode_result(
                "catalog-validation",
                Some(code),
                &Ok(ExplainSuccess {
                    explanation: value.clone(),
                    context: None,
                    source: None,
                }),
            )?;
        }
        Ok(result)
    }

    /// One JSON object plus LF. CLI supplies the actual executable version and
    /// must use the same ErrorKind-to-exit mapping; this does not write stdout.
    pub fn encode_result(
        &self,
        tool_version: &str,
        code: Option<&Id>,
        result: &Result<ExplainSuccess>,
    ) -> Result<Vec<u8>> {
        if let Ok(success) = result {
            if let Some(bound) = &success.source
                && (success.context.is_none()
                    || bound.source.reference.path != bound.source.location.path
                    || bound.byte_offset > bound.source.reference.size)
            {
                return Err(invalid(
                    "result",
                    "source point is inconsistent with its context/reference",
                ));
            }
            let location = success.source.as_ref().map(|bound| &bound.source.location);
            if success.explanation.repairs.iter().any(|repair| {
                repair
                    .location
                    .as_ref()
                    .is_some_and(|point| Some(point) != location)
            }) {
                return Err(invalid(
                    "result",
                    "repair location is not the bound source point",
                ));
            }
        }
        let (context, explanation, source, diagnostics, exit_code) = match result {
            Ok(success) => (
                success.context.as_ref().map(|reference| {
                    let mut value = file_value(reference);
                    value["schema"] = json!("madb.diagnostic-context/1");
                    value
                }),
                Some(explanation_value(&success.explanation)),
                success.source.as_ref().map(source_value),
                Vec::new(),
                0,
            ),
            Err(error) => {
                let diagnostics = error
                    .findings
                    .iter()
                    .take(64)
                    .map(|finding| {
                        json!({"code":finding.code,"message":bounded_message(&finding.message),
                        "location":finding.locations.first().map(location_value)})
                    })
                    .collect();
                let exit = match error.kind {
                    ErrorKind::InvalidInput | ErrorKind::CheckFailed => 2,
                    ErrorKind::Host => 3,
                    ErrorKind::Conflict => 4,
                };
                (None, None, None, diagnostics, exit)
            }
        };
        let value = json!({"schema":"madb.explain-result/1","tool_version":tool_version,
            "code":code.map(Id::as_str),"context":context,"explanation":explanation,
            "source":source,"diagnostics":diagnostics,"exit_code":exit_code});
        self.registry
            .validate(DocumentKind::ExplainResult, value.clone())
            .map_err(format_error)?;
        let mut bytes =
            serde_json::to_vec(&value).map_err(|error| invalid("result", error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
impl ExplainFormat for ExplainAdapter {
    fn lookup(&self, code: &Id) -> Result<Option<Explanation>> {
        Ok(self.entries.get(code).cloned())
    }
    fn decode_context(&self, bytes: &[u8]) -> Result<DiagnosticContext> {
        let document = self
            .registry
            .decode(DocumentKind::DiagnosticContext, bytes)
            .map_err(format_error)?;
        let value = document.value();
        let diagnostic = &value["diagnostic"];
        let source = if value["source"].is_null() {
            None
        } else {
            let reference = reference(&value["source"])?;
            let location = location(&diagnostic["location"])?;
            if reference.path != location.path {
                return Err(invalid("source", "source and diagnostic paths differ"));
            }
            let encoding = match value["position_encoding"].as_str() {
                Some("byte-lf") => PositionEncoding::ByteLf,
                Some("utf16-js") => PositionEncoding::Utf16Js,
                _ => return Err(invalid("position_encoding", "unknown position encoding")),
            };
            let precision = match value["reported_precision"].as_str() {
                Some("compiler-derived") => Some(Precision::CompilerDerived),
                Some("artifact-derived") => Some(Precision::ArtifactDerived),
                Some("schema-derived") => Some(Precision::SchemaDerived),
                Some("heuristic") => Some(Precision::Heuristic),
                None if value["reported_precision"].is_null() => None,
                _ => return Err(invalid("reported_precision", "unknown source precision")),
            };
            Some(LocatedSource {
                reference,
                location,
                encoding,
                precision,
            })
        };
        Ok(DiagnosticContext {
            code: id(&diagnostic["code"])?,
            message: string(&diagnostic["message"])?.into(),
            source,
        })
    }
}
fn explanation(value: &Value) -> Result<Explanation> {
    let repairs = value["repairs"]
        .as_array()
        .ok_or_else(|| invalid("catalog", "repairs must be an array"))?
        .iter()
        .map(|repair| {
            let kind = match repair["kind"].as_str() {
                Some("edit-author-input") => RepairKind::EditAuthorInput,
                Some("inspect-configuration") => RepairKind::InspectConfiguration,
                Some("restore-locked-input") => RepairKind::RestoreLockedInput,
                _ => return Err(invalid("catalog", "unknown bundled repair kind")),
            };
            Ok(Repair {
                kind,
                instruction: string(&repair["instruction"])?.into(),
                location: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Explanation {
        meaning: string(&value["meaning"])?.into(),
        triggers: strings(&value["triggers"])?,
        repairs,
        limits: strings(&value["limits"])?,
    })
}
fn strings(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| invalid("catalog", "expected string array"))?
        .iter()
        .map(|item| string(item).map(String::from))
        .collect()
}
fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("document", "expected string"))
}
fn id(value: &Value) -> Result<Id> {
    Id::new(string(value)?).map_err(|error| invalid("code", error.to_string()))
}
fn reference(value: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: LogicalPath::new(string(&value["path"])?)
            .map_err(|error| invalid("path", error.to_string()))?,
        digest: Digest::new(string(&value["sha256"])?)
            .map_err(|error| invalid("sha256", error.to_string()))?,
        size: value["size"]
            .as_u64()
            .ok_or_else(|| invalid("size", "expected safe integer"))?,
    })
}
fn location(value: &Value) -> Result<Location> {
    Ok(Location {
        path: LogicalPath::new(string(&value["path"])?)
            .map_err(|error| invalid("path", error.to_string()))?,
        line: value["line"]
            .as_u64()
            .ok_or_else(|| invalid("line", "expected one-based line"))?,
        column: value["column"]
            .as_u64()
            .ok_or_else(|| invalid("column", "expected one-based column"))?,
    })
}
fn file_value(reference: &FileReference) -> Value {
    json!({"path":reference.path.as_str(),"sha256":reference.digest.as_str(),"size":reference.size})
}
fn location_value(location: &Location) -> Value {
    json!({"path":location.path.as_str(),"line":location.line,"column":location.column})
}
fn source_value(bound: &BoundSource) -> Value {
    let source = &bound.source;
    let precision = source.precision.map(|value| match value {
        Precision::CompilerDerived => "compiler-derived",
        Precision::ArtifactDerived => "artifact-derived",
        Precision::SchemaDerived => "schema-derived",
        Precision::Heuristic => "heuristic",
    });
    json!({"reference":file_value(&source.reference),"location":location_value(&source.location),
        "position_encoding":match source.encoding {PositionEncoding::ByteLf=>"byte-lf",PositionEncoding::Utf16Js=>"utf16-js"},
        "reported_precision":precision,"byte_offset":bound.byte_offset})
}
fn explanation_value(value: &Explanation) -> Value {
    let repairs: Vec<_> = value.repairs.iter().map(|repair| json!({
        "kind":match repair.kind {RepairKind::EditAuthorInput=>"edit-author-input",RepairKind::InspectConfiguration=>"inspect-configuration",RepairKind::RestoreLockedInput=>"restore-locked-input"},
        "instruction":repair.instruction,"location":repair.location.as_ref().map(location_value),
    })).collect();
    json!({"meaning":value.meaning,"triggers":value.triggers,"repairs":repairs,"limits":value.limits})
}
fn bounded_message(message: &str) -> String {
    if message.is_empty() {
        "explanation query failed".into()
    } else {
        message.chars().take(4096).collect()
    }
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "EXPLAIN002", subject, message)
}
fn format_error(error: FormatError) -> QueryError {
    invalid(
        &error.path,
        format!("{}: {}", error.code, bounded_message(&error.message)),
    )
}
