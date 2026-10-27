//! Explicit composition input checks. None of these values grants execution,
//! publication, or certification. The old profile/1 entry points stay separate.
mod outputs;
mod ownership;
mod pins;

// The fixed recipe selection exposes this domain type in its fields.
pub use madb_model::contract::Direction;

pub use outputs::{
    BoundGeneratedOutputs, BoundGeneratedSlot, GenerationBindingContext, GenerationSchemaPaths,
    GenerationStepObservation, GenerationStepRequirement, bind_generated_outputs,
};
pub use ownership::{
    AssetOrigin, ResolvedOwner, ResolvedResponsibility, ResolvedScope, ResolvedSubject,
    ResponsibilityKey, compare_responsibility_sets,
};
pub use pins::{
    BoundCompositionInputs, ComponentRequirement, CompositionInputContext, CompositionSchemaPaths,
    GeneratedSlot, RecipeRequirements, RecipeSelection, bind_composition_inputs,
    check_composition_snapshot_binding,
};

use crate::{Document, DocumentKind, FormatError, wire::Wire};
use madb_protocol::{DigestDomain, Value, canonical_json, semantic_digest};
use serde_json::json;
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, FormatError>;

/// Compute an input identity, not an assertion that its recipe or claims work.
pub fn composition_digest(document: &Document) -> Result<String> {
    let mut v = Wire::root(document, DocumentKind::Composition)?
        .value
        .clone();
    for field in ["packages", "components", "assets"] {
        sort_set(&mut v[field], |row| Ok(vec![text(&row["id"])?.into()]))?;
    }
    for package in v["packages"].as_array_mut().expect("checked packages") {
        sort_files(&mut package["source"]["files"])?;
    }
    for asset in v["assets"].as_array_mut().expect("checked assets") {
        if asset["source"]["kind"] == "source" {
            sort_files(&mut asset["source"]["files"])?;
        } else {
            sort_set(&mut asset["source"]["paths"], |path| {
                Ok(vec![text(path)?.into()])
            })?;
        }
    }
    sort_set(&mut v["dependencies"], |row| {
        ["from", "to", "scope"]
            .map(|key| text(&row[key]).map(str::to_owned))
            .into_iter()
            .collect()
    })?;
    sort_set(&mut v["responsibilities"], |row| {
        Ok(vec![
            text(&row["boundary"])?.into(),
            canonical_text(&row["subject"])?,
            text(&row["stage"])?.into(),
            canonical_text(&row["scope"])?,
        ])
    })?;
    semantic_digest(DigestDomain::Composition, &v)
}

/// The /2 domain is explicit. Byte references are checked separately from this
/// semantic projection, and this function never converts /2 into a /1 document.
pub fn profile_v2_digest(document: &Document) -> Result<String> {
    let root = Wire::root(document, DocumentKind::ProfileV2)?;
    let mut v = crate::projections::profile_projection(root.value)?;
    let composition = &root.value["composition"];
    v["composition"] =
        json!({"id":composition["id"], "semantic_digest":composition["semantic_digest"]});
    semantic_digest(DigestDomain::ProfileV2, &v)
}

fn sort_files(value: &mut Value) -> Result<()> {
    sort_set(value, |file| Ok(vec![text(&file["path"])?.into()]))
}
fn sort_set(value: &mut Value, key: impl Fn(&Value) -> Result<Vec<String>>) -> Result<()> {
    let mut keyed = Vec::new();
    let mut seen = BTreeSet::new();
    for row in value
        .as_array()
        .ok_or_else(|| invalid("HASH004", "set", "expected array"))?
    {
        let key = key(row)?
            .into_iter()
            .map(|s| s.encode_utf16().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if !seen.insert(key.clone()) {
            return Err(invalid("HASH004", "set", "duplicate collection identity"));
        }
        keyed.push((key, row.clone()));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    *value = Value::Array(keyed.into_iter().map(|(_, value)| value).collect());
    Ok(())
}
fn canonical_text(value: &Value) -> Result<String> {
    String::from_utf8(canonical_json(value)?)
        .map_err(|_| invalid("HASH004", "", "canonical JSON is not UTF-8"))
}
fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("DTO001", "", "expected string"))
}
fn invalid(code: &'static str, subject: &str, message: impl Into<String>) -> FormatError {
    FormatError::new(code, subject, message)
}

pub mod admission;
pub mod execution;
