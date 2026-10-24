//! Private, versioned store records. These are not public SDK wire payloads.
use super::{fail, io_error};
use crate::LocalWorkspace;
use madb_app::{
    Digest, LogicalPath, Result,
    ports::{FileReference, ReadWorkspace},
    transaction::{BatchInfo, BatchSpec, IndexSnapshot, Layer},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs::File};
const RECORD_LIMIT: usize = 8 * 1024 * 1024;
fn digest(value: &Value) -> Result<Digest> {
    Digest::new(
        value
            .as_str()
            .ok_or_else(|| fail("STORE004", "digest", "expected digest string"))?,
    )
    .map_err(|e| fail("STORE004", "digest", e.to_string()))
}
fn optional(value: &Value) -> Result<Option<Digest>> {
    if value.is_null() {
        Ok(None)
    } else {
        digest(value).map(Some)
    }
}
fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let map = value
        .as_object()
        .ok_or_else(|| fail("STORE004", "record", "expected object"))?;
    if map.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected.iter().copied().collect()
    {
        return Err(fail(
            "STORE004",
            "record",
            "unexpected or missing record fields",
        ));
    }
    Ok(())
}
pub(super) fn read(root: &File, name: &str) -> Result<Option<Value>> {
    let workspace = LocalWorkspace {
        root: root.try_clone().map_err(|e| io_error(name, e))?,
    };
    let bytes = workspace
        .read(
            &LogicalPath::new(name).map_err(|e| fail("STORE004", name, e.to_string()))?,
            RECORD_LIMIT,
        )
        .map_err(|e| fail("STORE004", name, e.to_string()))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| fail("STORE004", name, e.to_string()))?;
    if encode(&value)? != bytes {
        return Err(fail(
            "STORE004",
            name,
            "store record is not canonical or contains duplicate keys",
        ));
    }
    Ok(Some(value))
}
pub(super) fn encode(value: &Value) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|e| fail("STORE004", "record", e.to_string()))?;
    if bytes.len() > RECORD_LIMIT {
        return Err(fail("STORE007", "record", "store metadata exceeds 8 MiB"));
    }
    Ok(bytes)
}
pub(super) fn index_value(index: &IndexSnapshot) -> Value {
    json!({"schema":"madb.storage-index/1","revision":index.revision,"source":index.source.as_ref().map(Digest::as_str),"build":index.build.as_ref().map(Digest::as_str),"candidate":index.candidate.as_ref().map(Digest::as_str)})
}
pub(super) fn index(root: &File) -> Result<IndexSnapshot> {
    let Some(value) = read(root, "current.json")? else {
        return Ok(IndexSnapshot::default());
    };
    keys(
        &value,
        &["schema", "revision", "source", "build", "candidate"],
    )?;
    if value["schema"] != "madb.storage-index/1" {
        return Err(fail("STORE004", "index", "unsupported store index version"));
    }
    let index = IndexSnapshot {
        revision: value["revision"]
            .as_u64()
            .filter(|r| *r > 0)
            .ok_or_else(|| fail("STORE004", "revision", "invalid store revision"))?,
        source: optional(&value["source"])?,
        build: optional(&value["build"])?,
        candidate: optional(&value["candidate"])?,
    };
    if index.source.is_none() || (index.candidate.is_some() && index.build.is_none()) {
        return Err(fail("STORE004", "index", "incomplete layer ancestry"));
    }
    Ok(index)
}
pub(super) fn metadata(info: &BatchInfo) -> Value {
    let spec = &info.spec;
    json!({"schema":"madb.storage-batch/1","id":info.id.as_str(),"content_id":info.content_id.as_str(),
        "spec":{"layer":spec.layer.as_str(),"input_digest":spec.input_digest.as_str(),"task_digest":spec.task_digest.as_str(),"profile_digest":spec.profile_digest.as_str(),"generation_id":spec.generation_id.as_str(),"parent":spec.parent.as_ref().map(Digest::as_str)},
        "files":info.files.iter().map(|f| json!({"path":f.path.as_str(),"sha256":f.digest.as_str(),"size":f.size})).collect::<Vec<_>>()})
}
pub(super) fn info(root: &File) -> Result<BatchInfo> {
    let value = read(root, "metadata.json")?
        .ok_or_else(|| fail("STORE004", "metadata", "batch metadata missing"))?;
    keys(&value, &["schema", "id", "content_id", "spec", "files"])?;
    if value["schema"] != "madb.storage-batch/1" {
        return Err(fail("STORE004", "metadata", "unsupported batch version"));
    }
    let spec = &value["spec"];
    keys(
        spec,
        &[
            "layer",
            "input_digest",
            "task_digest",
            "profile_digest",
            "generation_id",
            "parent",
        ],
    )?;
    let layer = match spec["layer"].as_str() {
        Some("source") => Layer::Source,
        Some("build") => Layer::Build,
        Some("candidate") => Layer::Candidate,
        _ => return Err(fail("STORE004", "layer", "invalid batch layer")),
    };
    let values = value["files"]
        .as_array()
        .filter(|files| !files.is_empty() && files.len() <= 4096)
        .ok_or_else(|| fail("STORE004", "files", "invalid batch file inventory"))?;
    let mut files = Vec::with_capacity(values.len());
    for file in values {
        keys(file, &["path", "sha256", "size"])?;
        files.push(FileReference {
            path: LogicalPath::new(
                file["path"]
                    .as_str()
                    .ok_or_else(|| fail("STORE004", "path", "invalid path"))?,
            )
            .map_err(|e| fail("STORE004", "path", e.to_string()))?,
            digest: digest(&file["sha256"])?,
            size: file["size"]
                .as_u64()
                .ok_or_else(|| fail("STORE004", "size", "invalid size"))?,
        });
    }
    Ok(BatchInfo {
        id: digest(&value["id"])?,
        content_id: digest(&value["content_id"])?,
        files,
        spec: BatchSpec {
            layer,
            input_digest: digest(&spec["input_digest"])?,
            task_digest: digest(&spec["task_digest"])?,
            profile_digest: digest(&spec["profile_digest"])?,
            generation_id: digest(&spec["generation_id"])?,
            parent: optional(&spec["parent"])?,
        },
    })
}
