use super::{ProjectBuildAdapter, encode, format_error, invalid};
use madb_app::use_cases::project_build::*;
use madb_app::{LogicalPath, Result};
use std::collections::{BTreeMap, BTreeSet};

const REQUIRED: &[&str] = &[
    DESCRIPTOR,
    ORIGIN,
    ".madb/template-package.tar",
    ".madb/template-input/profile.json",
    ".madb/template-input/toolchain.json",
    "consumer/main.c",
    "rust/src/lib.rs",
];

pub(super) fn descriptor(v: &Descriptor) -> Result<()> {
    madb_plan::project::validate_identifier(&v.namespace)
        .map_err(|error| invalid("namespace", error.message))?;
    if v.template.id.as_str() != "org.madb.template/rust-c-sdk"
        || v.template.version != "0.2.0"
        || v.template.archive.path.as_str() != ".madb/template-package.tar"
        || v.profile.file.path.as_str() != ".madb/template-input/profile.json"
        || v.profile.schema.as_str() != "madb.profile/1"
        || v.sdk.file.path.as_str() != ".madb/template-input/toolchain.json"
        || v.sdk.schema.as_str() != "madb.native-toolchain/1"
    {
        return Err(invalid(
            "descriptor",
            "descriptor is outside the approved finite template/profile/SDK locations",
        ));
    }
    check_tool(&v.tool)
}
fn check_tool(v: &ToolIdentity) -> Result<()> {
    if v.id.as_str() != "org.madb/cli"
        || v.version != "0.1.0"
        || v.size == 0
        || v.size > 256 * 1024 * 1024
        || v.source_revision.len() != 40
        || !v
            .source_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid(
            "tool",
            "a fixed real 0.1.0 source-bound tool image is required",
        ));
    }
    Ok(())
}
fn section(value: &serde_json::Value, name: &str) -> Result<()> {
    if madb_protocol::canonical_json(value)
        .map_err(format_error)?
        .len()
        > MAX_SECTION_BYTES
    {
        return Err(invalid(
            name,
            "project section exceeds the adopted 128 KiB metadata budget",
        ));
    }
    Ok(())
}
pub(super) fn inputs(v: &InputInventory, allow_empty: bool) -> Result<()> {
    if v.files.is_empty() && v.directories.is_empty() && allow_empty {
        return Ok(());
    }
    if v.files.len() > 4096 || v.directories.len() != 2 {
        return Err(invalid(
            "inputs",
            "incomplete or excessive source inventory",
        ));
    }
    let mut sources = BTreeMap::new();
    let mut total = 0u64;
    for f in &v.files {
        if !REQUIRED.contains(&f.source.as_str())
            && !f.source.as_str().starts_with("consumer/include/")
        {
            return Err(invalid(
                f.source.as_str(),
                "unknown source location in the finite recipe",
            ));
        }
        if f.file.path.as_str() != format!("inputs/{}", f.source.as_str())
            || sources.insert(f.source.as_str(), f).is_some()
        {
            return Err(invalid(
                f.source.as_str(),
                "snapshot path differs or source appears twice",
            ));
        }
        if f.file.size > 32 * 1024 * 1024 {
            return Err(invalid(
                f.source.as_str(),
                "source file exceeds finite byte budget",
            ));
        }
        total = total
            .checked_add(f.file.size)
            .ok_or_else(|| invalid("inputs", "source byte count overflow"))?;
    }
    if total > 128 * 1024 * 1024 || REQUIRED.iter().any(|p| !sources.contains_key(p)) {
        return Err(invalid(
            "inputs",
            "missing required sources or total source budget exceeded",
        ));
    }
    let mut directories = BTreeSet::new();
    let mut observed_members = BTreeSet::new();
    let mut count = 0;
    for d in &v.directories {
        if !["consumer/include", "rust/src"].contains(&d.path.as_str())
            || !directories.insert(d.path.as_str())
        {
            return Err(invalid(
                "directories",
                "unknown or repeated source directory",
            ));
        }
        let mut members = BTreeMap::new();
        for m in &d.members {
            count += 1;
            if !d.present
                || m.path.as_str().split('/').count() > 64
                || members.insert(m.path.as_str(), m.kind).is_some()
            {
                return Err(invalid(
                    d.path.as_str(),
                    "absent, duplicate or too-deep directory member",
                ));
            }
            let source = format!("{}/{}", d.path.as_str(), m.path.as_str());
            if m.kind == MemberKind::File
                && (!sources.contains_key(source.as_str()) || !observed_members.insert(source))
            {
                return Err(invalid(
                    m.path.as_str(),
                    "member and captured file inventories differ",
                ));
            }
        }
        for name in members.keys() {
            let mut parent = *name;
            while let Some((next, _)) = parent.rsplit_once('/') {
                if members.get(next) != Some(&MemberKind::Directory) {
                    return Err(invalid(*name, "directory inventory omits an ancestor"));
                }
                parent = next;
            }
        }
        if d.path.as_str() == "rust/src"
            && (!d.present
                || members.len() != 1
                || members.get("lib.rs") != Some(&MemberKind::File))
        {
            return Err(invalid(
                "rust/src",
                "only the single natural lib.rs source is supported",
            ));
        }
    }
    if count > 16384
        || sources
            .keys()
            .filter(|p| p.starts_with("consumer/include/") || **p == "rust/src/lib.rs")
            .any(|p| !observed_members.contains(*p))
    {
        return Err(invalid(
            "directories",
            "directory closure is incomplete or exceeds budget",
        ));
    }
    section(&encode::inventory(v), "inputs")
}
pub(super) fn outputs(v: &OutputProjection) -> Result<()> {
    if v.outputs.len() < 3 || v.outputs.len() > 16384 {
        return Err(invalid(
            "outputs",
            "incomplete or excessive project outputs",
        ));
    }
    let prefix = format!("{}/", v.candidate.root.as_str());
    if v.candidate.manifest.path.as_str() != format!("{prefix}candidate.json")
        || v.consumer.path.as_str().starts_with(&prefix)
    {
        return Err(invalid(
            "candidate",
            "candidate manifest or consumer is outside its fixed ownership",
        ));
    }
    let mut paths = BTreeMap::new();
    for f in &v.outputs {
        if paths.insert(f.path.as_str(), f).is_some()
            || (!f.path.as_str().starts_with(&prefix) && f.path != v.consumer.path)
        {
            return Err(invalid(f.path.as_str(), "duplicate or extraneous output"));
        }
    }
    if paths.get(v.consumer.path.as_str()) != Some(&&v.consumer)
        || paths.get(v.candidate.manifest.path.as_str()) != Some(&&v.candidate.manifest)
    {
        return Err(invalid(
            "outputs",
            "manifest/consumer identity differs from complete output inventory",
        ));
    }
    section(
        &serde_json::json!(v.outputs.iter().map(encode::file).collect::<Vec<_>>()),
        "outputs",
    )
    .map_err(|error| {
        madb_app::QueryError::new(
            madb_app::ErrorKind::CheckFailed,
            "EVD001",
            "outputs",
            error.to_string(),
        )
    })
}
fn descriptor_reference(v: &InputInventory) -> Result<FileIdentity> {
    let f = v
        .files
        .iter()
        .find(|f| f.source.as_str() == DESCRIPTOR)
        .ok_or_else(|| invalid("inputs", "missing descriptor"))?;
    Ok(FileIdentity {
        path: LogicalPath::new(DESCRIPTOR).expect("fixed descriptor"),
        sha256: f.file.sha256.clone(),
        size: f.file.size,
    })
}
pub(super) fn report(format: &ProjectBuildAdapter, v: &ProjectReport) -> Result<()> {
    inputs(&v.inputs, true)?;
    if let Some(t) = &v.tool {
        check_tool(t)?;
    }
    if !v.stages.is_empty() {
        if v.stages.iter().map(|s| s.id).collect::<Vec<_>>() != v.operation.stages() {
            return Err(invalid(
                "stages",
                "fixed complete stage sequence is required",
            ));
        }
        for (position, stage) in v.stages.iter().enumerate() {
            let mut seen = BTreeSet::new();
            for blocked in &stage.blocked_by {
                if !v.operation.stages()[..position].contains(blocked)
                    || !seen.insert(blocked.as_str())
                {
                    return Err(invalid(
                        "blocked_by",
                        "blocking reference must name a unique earlier stage",
                    ));
                }
            }
            if (stage.status == StageStatus::Blocked) != (!stage.blocked_by.is_empty()) {
                return Err(invalid("blocked_by", "blocking facts and status disagree"));
            }
        }
    }
    if !v.inputs.files.is_empty() {
        if v.artifact_root.is_none() {
            return Err(invalid(
                "artifact_root",
                "captured copies need their actual attempt root",
            ));
        }
        if let (Some(root), Some(request)) = (&v.root, &v.request_digest)
            && *request
                != format.request_digest(&RequestIdentity {
                    operation: v.operation,
                    root: root.clone(),
                    descriptor: descriptor_reference(&v.inputs)?,
                })?
        {
            return Err(invalid(
                "request_digest",
                "request projection differs from captured original descriptor",
            ));
        }
    }
    if let Some(result) = &v.result {
        if v.published != Some(true) {
            return Err(invalid(
                "result",
                "only an actually published output may be returned as result",
            ));
        }
        let tool = v
            .tool
            .as_ref()
            .ok_or_else(|| invalid("tool", "published result lacks actual tool identity"))?;
        if result.projection.snapshot_digest
            != format.input_digest(&descriptor_reference(&v.inputs)?, &v.inputs)?
            || result.identity != format.output_digest(tool, &result.projection)?
        {
            return Err(invalid(
                "result.identity",
                "project input or output identity does not match its exact projection",
            ));
        }
        if v.stages.len() != 9
            || v.stages[..8]
                .iter()
                .any(|s| s.status != StageStatus::Passed)
        {
            return Err(invalid(
                "stages",
                "publication requires every preceding stage to pass",
            ));
        }
    }
    if let Some(installed) = &v.installed_tool {
        let root = v
            .root
            .as_ref()
            .ok_or_else(|| invalid("root", "installed image requires actual project root"))?;
        let tool = v
            .tool
            .as_ref()
            .ok_or_else(|| invalid("tool", "installed image requires actual tool identity"))?;
        if installed.path.as_str() != format!("{}/.madb/wrapper/madb", root.as_str())
            || installed.sha256 != tool.sha256
            || installed.size != tool.size
        {
            return Err(invalid(
                "installed_tool",
                "installed image differs from the actual image or fixed path",
            ));
        }
    }
    if v.exit_code == 0 && (v.evidence_root.is_none() || v.diagnostics.len() > 128) {
        return Err(invalid(
            "evidence_root",
            "complete request requires saved local evidence",
        ));
    }
    let value = encode::report(v);
    section(
        &serde_json::json!({"logs":value["logs"],"attachments":value["stages"].as_array().into_iter().flatten().map(|s|&s["attachments"]).collect::<Vec<_>>()}),
        "log index",
    )?;
    section(
        &serde_json::json!({"diagnostics":value["diagnostics"],"recovery":value["recovery"],"stages":value["stages"].as_array().into_iter().flatten().map(|s|&s["diagnostics"]).collect::<Vec<_>>()}),
        "diagnostics",
    )?;
    Ok(())
}
