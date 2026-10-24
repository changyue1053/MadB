//! Validate package lineage against declared files and pinned parent batches.
use super::{
    artifact::validate_sources,
    digest,
    files::{
        declared_file, reference_schema, references, require_file, source_manifest,
        validate_output_set,
    },
    id, invalid,
    spec::ExecutionTool,
};
use crate::{
    Document, DocumentKind,
    query::{PreparedQuery, SnapshotAdapter, reference},
};
use madb_app::{Digest, Result, inputs::FrozenInputs, ports::FileReference};
use std::collections::BTreeSet;

pub(super) fn validate_package(
    adapter: &SnapshotAdapter,
    prepared: &PreparedQuery,
    request: &Document,
    provided: &FrozenInputs,
    input: &Digest,
    symbols: &BTreeSet<String>,
    materialized: &[FileReference],
) -> Result<()> {
    let q = request.value();
    reference_schema(&q["output_set"], DocumentKind::OutputSet)?;
    let output_ref = reference(&q["output_set"])?;
    declared_file(materialized, &output_ref)?;
    let outputs = adapter.decode(
        DocumentKind::OutputSet,
        require_file(provided, &output_ref)?,
    )?;
    for file in references(&outputs.value()["files"])? {
        declared_file(materialized, &file)?;
    }
    validate_output_set(&outputs, provided)?;
    validate_sources(&outputs, symbols)?;
    let v = outputs.value();
    if v["manifest_path"].as_str() != Some(output_ref.path.as_str()) {
        return Err(invalid(
            "EXECUTION002",
            "manifest_path",
            "package references a renamed source manifest",
        ));
    }
    if q["generation_id"] != v["generation_id"]
        || v["input_digest"].as_str() != Some(input.as_str())
        || v["profile_digest"] != q["selector"]["profile_digest"]
    {
        return Err(invalid(
            "EXECUTION002",
            "output_set",
            "package source generation or lineage differs",
        ));
    }
    let declared: BTreeSet<_> = q["public_symbols"]
        .as_array()
        .expect("validated")
        .iter()
        .map(|s| s.as_str().expect("validated").to_owned())
        .collect();
    if &declared != symbols {
        return Err(invalid(
            "EXECUTION002",
            "public_symbols",
            "package symbols differ from checked contract",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut phases = BTreeSet::new();
    for item in q["build_evidence"].as_array().expect("validated") {
        reference_schema(item, DocumentKind::BuildEvidence)?;
    }
    for file in references(&q["build_evidence"])? {
        declared_file(materialized, &file)?;
        let evidence =
            adapter.decode(DocumentKind::BuildEvidence, require_file(provided, &file)?)?;
        let e = evidence.value();
        phases.insert(e["phase"].as_str().expect("validated").to_owned());
        if !ids.insert(e["task_id"].as_str().expect("validated").to_owned())
            || e["status"] != "passed"
            || e["exit_code"] != 0
            || e["generation_id"] != v["generation_id"]
            || e["input_digest"] != v["input_digest"]
            || e["profile_digest"] != v["profile_digest"]
            || e["target"] != q["selector"]["target"]
        {
            return Err(invalid(
                "EXECUTION002",
                "build_evidence",
                "package evidence is unsuccessful, duplicated, or from another source generation",
            ));
        }
        let tool = ExecutionTool {
            id: id(&e["tool"]["id"])?,
            version: e["tool"]["version"].as_str().expect("validated").to_owned(),
            digest: digest(&e["tool"]["sha256"])?,
        };
        // Evidence carries tool identity, not a second executable path.
        let selected = prepared.profile.value()["tools"]
            .as_array()
            .expect("validated")
            .iter()
            .find(|item| {
                item["id"].as_str() == Some(tool.id.as_str())
                    && item["version"] == e["tool"]["version"]
                    && item["sha256"] == e["tool"]["sha256"]
            });
        if selected.is_none_or(|tool| {
            !prepared.lock.value()["tools"]
                .as_array()
                .expect("validated")
                .contains(tool)
        }) {
            return Err(invalid(
                "LOCK004",
                "build_evidence/tool",
                "evidence tool is not exactly locked",
            ));
        }
        for output in references(&e["outputs"])? {
            declared_file(materialized, &output)?;
            require_file(provided, &output)?;
        }
    }
    if ["compile", "link", "run"]
        .iter()
        .any(|phase| !phases.contains(*phase))
    {
        return Err(invalid(
            "EXECUTION003",
            "build_evidence",
            "package requires successful compile, link and run evidence",
        ));
    }
    for dependency in references(&q["dependencies"])? {
        declared_file(materialized, &dependency)?;
        require_file(provided, &dependency)?;
    }
    Ok(())
}

/// Package lineage comes exclusively from pinned parents. Workspace files with
/// plausible evidence content cannot fill gaps or replace the committed inventory.
pub fn validate_package_parents(
    request: &[u8],
    source: &FrozenInputs,
    build: &FrozenInputs,
) -> Result<()> {
    let adapter = SnapshotAdapter::new()?;
    let request = adapter.decode(DocumentKind::PackageRequest, request)?;
    let q = request.value();
    reference_schema(&q["output_set"], DocumentKind::OutputSet)?;
    let manifest = source_manifest(source)?;
    let output_ref = reference(&q["output_set"])?;
    if require_file(source, &output_ref)? != manifest {
        return Err(invalid(
            "EXECUTION002",
            "output_set",
            "package must use the pinned source manifest",
        ));
    }
    let mut expected = Vec::new();
    for (file, bytes) in build.files() {
        let path = file.path.as_str();
        let Some(index) = path
            .strip_prefix("evidence/task-")
            .and_then(|tail| tail.strip_suffix(".json"))
        else {
            continue;
        };
        if index.len() != 4 || !index.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid(
                "EXECUTION002",
                "build_evidence",
                "malformed pinned evidence path",
            ));
        }
        adapter.decode(DocumentKind::BuildEvidence, bytes)?;
        expected.push(file.clone());
    }
    if expected.is_empty() {
        return Err(invalid(
            "EXECUTION002",
            "build_evidence",
            "pinned build lacks its evidence inventory",
        ));
    }
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    for (index, file) in expected.iter().enumerate() {
        if file.path.as_str() != format!("evidence/task-{index:04}.json") {
            return Err(invalid(
                "EXECUTION002",
                "build_evidence",
                "pinned evidence inventory has a gap",
            ));
        }
    }
    let mut declared = references(&q["build_evidence"])?;
    for item in q["build_evidence"].as_array().expect("validated") {
        reference_schema(item, DocumentKind::BuildEvidence)?;
    }
    for file in &declared {
        require_file(build, file)?;
    }
    declared.sort_by(|a, b| a.path.cmp(&b.path));
    if declared != expected {
        return Err(invalid(
            "EXECUTION002",
            "build_evidence",
            "package must reference every pinned evidence file exactly once",
        ));
    }
    Ok(())
}
