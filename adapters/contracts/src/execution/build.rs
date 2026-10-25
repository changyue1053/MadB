//! Validate selected build inputs and assemble their accepted evidence and outputs.
use super::{
    files::{check_bytes, references, validate_output_set},
    id, invalid,
};
use crate::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::validate_port_pair,
    query::{SnapshotAdapter, format_error},
};
use madb_app::{
    Digest, Id, LogicalPath, Result,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
    task::ValidatedTaskResult,
};
use madb_protocol::{DigestDomain, semantic_digest};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// Prove build recipes consume the exact committed generation and matching file identities.
pub fn validate_build_generation(
    tasks: &[Vec<u8>],
    output_set: &[u8],
    provided: &FrozenInputs,
) -> Result<()> {
    let adapter = SnapshotAdapter::new()?;
    let outputs = adapter.decode(DocumentKind::OutputSet, output_set)?;
    validate_output_set(&outputs, provided)?;
    let generated = references(&outputs.value()["files"])?;
    for bytes in tasks {
        let task = adapter.decode(DocumentKind::BuildTask, bytes)?;
        let v = task.value();
        if v["generation_id"] != outputs.value()["generation_id"]
            || v["input_digest"] != outputs.value()["input_digest"]
            || v["selector"]["profile_digest"] != outputs.value()["profile_digest"]
        {
            return Err(invalid(
                "EXECUTION002",
                "generation_id",
                "build task belongs to another committed generation",
            ));
        }
        for input in references(&v["inputs"])? {
            if generated
                .iter()
                .any(|file| file.path == input.path && file != &input)
            {
                return Err(invalid(
                    "FILE004",
                    input.path.as_str(),
                    "build input differs from committed generated file",
                ));
            }
        }
    }
    Ok(())
}

pub struct BuildArtifacts {
    pub identity: Digest,
    pub files: FrozenInputs,
    pub evidence: Vec<FileReference>,
}
/// Preserve accepted evidence bytes alongside verified outputs in deterministic task order.
pub fn assemble_build_files(
    tasks: &[Vec<u8>],
    results: &BTreeMap<Id, ValidatedTaskResult>,
    profile: &[u8],
) -> Result<BuildArtifacts> {
    let adapter = SnapshotAdapter::new()?;
    let profile = adapter.decode(DocumentKind::Profile, profile)?;
    let policy = crate::build_completion_policy(&profile).map_err(format_error)?;
    let profile_digest = crate::profile_digest(&profile).map_err(format_error)?;
    let mut documents = BTreeMap::new();
    for bytes in tasks {
        let task = adapter.decode(DocumentKind::BuildTask, bytes)?;
        if task.value()["selector"]["profile_digest"].as_str() != Some(profile_digest.as_str()) {
            return Err(invalid(
                "EXECUTION002",
                "profile_digest",
                "build task differs from completion policy profile",
            ));
        }
        if documents.insert(id(&task.value()["id"])?, task).is_some() {
            return Err(invalid("EXECUTION002", "tasks", "duplicate build task"));
        }
    }
    if documents.is_empty()
        || documents.len() != results.len()
        || documents.keys().ne(results.keys())
    {
        return Err(invalid(
            "EXECUTION002",
            "results",
            "build results do not cover the exact selected task set",
        ));
    }
    let phases = documents
        .values()
        .map(|task| super::build_phase(&task.value()["phase"]))
        .collect::<Result<Vec<_>>>()?;
    if !policy.is_complete(phases) {
        return Err(invalid(
            "EXECUTION003",
            "phase",
            "committed build lacks the selected policy's required phases or includes a forbidden phase",
        ));
    }
    let identity = Digest::new(semantic_digest(DigestDomain::Task, &json!({"operation":"build-set","tasks":documents.values().map(Document::value).collect::<Vec<_>>()})).map_err(format_error)?)
        .map_err(|e| invalid("HASH004", "build-set", e.to_string()))?;
    let mut workspace = MemoryFiles(BTreeMap::new());
    let mut files = Vec::new();
    let mut evidence = Vec::new();
    for (index, (id, task)) in documents.iter().enumerate() {
        let result = &results[id];
        let document = adapter.decode(DocumentKind::BuildEvidence, result.payload())?;
        validate_port_pair(task, &document).map_err(format_error)?;
        if document.value()["status"] != "passed" || document.value()["exit_code"] != 0 {
            return Err(invalid(
                "PORT004",
                "build_evidence",
                "build result did not pass",
            ));
        }
        for (file, bytes) in result.outputs() {
            workspace.insert(file, bytes)?;
            files.push(file.clone());
        }
        let file = FileReference {
            path: LogicalPath::new(format!("evidence/task-{index:04}.json"))
                .map_err(|e| invalid("FILE001", "evidence", e.to_string()))?,
            digest: Digest::of_bytes(result.payload()),
            size: result.payload().len() as u64,
        };
        workspace.insert(&file, result.payload())?;
        evidence.push(file.clone());
        files.push(file);
    }
    Ok(BuildArtifacts {
        identity,
        files: capture_inputs(&workspace, files)?,
        evidence,
    })
}
struct MemoryFiles(BTreeMap<LogicalPath, Vec<u8>>);
impl MemoryFiles {
    fn insert(&mut self, file: &FileReference, bytes: &[u8]) -> Result<()> {
        check_bytes(file, bytes)?;
        if self.0.insert(file.path.clone(), bytes.to_vec()).is_some() {
            return Err(invalid(
                "FILE001",
                file.path.as_str(),
                "duplicate build artifact path",
            ));
        }
        Ok(())
    }
}
impl ReadWorkspace for MemoryFiles {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .get(path)
            .map(|b| b[..b.len().min(limit.saturating_add(1))].to_vec()))
    }
}

/// Retain exact request bytes for the selected graph; reject missing or repeated IDs.
pub fn selected_task_documents(tasks: &[Vec<u8>], selected: &BTreeSet<Id>) -> Result<Vec<Vec<u8>>> {
    let adapter = SnapshotAdapter::new()?;
    let mut documents = BTreeMap::new();
    for bytes in tasks {
        let task = adapter.decode(DocumentKind::BuildTask, bytes)?;
        if documents.insert(id(&task.value()["id"])?, bytes).is_some() {
            return Err(invalid("EXECUTION002", "tasks", "duplicate build task"));
        }
    }
    selected
        .iter()
        .map(|id| {
            documents
                .get(id)
                .map(|bytes| (*bytes).clone())
                .ok_or_else(|| invalid("EXECUTION002", "tasks", "selected task is missing"))
        })
        .collect()
}
/// Extract environment names after strict BuildTask validation.
pub fn build_environment_names(tasks: &[Vec<u8>]) -> Result<BTreeSet<String>> {
    let adapter = SnapshotAdapter::new()?;
    let mut names = BTreeSet::new();
    for bytes in tasks {
        let task = adapter.decode(DocumentKind::BuildTask, bytes)?;
        for pair in task.value()["environment"]
            .as_array()
            .expect("schema checked")
        {
            names.insert(pair["name"].as_str().expect("schema checked").to_owned());
        }
    }
    Ok(names)
}

/// Gather the complete declared task inventory before creating managed output
/// directories. Duplicate references must agree even across unselected tasks.
pub fn build_input_references(tasks: &[Vec<u8>]) -> Result<Vec<FileReference>> {
    let registry = SchemaRegistry::new().map_err(format_error)?;
    let mut files = BTreeMap::new();
    for bytes in tasks {
        let task = registry
            .decode(DocumentKind::BuildTask, bytes)
            .map_err(format_error)?;
        for file in references(&task.value()["inputs"])? {
            if let Some(previous) = files.insert(file.path.clone(), file.clone())
                && previous != file
            {
                return Err(invalid(
                    "EXECUTION003",
                    file.path.as_str(),
                    "task input identities conflict",
                ));
            }
        }
    }
    Ok(files.into_values().collect())
}
