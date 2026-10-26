//! The existing extension-wire v1 envelope, not a protocol session or a grant
//! of execution authority. Identity, negotiation, ordering and file access are
//! checked by the host after this format boundary.

use crate::{FormatError, SchemaRegistry, Value, canonical_json, strict_json};

/// The complete JSON Lines frame budget, including its terminating LF.
pub const MAX_EXTENSION_FRAME_BYTES: usize = 1024 * 1024;
/// Maximum number of nested object/array containers in a control frame.
pub const MAX_EXTENSION_FRAME_DEPTH: usize = 32;

/// Immutable schema-checked envelope. The bundled schema owns the fields;
/// this wrapper deliberately does not maintain a second wire structure.
#[derive(Clone, Debug)]
pub struct ExtensionFrame {
    value: Value,
}

impl ExtensionFrame {
    /// Decode exactly one LF-terminated JSON line. The transport must bound
    /// accumulation before calling this function and terminate on failure.
    /// Escaped newlines in strings are JSON data; raw additional lines are not.
    pub fn decode_line(line: &[u8]) -> Result<Self, FormatError> {
        require_size(line.len())?;
        let body = line
            .strip_suffix(b"\n")
            .ok_or_else(|| invalid("", "extension frame must end with a line feed"))?;
        if body.contains(&b'\n') {
            return Err(invalid("", "expected exactly one extension frame line"));
        }
        let value = strict_json(body).map_err(protocol_error)?;
        require_depth(&value, 0)?;
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/extension-wire.schema.json"
        )))
        .map_err(|error| FormatError::new("FMT000", "", error.to_string()))?;
        SchemaRegistry::validate_bundled(&schema, body).map_err(protocol_error)?;
        Ok(Self { value })
    }

    /// Check an author-created envelope through the same schema and budgets as
    /// received bytes. No fields, versions or capabilities are inferred.
    pub fn from_value(value: Value) -> Result<Self, FormatError> {
        require_depth(&value, 0)?;
        let mut bytes = canonical_json(&value).map_err(protocol_error)?;
        bytes.push(b'\n');
        Self::decode_line(&bytes)
    }

    /// Encode one deterministic JSON line. This is a transport encoding, not
    /// an identity or semantic digest, and it does not prove session validity.
    pub fn encode_line(&self) -> Result<Vec<u8>, FormatError> {
        let mut bytes = canonical_json(&self.value).map_err(protocol_error)?;
        bytes.push(b'\n');
        require_size(bytes.len())?;
        Ok(bytes)
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

fn require_size(size: usize) -> Result<(), FormatError> {
    if size > MAX_EXTENSION_FRAME_BYTES {
        return Err(invalid("", "extension frame exceeds byte budget"));
    }
    Ok(())
}

fn require_depth(value: &Value, depth: usize) -> Result<(), FormatError> {
    let children: Box<dyn Iterator<Item = &Value> + '_> = match value {
        Value::Array(items) => Box::new(items.iter()),
        Value::Object(fields) => Box::new(fields.values()),
        _ => return Ok(()),
    };
    if depth >= MAX_EXTENSION_FRAME_DEPTH {
        return Err(invalid("", "extension frame exceeds nesting budget"));
    }
    for child in children {
        require_depth(child, depth + 1)?;
    }
    Ok(())
}

fn invalid(path: impl Into<String>, message: impl Into<String>) -> FormatError {
    FormatError::new("ProtocolInvalid", path, message)
}

fn protocol_error(error: FormatError) -> FormatError {
    if error.code == "FMT000" {
        // A broken bundled schema is an implementation error, not peer input.
        error
    } else {
        invalid(error.path, format!("{}: {}", error.code, error.message))
    }
}
