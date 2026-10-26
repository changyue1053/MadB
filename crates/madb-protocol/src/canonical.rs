use crate::{
    FormatError,
    decode::{MAX_DOCUMENT_DEPTH, MAX_SAFE_INTEGER},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Domains prevent identities for unrelated objects from being interchangeable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestDomain {
    Contract,
    Policy,
    Shape,
    Profile,
    ProfileV2,
    Composition,
    Source,
    Task,
    Candidate,
    Generation,
    Case,
    Catalog,
    Project,
}
impl DigestDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contract => "madb.contract/1",
            Self::Policy => "madb.policy/1",
            Self::Shape => "madb.shape/1",
            Self::Profile => "madb.profile/1",
            Self::ProfileV2 => "madb.profile/2",
            Self::Composition => "madb.composition/1",
            Self::Source => "madb.source/1",
            Self::Task => "madb.task/1",
            Self::Candidate => "madb.candidate/1",
            Self::Generation => "madb.generation/1",
            Self::Case => "madb.case/1",
            Self::Catalog => "madb.catalog/1",
            Self::Project => "madb.project/1",
        }
    }
}

pub fn byte_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

/// RFC 8785 encoding for MadB's restricted numeric domain. Arrays retain order;
/// property keys sort by UTF-16, not Rust's UTF-8 ordering. No Unicode folding.
pub fn canonical_json(value: &Value) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Vec::new();
    write_value(value, &mut bytes, 0)?;
    Ok(bytes)
}

pub fn semantic_digest(domain: DigestDomain, projection: &Value) -> Result<String, FormatError> {
    let mut hash = Sha256::new();
    hash.update(domain.as_str().as_bytes());
    hash.update(b"\n");
    hash.update(canonical_json(projection)?);
    Ok(format!("sha256:{}", hex(&hash.finalize())))
}

fn write_value(value: &Value, out: &mut Vec<u8>, depth: usize) -> Result<(), FormatError> {
    if depth > MAX_DOCUMENT_DEPTH {
        return Err(FormatError::new("FMT002", "", "canonical depth exceeded"));
    }
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<_> = fields.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push(b'{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_string(key, out)?;
                out.push(b':');
                write_value(&fields[key], out, depth + 1)?;
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_value(item, out, depth + 1)?;
            }
            out.push(b']');
        }
        Value::String(value) => write_string(value, out)?,
        Value::Number(value) => {
            let integer = value
                .as_i64()
                .filter(|n| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(n))
                .ok_or_else(|| {
                    FormatError::new("FMT006", "", "canonical projection needs safe integers")
                })?;
            out.extend_from_slice(integer.to_string().as_bytes());
        }
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Null => out.extend_from_slice(b"null"),
    }
    Ok(())
}

fn write_string(value: &str, out: &mut Vec<u8>) -> Result<(), FormatError> {
    serde_json::to_writer(out, value).map_err(|e| FormatError::new("FMT001", "", e.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
