use crate::FormatError;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
pub const MAX_DOCUMENT_DEPTH: usize = 64;
pub(crate) const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Reject duplicate keys before they can disappear into a map. Decimal/float
/// ABI constants use typed strings; ordinary JSON numbers are safe integers.
pub fn strict_json(bytes: &[u8]) -> Result<Value, FormatError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(FormatError::new(
            "FMT002",
            "",
            "document exceeds byte budget",
        ));
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = Seed { depth: 0 }
        .deserialize(&mut decoder)
        .map_err(parse_error)?;
    decoder.end().map_err(parse_error)?;
    Ok(value)
}

fn parse_error(error: serde_json::Error) -> FormatError {
    let message = error.to_string();
    let code = ["FMT002", "FMT005", "FMT006"]
        .into_iter()
        .find(|code| message.contains(code))
        .unwrap_or("FMT001");
    FormatError::new(
        code,
        format!("line {} column {}", error.line(), error.column()),
        message,
    )
}

struct Seed {
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        if self.depth > MAX_DOCUMENT_DEPTH {
            return Err(de::Error::custom("FMT002: nesting budget exceeded"));
        }
        decoder.deserialize_any(StrictVisitor { depth: self.depth })
    }
}

struct StrictVisitor {
    depth: usize,
}
impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded I-JSON with safe integers")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        if !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&v) {
            return Err(E::custom("FMT006: integer requires a typed decimal string"));
        }
        Ok(Value::Number(Number::from(v)))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        if v > MAX_SAFE_INTEGER as u64 {
            return Err(E::custom("FMT006: integer exceeds safe range"));
        }
        Ok(Value::Number(Number::from(v)))
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Value, E> {
        // Never accept a decimal that rounded to an integer during parsing.
        // JSON numbers in this protocol use integral spelling only.
        Err(E::custom(
            "FMT006: use integral JSON spelling or a typed IEEE bit string",
        ))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Seed {
            depth: self.depth + 1,
        })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut items = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if items.contains_key(&key) {
                return Err(de::Error::custom(format!("FMT005: duplicate key {key:?}")));
            }
            let value = map.next_value_seed(Seed {
                depth: self.depth + 1,
            })?;
            items.insert(key, value);
        }
        Ok(Value::Object(items))
    }
}
