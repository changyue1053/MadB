//! Cross-document checks. Byte loading and filesystem confinement belong to the host.
use crate::wire::Wire;
use crate::{
    Document, DocumentKind, FormatError, capability, contract, profile_digest, project_digest,
    source_digest,
};
use std::collections::BTreeSet;

pub fn check_frozen_inputs(
    ir: &Document,
    project: &Document,
    profile: &Document,
    lock: &Document,
) -> Result<(), FormatError> {
    let m = contract(ir)?;
    let c = capability(profile)?;
    let p = Wire::root(project, DocumentKind::Project)?;
    let l = Wire::root(lock, DocumentKind::Lock)?;
    if m.namespace != p.field("sdk_namespace")?.id()? || m.selector != c.selector {
        return Err(FormatError::new(
            "LOCK001",
            "/selector",
            "snapshot, project and resolved profile disagree",
        ));
    }
    if l.field("project_digest")?.string()? != project_digest(project)? {
        return Err(FormatError::new(
            "LOCK002",
            "/project_digest",
            "project policies or configuration changed",
        ));
    }
    let boundaries = p.field("boundaries")?.array()?;
    unique(&boundaries, "id")?;
    let selected: Vec<_> = boundaries
        .iter()
        .filter(|b| b.value["id"].as_str() == Some(m.boundary_id.as_str()))
        .collect();
    if selected.len() != 1 {
        return Err(FormatError::new(
            "LOCK001",
            "/boundaries",
            "snapshot boundary is not declared exactly once",
        ));
    }
    let boundary = selected[0];
    for field in ["route", "direction", "target"] {
        if boundary.value[field] != profile.value()[field] {
            return Err(FormatError::new(
                "LOCK001",
                field,
                "boundary is unresolved or differs from profile",
            ));
        }
    }
    let profiles = l.field("profiles")?.array()?;
    unique(&profiles, "id")?;
    let matched = profiles
        .iter()
        .find(|v| v.value["id"] == profile.value()["id"])
        .ok_or_else(|| FormatError::new("LOCK001", "/profiles", "profile is not locked"))?;
    if matched.field("semantic_digest")?.string()? != profile_digest(profile)?
        || matched.value["file"] != boundary.value["profile"]
    {
        return Err(FormatError::new(
            "LOCK003",
            "/profiles",
            "profile file or semantic identity differs",
        ));
    }
    let tools = l.field("tools")?.array()?;
    unique(&tools, "id")?;
    for tool in profile.value()["tools"]
        .as_array()
        .expect("schema-validated tools")
    {
        if !tools.iter().any(|t| t.value == tool) {
            return Err(FormatError::new(
                "LOCK004",
                "/tools",
                "profile tool is not exactly locked",
            ));
        }
        if tool["target"] != profile.value()["target"] {
            return Err(FormatError::new(
                "LOCK004",
                "/tools/target",
                "tool targets a different profile",
            ));
        }
    }
    unique(&l.field("components")?.array()?, "id")?;
    unique(&l.field("schemas")?.array()?, "path")?;
    if source_digest(l.field("inputs")?.value)? != m.source_digest.as_str() {
        return Err(FormatError::new(
            "LOCK005",
            "/inputs",
            "snapshot source inventory changed",
        ));
    }
    if !l
        .field("inputs")?
        .array()?
        .iter()
        .any(|v| v.value["path"] == boundary.value["source"]["path"])
    {
        return Err(FormatError::new(
            "LOCK005",
            "/inputs",
            "authoritative source is not tracked",
        ));
    }
    Ok(())
}
fn unique(items: &[Wire<'_>], field: &str) -> Result<(), FormatError> {
    let mut keys = BTreeSet::new();
    for item in items {
        if !keys.insert(item.field(field)?.string()?) {
            return Err(FormatError::new(
                "LOCK006",
                field,
                "duplicate locked identity",
            ));
        }
    }
    Ok(())
}
