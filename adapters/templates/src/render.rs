use crate::{AssetError, AssetErrorKind, CheckedInventory};
use madb_model::Id;
use madb_plan::project::{
    FileOwnership, IdentifierLanguage, ParameterValue, Renderer, TemplateAsset, pointer_tokens,
    validate_identifier,
};
use madb_protocol::{
    Document, DocumentKind, MAX_DOCUMENT_BYTES, SchemaRegistry, Value, canonical_json, strict_json,
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_BINDINGS: usize = 4096;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MARKER: &str = "@@MADB:";

/// Render one declared asset from a complete checked package. Local-state
/// reservations return None and never produce an empty placeholder file.
pub fn render_asset(
    asset: &TemplateAsset,
    package: &CheckedInventory,
    parameters: &BTreeMap<Id, ParameterValue>,
    registry: &SchemaRegistry,
    validate_document: impl FnMut(&Document) -> Result<(), AssetError>,
) -> Result<Option<Vec<u8>>, AssetError> {
    if asset.ownership == FileOwnership::LocalState {
        if asset.source.is_some() || asset.renderer.is_some() {
            return Err(render_error(
                asset.id.as_str(),
                "local-state must reserve a path without source or renderer",
            ));
        }
        return Ok(None);
    }
    let source = asset
        .source
        .as_ref()
        .ok_or_else(|| render_error(asset.id.as_str(), "asset has no source"))?;
    let renderer = asset
        .renderer
        .as_ref()
        .ok_or_else(|| render_error(asset.id.as_str(), "asset has no renderer"))?;
    render_bytes(
        renderer,
        package.referenced_bytes(source)?,
        parameters,
        registry,
        validate_document,
    )
    .map(Some)
}

/// Deterministic rendering only. Recognized MadB JSON output is decoded by the
/// existing registry and then passed to the caller's semantic validator; that
/// validator must perform the applicable contracts and frozen-reference checks.
/// This function does not grant recipe execution or create a checked workspace.
pub fn render_bytes(
    renderer: &Renderer,
    source: &[u8],
    parameters: &BTreeMap<Id, ParameterValue>,
    registry: &SchemaRegistry,
    mut validate_document: impl FnMut(&Document) -> Result<(), AssetError>,
) -> Result<Vec<u8>, AssetError> {
    if source.len() > MAX_FILE_BYTES {
        return Err(render_error(
            "source",
            "asset exceeds individual file budget",
        ));
    }
    match renderer {
        Renderer::Copy => Ok(source.to_vec()),
        Renderer::JsonValues { bindings } => {
            if bindings.len() > MAX_BINDINGS {
                return Err(render_error(
                    "bindings",
                    "JSON binding count exceeds budget",
                ));
            }
            let mut value = strict_json(source).map_err(format_error)?;
            check_json_array_budget(&value)?;
            let source_kind = document_kind(&value)?;
            let mut addresses: Vec<Vec<String>> = Vec::new();
            // Resolve every address and parameter before mutating the JSON. A
            // failed later binding must not reinterpret an earlier replacement.
            let mut replacements = Vec::new();
            for binding in bindings {
                if binding.pointer.len() > 4096 {
                    return Err(render_error("pointer", "JSON pointer exceeds byte budget"));
                }
                let path = pointer_tokens(&binding.pointer)
                    .map_err(|error| render_error(error.subject, error.message))?;
                if addresses
                    .iter()
                    .any(|other| path.starts_with(other) || other.starts_with(&path))
                {
                    return Err(render_error(
                        &binding.pointer,
                        "JSON pointer targets overlap or alias",
                    ));
                }
                let target = locate(&value, &path)?;
                if target.is_array() || target.is_object() {
                    return Err(render_error(
                        &binding.pointer,
                        "JSON pointer must address an existing scalar or null",
                    ));
                }
                let parameter = parameters.get(&binding.parameter).ok_or_else(|| {
                    render_error(
                        binding.parameter.as_str(),
                        "renderer parameter has not been resolved",
                    )
                })?;
                let replacement = parameter_json(parameter)?;
                addresses.push(path.clone());
                replacements.push((path, replacement));
            }
            for (path, replacement) in replacements {
                *locate_mut(&mut value, &path)? = replacement;
            }
            let output_kind = document_kind(&value)?;
            if source_kind.is_some() && source_kind != output_kind {
                return Err(render_error(
                    "schema",
                    "rendering cannot remove or change an existing MadB document kind",
                ));
            }
            let mut bytes = canonical_json(&value).map_err(format_error)?;
            bytes.push(b'\n');
            if bytes.len() > MAX_DOCUMENT_BYTES {
                return Err(render_error(
                    "output",
                    "rendered JSON exceeds document byte budget",
                ));
            }
            if let Some(kind) = output_kind {
                let document = registry.decode(kind, &bytes).map_err(format_error)?;
                validate_document(&document)?;
            }
            Ok(bytes)
        }
        Renderer::Identifiers {
            language,
            parameters: names,
        } => {
            if matches!(language, IdentifierLanguage::Unknown(_)) {
                return Err(render_error(
                    "language",
                    "unsupported identifier renderer language",
                ));
            }
            render_identifiers(source, names, parameters)
        }
        Renderer::Unknown { id } => Err(render_error(id.as_str(), "unsupported renderer")),
    }
}

fn render_identifiers(
    source: &[u8],
    names: &[Id],
    parameters: &BTreeMap<Id, ParameterValue>,
) -> Result<Vec<u8>, AssetError> {
    if names.is_empty() || names.len() > MAX_BINDINGS {
        return Err(render_error(
            "parameters",
            "identifier parameters must be a bounded nonempty set",
        ));
    }
    let mut values = BTreeMap::new();
    for name in names {
        let Some(ParameterValue::String(value)) = parameters.get(name) else {
            return Err(render_error(
                name.as_str(),
                "identifier renderer needs a resolved string parameter",
            ));
        };
        validate_identifier(value)
            .map_err(|error| render_error(name.as_str(), error.to_string()))?;
        if values.insert(name.as_str(), value.as_str()).is_some() {
            return Err(render_error(
                name.as_str(),
                "duplicate identifier parameter",
            ));
        }
    }
    let source = std::str::from_utf8(source)
        .map_err(|_| render_error("source", "identifier source must be UTF-8"))?;
    let mut cursor = 0;
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(relative) = source[cursor..].find(MARKER) {
        let start = cursor + relative;
        append_bounded(&mut output, &source.as_bytes()[cursor..start])?;
        let body_start = start + MARKER.len();
        let body_end = source[body_start..]
            .find("@@")
            .map(|end| body_start + end)
            .ok_or_else(|| render_error("source", "unclosed identifier marker"))?;
        let name = &source[body_start..body_end];
        if source[body_end + 2..].starts_with("MADB:") {
            return Err(render_error(
                "source",
                "overlapping or nested identifier marker",
            ));
        }
        let value = values
            .get(name)
            .ok_or_else(|| render_error(name, "unknown or malformed identifier marker"))?;
        append_bounded(&mut output, value.as_bytes())?;
        seen.insert(name);
        cursor = body_end + 2;
    }
    append_bounded(&mut output, &source.as_bytes()[cursor..])?;
    if values.keys().any(|name| !seen.contains(name)) {
        return Err(render_error(
            "source",
            "a declared identifier parameter has no marker",
        ));
    }
    if output
        .windows(MARKER.len())
        .any(|window| window == MARKER.as_bytes())
    {
        return Err(render_error(
            "output",
            "identifier marker remains after rendering",
        ));
    }
    Ok(output)
}

fn append_bounded(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), AssetError> {
    if output
        .len()
        .checked_add(bytes.len())
        .is_none_or(|size| size > MAX_FILE_BYTES)
    {
        return Err(render_error(
            "output",
            "rendered asset exceeds individual file budget",
        ));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn document_kind(value: &Value) -> Result<Option<DocumentKind>, AssetError> {
    let Some(schema) = value.get("schema").and_then(Value::as_str) else {
        return Ok(None);
    };
    let kind = DocumentKind::ALL
        .into_iter()
        .find(|kind| kind.schema_id() == schema);
    if kind.is_none() && schema.starts_with("madb.") {
        return Err(render_error("schema", "unrecognized MadB document schema"));
    }
    Ok(kind)
}

fn check_json_array_budget(value: &Value) -> Result<(), AssetError> {
    match value {
        Value::Array(items) => {
            if items.len() > MAX_BINDINGS {
                return Err(render_error("source", "JSON array exceeds 4096 items"));
            }
            for item in items {
                check_json_array_budget(item)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                check_json_array_budget(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parameter_json(value: &ParameterValue) -> Result<Value, AssetError> {
    match value {
        ParameterValue::String(value) if value.chars().count() <= 4096 => {
            Ok(Value::String(value.clone()))
        }
        ParameterValue::Boolean(value) => Ok(Value::Bool(*value)),
        ParameterValue::Integer(value)
            if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(value) =>
        {
            Ok(Value::from(*value))
        }
        _ => Err(render_error(
            "parameter",
            "parameter value exceeds its wire scalar budget",
        )),
    }
}

fn array_index(text: &str, length: usize) -> Result<usize, AssetError> {
    if text.is_empty()
        || (text.starts_with('0') && text != "0")
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(render_error(
            text,
            "array pointer index is not canonical decimal",
        ));
    }
    text.parse::<usize>()
        .ok()
        .filter(|index| *index < length)
        .ok_or_else(|| render_error(text, "array pointer does not name an existing element"))
}

fn locate<'a>(mut value: &'a Value, path: &[String]) -> Result<&'a Value, AssetError> {
    for part in path {
        value = match value {
            Value::Object(fields) => fields
                .get(part)
                .ok_or_else(|| render_error(part, "JSON pointer does not name an existing key"))?,
            Value::Array(items) => &items[array_index(part, items.len())?],
            _ => return Err(render_error(part, "JSON pointer crosses a scalar value")),
        };
    }
    Ok(value)
}

fn locate_mut<'a>(mut value: &'a mut Value, path: &[String]) -> Result<&'a mut Value, AssetError> {
    for part in path {
        value = match value {
            Value::Object(fields) => fields
                .get_mut(part)
                .ok_or_else(|| render_error(part, "JSON pointer does not name an existing key"))?,
            Value::Array(items) => {
                let index = array_index(part, items.len())?;
                &mut items[index]
            }
            _ => return Err(render_error(part, "JSON pointer crosses a scalar value")),
        };
    }
    Ok(value)
}

fn render_error(subject: impl Into<String>, message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::Render, subject, message)
}
fn format_error(error: madb_protocol::FormatError) -> AssetError {
    AssetError::new(
        AssetErrorKind::DocumentValidation,
        error.path,
        error.message,
    )
}
