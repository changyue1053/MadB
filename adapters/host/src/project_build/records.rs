//! Closed Host-private records. They are not public protocol decoders.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Prepare the closed Host-private provenance record for the checked 0.2.0
/// initialization plan. This function performs no IO and grants no permission
/// to write an existing project; initialization owns its no-replace commit.
pub fn origin_bytes(descriptor: &Descriptor, original: &[u8]) -> Result<Vec<u8>> {
    check_descriptor_paths(descriptor)?;
    encode(
        &json!({"kind":"madb-project-origin","version":1,"recipe":RECIPE,
        "descriptor":file_value(&FileIdentity{path:logical(DESCRIPTOR)?,sha256:Digest::of_bytes(original),size:original.len() as u64}),
        "template_archive":file_value(&descriptor.template.archive),
        "managed_roots":[".madb/wrapper",".madb/build/attempts",".madb/project-state"]}),
    )
}
pub(super) fn check_descriptor_paths(d: &Descriptor) -> Result<()> {
    if d.template.id.as_str() != "org.madb.template/rust-c-sdk"
        || d.template.version != "0.2.0"
        || d.template.archive.path.as_str() != ".madb/template-package.tar"
        || d.profile.file.path.as_str() != ".madb/template-input/profile.json"
        || d.sdk.file.path.as_str() != ".madb/template-input/toolchain.json"
    {
        return Err(invalid(
            "descriptor",
            "descriptor is outside the approved finite recipe",
        ));
    }
    Ok(())
}
pub(super) fn encode(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| host("record", e))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(invalid("record", "private record exceeds byte budget"));
    }
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(invalid("record", "private record exceeds byte budget"));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|e| conflict("record", e))?;
    if encode(&value)? != bytes {
        return Err(conflict(
            "record",
            "noncanonical or duplicate private record members",
        ));
    }
    Ok(value)
}
pub(super) fn keys(v: &Value, names: &[&str]) -> Result<()> {
    let actual = v
        .as_object()
        .ok_or_else(|| conflict("record", "expected object"))?;
    if actual.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.iter().copied().collect()
    {
        return Err(conflict(
            "record",
            "unknown or missing private record fields",
        ));
    }
    Ok(())
}
pub(super) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| conflict(key, "expected string"))
}
pub(super) fn digest(v: &Value, key: &str) -> Result<Digest> {
    Digest::new(text(v, key)?).map_err(|e| conflict(key, e))
}
pub(super) fn file_value(f: &FileIdentity) -> Value {
    json!({"path":f.path.as_str(),"sha256":f.sha256.as_str(),"size":f.size})
}
pub(super) fn file(v: &Value) -> Result<FileIdentity> {
    keys(v, &["path", "sha256", "size"])?;
    let size = v["size"]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| conflict("size", "expected safe unsigned integer"))?;
    Ok(FileIdentity {
        path: logical(text(v, "path")?)?,
        sha256: digest(v, "sha256")?,
        size,
    })
}
pub(super) fn projection_value(p: &OutputProjection) -> Value {
    json!({"snapshot_digest":p.snapshot_digest.as_str(),"candidate":{"root":p.candidate.root.as_str(),"manifest":file_value(&p.candidate.manifest)},
        "consumer":file_value(&p.consumer),"outputs":p.outputs.iter().map(file_value).collect::<Vec<_>>()})
}
pub(super) fn output_bytes(output: &ProjectOutput) -> Result<Vec<u8>> {
    encode(
        &json!({"kind":"madb-project-output","version":1,"identity":output.identity.as_str(),"projection":projection_value(&output.projection)}),
    )
}
pub(super) fn output(bytes: &[u8]) -> Result<ProjectOutput> {
    let v = decode(bytes)?;
    keys(&v, &["kind", "version", "identity", "projection"])?;
    if v["kind"] != "madb-project-output" || v["version"] != 1 {
        return Err(conflict("output", "unknown output record"));
    }
    let p = &v["projection"];
    keys(p, &["snapshot_digest", "candidate", "consumer", "outputs"])?;
    let c = &p["candidate"];
    keys(c, &["root", "manifest"])?;
    let files = p["outputs"]
        .as_array()
        .filter(|a| a.len() <= 16384)
        .ok_or_else(|| conflict("outputs", "invalid inventory"))?;
    Ok(ProjectOutput {
        identity: digest(&v, "identity")?,
        projection: OutputProjection {
            snapshot_digest: digest(p, "snapshot_digest")?,
            candidate: CandidateOutput {
                root: logical(text(c, "root")?)?,
                manifest: file(&c["manifest"])?,
            },
            consumer: file(&p["consumer"])?,
            outputs: files.iter().map(file).collect::<Result<Vec<_>>>()?,
        },
    })
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Index {
    pub generation: u64,
    pub attempt: String,
    pub record: FileIdentity,
    pub identity: Digest,
}
pub(super) fn index_value(i: &Index) -> Value {
    json!({"kind":"madb-project-current","version":1,"generation":i.generation,"attempt":i.attempt,"record":file_value(&i.record),"identity":i.identity.as_str()})
}
pub(super) fn index(bytes: &[u8]) -> Result<Index> {
    let v = decode(bytes)?;
    keys(
        &v,
        &[
            "kind",
            "version",
            "generation",
            "attempt",
            "record",
            "identity",
        ],
    )?;
    if v["kind"] != "madb-project-current" || v["version"] != 1 {
        return Err(conflict("current", "unknown current record"));
    }
    let generation = v["generation"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
        .ok_or_else(|| conflict("generation", "invalid generation"))?;
    let attempt = text(&v, "attempt")?.to_owned();
    if attempt.is_empty()
        || !attempt
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
    {
        return Err(conflict("attempt", "invalid owned attempt name"));
    }
    let record = file(&v["record"])?;
    if record.path.as_str() != "project-output.json" {
        return Err(conflict(
            "record",
            "invalid immutable output record location",
        ));
    }
    Ok(Index {
        generation,
        attempt,
        record,
        identity: digest(&v, "identity")?,
    })
}
