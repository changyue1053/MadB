use super::invalid;
use madb_app::{Digest, Id, LogicalPath, Result};
use madb_model::{Location, evidence::EvidenceLevel};
use madb_plan::project::{
    CheckLevel, ComponentIdentity, FileIdentity, FileOwnership, FileReference, Version,
    VersionRange,
};
use madb_protocol::Value;
pub(super) fn string(v: &Value) -> Result<&str> {
    v.as_str()
        .ok_or_else(|| invalid("value", "expected string"))
}
pub(super) fn array(v: &Value) -> Result<&[Value]> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("value", "expected array"))
}
pub(super) fn id(v: &Value) -> Result<Id> {
    Id::new(string(v)?).map_err(|e| invalid("id", e.to_string()))
}
pub(super) fn path(v: &Value) -> Result<LogicalPath> {
    LogicalPath::new(string(v)?).map_err(|e| invalid("path", e.to_string()))
}
pub(super) fn digest(v: &Value) -> Result<Digest> {
    Digest::new(string(v)?).map_err(|e| invalid("digest", e.to_string()))
}
pub(super) fn uint(v: &Value) -> Result<u64> {
    v.as_u64()
        .ok_or_else(|| invalid("value", "expected safe unsigned integer"))
}
pub(super) fn integer(v: &Value) -> Result<i64> {
    v.as_i64()
        .ok_or_else(|| invalid("value", "expected safe integer"))
}
pub(super) fn boolean(v: &Value) -> Result<bool> {
    v.as_bool()
        .ok_or_else(|| invalid("value", "expected boolean"))
}
pub(super) fn ids(v: &Value) -> Result<Vec<Id>> {
    array(v)?.iter().map(id).collect()
}
pub(super) fn strings(v: &Value) -> Result<Vec<String>> {
    array(v)?
        .iter()
        .map(|v| Ok(string(v)?.to_owned()))
        .collect()
}
pub(super) fn optional<T>(v: &Value, parse: impl FnOnce(&Value) -> Result<T>) -> Result<Option<T>> {
    if v.is_null() {
        Ok(None)
    } else {
        parse(v).map(Some)
    }
}
pub(super) fn version(v: &Value) -> Result<Version> {
    Version::parse(string(v)?).map_err(|e| invalid(e.subject, e.message))
}
pub(super) fn versions(v: &Value) -> Result<VersionRange> {
    let r = VersionRange {
        min_inclusive: version(&v["min_inclusive"])?,
        max_exclusive: version(&v["max_exclusive"])?,
    };
    r.validate().map_err(|e| invalid(e.subject, e.message))?;
    Ok(r)
}
pub(super) fn file(v: &Value) -> Result<FileIdentity> {
    Ok(FileIdentity {
        path: path(&v["path"])?,
        sha256: digest(&v["sha256"])?,
        size: uint(&v["size"])?,
    })
}
pub(super) fn reference(v: &Value) -> Result<FileReference> {
    Ok(FileReference {
        file: file(v)?,
        schema: id(&v["schema"])?,
    })
}
pub(super) fn component(v: &Value) -> Result<ComponentIdentity> {
    Ok(ComponentIdentity {
        id: id(&v["id"])?,
        version: string(&v["version"])?.into(),
        sha256: digest(&v["sha256"])?,
    })
}
pub(super) fn ownership(v: &Value) -> Result<FileOwnership> {
    match string(v)? {
        "user" => Ok(FileOwnership::User),
        "managed" => Ok(FileOwnership::Managed),
        "local-state" => Ok(FileOwnership::LocalState),
        "provenance" => Ok(FileOwnership::Provenance),
        _ => Err(invalid("ownership", "unsupported ownership")),
    }
}
pub(super) fn level(v: &Value) -> Result<CheckLevel> {
    match string(v)? {
        "structure" => Ok(CheckLevel::Structure),
        "build" => Ok(CheckLevel::Build),
        "run" => Ok(CheckLevel::Run),
        "import" => Ok(CheckLevel::Import),
        "debug" => Ok(CheckLevel::Debug),
        _ => Err(invalid("level", "unknown check level")),
    }
}
pub(super) fn evidence(v: &Value) -> Result<EvidenceLevel> {
    match string(v)? {
        "static" => Ok(EvidenceLevel::Static),
        "generated" => Ok(EvidenceLevel::Generated),
        "compiled" => Ok(EvidenceLevel::Compiled),
        "linked" => Ok(EvidenceLevel::Linked),
        "host-run" => Ok(EvidenceLevel::HostRun),
        "target-run" => Ok(EvidenceLevel::TargetRun),
        "device-run" => Ok(EvidenceLevel::DeviceRun),
        _ => Err(invalid("evidence", "unknown evidence level")),
    }
}
pub(super) fn location(v: &Value) -> Result<Location> {
    Ok(Location {
        path: path(&v["path"])?,
        line: uint(&v["line"])?,
        column: uint(&v["column"])?,
    })
}
