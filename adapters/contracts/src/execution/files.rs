//! Validate supplied file identities and the exact committed source inventory.
use super::invalid;
use crate::{
    Document, DocumentKind,
    query::{SnapshotAdapter, format_error, reference},
};
use madb_app::{Digest, Result, inputs::FrozenInputs, ports::FileReference};
use madb_protocol::Value;
use serde_json::json;
use std::collections::BTreeSet;

pub(crate) fn check_bytes(file: &FileReference, bytes: &[u8]) -> Result<()> {
    if file.size != bytes.len() as u64 || file.digest != Digest::of_bytes(bytes) {
        return Err(invalid(
            "FILE004",
            file.path.as_str(),
            "file reference differs from supplied bytes",
        ));
    }
    Ok(())
}
pub(crate) fn require_file<'a>(
    provided: &'a FrozenInputs,
    expected: &FileReference,
) -> Result<&'a [u8]> {
    let (file, bytes) = provided
        .files()
        .find(|(f, _)| f.path == expected.path)
        .ok_or_else(|| {
            invalid(
                "FILE003",
                expected.path.as_str(),
                "required frozen file is missing",
            )
        })?;
    if file != expected {
        return Err(invalid(
            "FILE004",
            expected.path.as_str(),
            "frozen file reference differs",
        ));
    }
    Ok(bytes)
}
pub(crate) fn file_value(file: &FileReference) -> Value {
    json!({"path":file.path.as_str(),"sha256":file.digest.as_str(),"size":file.size})
}
pub(crate) fn references(value: &Value) -> Result<Vec<FileReference>> {
    let files = value
        .as_array()
        .ok_or_else(|| invalid("DTO001", "files", "expected file array"))?
        .iter()
        .map(reference)
        .collect::<Result<Vec<_>>>()?;
    let mut paths = BTreeSet::new();
    for file in &files {
        if !paths.insert(&file.path) {
            return Err(invalid(
                "FILE001",
                file.path.as_str(),
                "duplicate file reference",
            ));
        }
    }
    Ok(files)
}

pub(crate) fn reference_schema(value: &Value, kind: DocumentKind) -> Result<()> {
    if value["schema"].as_str() != Some(kind.schema_id().as_str()) {
        return Err(invalid(
            "EXECUTION001",
            "schema",
            "file reference has the wrong schema identity",
        ));
    }
    Ok(())
}

pub(crate) fn declared_file(files: &[FileReference], expected: &FileReference) -> Result<()> {
    if !files.contains(expected) {
        return Err(invalid(
            "FILE003",
            expected.path.as_str(),
            "required file is not declared for process materialization",
        ));
    }
    Ok(())
}

pub(crate) fn validate_output_set(output_set: &Document, provided: &FrozenInputs) -> Result<()> {
    let v = output_set.value();
    if v["generation_id"].as_str()
        != Some(
            crate::generation_id(output_set)
                .map_err(format_error)?
                .as_str(),
        )
    {
        return Err(invalid(
            "FILE006",
            "generation_id",
            "output identity differs",
        ));
    }
    for file in references(&v["files"])? {
        require_file(provided, &file)?;
    }
    Ok(())
}

/// A source batch must contain exactly one valid OutputSet at its declared
/// manifest path, whose complete inventory matches the captured generation.
pub fn source_manifest(files: &FrozenInputs) -> Result<&[u8]> {
    let adapter = SnapshotAdapter::new()?;
    let mut found = None;
    for (file, bytes) in files.files() {
        let Ok(value) = madb_protocol::strict_json(bytes) else {
            continue;
        };
        if value["schema"] != "madb.output-set/1" {
            continue;
        }
        let document = adapter.decode(DocumentKind::OutputSet, bytes)?;
        if document.value()["manifest_path"].as_str() != Some(file.path.as_str()) {
            return Err(invalid(
                "EXECUTION002",
                "manifest_path",
                "source manifest path differs from captured file",
            ));
        }
        validate_output_set(&document, files)?;
        let mut expected = references(&document.value()["files"])?;
        expected.push(file.clone());
        expected.sort_by(|a, b| a.path.cmp(&b.path));
        if expected.iter().ne(files.references()) {
            return Err(invalid(
                "EXECUTION002",
                "files",
                "source batch contains undeclared files",
            ));
        }
        if found.replace(bytes).is_some() {
            return Err(invalid(
                "EXECUTION002",
                "manifest",
                "ambiguous source batch manifest",
            ));
        }
    }
    found.ok_or_else(|| {
        invalid(
            "EXECUTION002",
            "manifest",
            "source batch lacks its OutputSet",
        )
    })
}
