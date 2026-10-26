use super::pins::{RecipeRequirements, digest, index, logical};
use super::{Result, invalid, text};
use crate::extension::ExtensionManifest;
use madb_model::{Digest, Id, LogicalPath, PublicId};
use madb_protocol::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Input provenance, not a runtime object identity or a compiled export claim.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AssetOrigin {
    Source {
        package: Digest,
        paths: Vec<LogicalPath>,
    },
    Generated {
        package: Digest,
        producer: Id,
        slot: Id,
        paths: Vec<LogicalPath>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ResolvedSubject {
    PublicSymbol(PublicId),
    SdkExport { origin: AssetOrigin, member: Id },
    Output(LogicalPath),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ResolvedScope {
    pub kind: Id,
    pub anchor: ResolvedSubject,
    pub layer: Id,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ResponsibilityKey {
    pub boundary: Id,
    pub subject: ResolvedSubject,
    pub stage: Id,
    pub scope: ResolvedScope,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ResolvedOwner {
    Component { id: Id, package: Digest },
    SdkAsset(AssetOrigin),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResponsibility {
    pub key: ResponsibilityKey,
    pub owner: ResolvedOwner,
}

/// Compare independently established facts. Callers must obtain `required`
/// from the fixed host recipe and checked IR/exports/wiring, not the submitted
/// responsibility list. Equality alone never establishes acceptance authority.
pub fn compare_responsibility_sets(
    required: &[ResolvedResponsibility],
    provided: &[ResolvedResponsibility],
) -> Result<()> {
    let required: Vec<_> = required.iter().map(|row| (&row.key, &row.owner)).collect();
    let provided: Vec<_> = provided.iter().map(|row| (&row.key, &row.owner)).collect();
    madb_rules::compare_unique_owners(&required, &provided).map_err(|error| match error {
        madb_rules::OwnerSetError::EmptyRequired => invalid(
            "InputInvalid",
            "required",
            "no independent responsibility requirements",
        ),
        madb_rules::OwnerSetError::DuplicateKey => invalid(
            "InputInvalid",
            "responsibilities",
            "duplicate responsibility key or conflicting owner",
        ),
        madb_rules::OwnerSetError::Mismatch => invalid(
            "InputInvalid",
            "responsibilities",
            "missing, extra, or mismatched responsibility",
        ),
    })
}

pub(super) fn bind_assets(
    c: &Value,
    recipe: &RecipeRequirements,
    manifests: &BTreeMap<String, ExtensionManifest>,
    components: &BTreeMap<String, Digest>,
) -> Result<BTreeMap<String, AssetOrigin>> {
    let packages = index(&c["packages"], "id")?;
    let mut slots = BTreeMap::new();
    for slot in &recipe.generated_slots {
        let mut paths = slot.paths.clone();
        paths.sort();
        if paths.is_empty()
            || paths.windows(2).any(|p| p[0] == p[1])
            || slots
                .insert((slot.producer.as_str(), slot.slot.as_str()), paths)
                .is_some()
        {
            return Err(invalid(
                "InputInvalid",
                "recipe",
                "duplicate or empty generated slot",
            ));
        }
    }
    let mut used_slots = BTreeSet::new();
    let mut source_owners = BTreeSet::new();
    let mut output_paths = Vec::new();
    let mut assets = BTreeMap::new();
    for (id, asset) in index(&c["assets"], "id")? {
        let package_id = text(&asset["package"])?;
        let package = packages
            .get(package_id)
            .ok_or_else(|| invalid("InputInvalid", id, "asset package is absent"))?;
        let package_digest = digest(&package["source_digest"])?;
        let source = &asset["source"];
        let origin = if source["kind"] == "source" {
            let available = index(&package["source"]["files"], "path")?;
            let files = index(&source["files"], "path")?;
            let manifest = manifests[package_id].value();
            let mut paths = Vec::new();
            for (path, file) in files {
                if available.get(path).copied() != Some(file) {
                    return Err(invalid(
                        "FILE004",
                        path,
                        "asset is not an exact package source",
                    ));
                }
                let covered = manifest["assets"]
                    .as_array()
                    .expect("checked assets")
                    .iter()
                    .any(|entry| {
                        entry["kind"] == "sdk"
                            && entry["path"].as_str().is_some_and(|prefix| {
                                path == prefix
                                    || path
                                        .strip_prefix(prefix)
                                        .is_some_and(|tail| tail.starts_with('/'))
                            })
                    });
                if !covered {
                    return Err(invalid(
                        "InputInvalid",
                        path,
                        "source asset is not covered by a manifest SDK path",
                    ));
                }
                let path = logical(&file["path"])?;
                if !source_owners.insert((package_digest.clone(), path.clone())) {
                    return Err(invalid(
                        "InputInvalid",
                        id,
                        "two asset identities claim the same source",
                    ));
                }
                paths.push(path);
            }
            AssetOrigin::Source {
                package: package_digest,
                paths,
            }
        } else {
            let producer = text(&source["producer"])?;
            let slot = text(&source["output_slot"])?;
            if !components.contains_key(producer) {
                return Err(invalid(
                    "InputInvalid",
                    producer,
                    "generated producer is not selected",
                ));
            }
            let (owner, local) = producer
                .split_once('/')
                .ok_or_else(|| invalid("InputInvalid", producer, "invalid producer identity"))?;
            if owner != package_id {
                return Err(invalid(
                    "InputInvalid",
                    id,
                    "generated asset package differs from producer package",
                ));
            }
            let declared = index(&manifests[owner].value()["components"], "id")?;
            if declared[local]["port"] != "generator" {
                return Err(invalid(
                    "InputInvalid",
                    producer,
                    "generated producer is not a generator",
                ));
            }
            let mut paths = source["paths"]
                .as_array()
                .expect("checked paths")
                .iter()
                .map(logical)
                .collect::<Result<Vec<_>>>()?;
            paths.sort();
            if slots.get(&(producer, slot)) != Some(&paths) || !used_slots.insert((producer, slot))
            {
                return Err(invalid(
                    "InputInvalid",
                    id,
                    "generated slot differs from host recipe or has aliases",
                ));
            }
            output_paths.extend(paths.iter().cloned());
            AssetOrigin::Generated {
                package: package_digest,
                producer: identifier(&source["producer"])?,
                slot: identifier(&source["output_slot"])?,
                paths,
            }
        };
        assets.insert(id.into(), origin);
    }
    if used_slots.len() != slots.len() {
        return Err(invalid(
            "InputInvalid",
            "assets",
            "required generated slot is absent",
        ));
    }
    check_output_paths(&output_paths)?;
    Ok(assets)
}

// These are future output names, not FileReferences: inventing hashes merely
// to reuse a byte-inventory checker would conflate intent with materialization.
pub(super) fn check_output_paths(paths: &[LogicalPath]) -> Result<()> {
    let mut claims = BTreeMap::<String, (String, bool)>::new();
    for logical in paths {
        let path = logical.as_str();
        if !path.is_ascii() || path.split('/').count() > 32 {
            return Err(invalid(
                "InputInvalid",
                path,
                "portable generated paths require ASCII and at most 32 components",
            ));
        }
        for end in path.match_indices('/').map(|(i, _)| i).chain([path.len()]) {
            let prefix = &path[..end];
            let file = end == path.len();
            if let Some((old, was_file)) = claims.get(&prefix.to_ascii_lowercase()) {
                if old != prefix || *was_file || file {
                    return Err(invalid(
                        "InputInvalid",
                        path,
                        "generated output alias or file/directory conflict",
                    ));
                }
            } else {
                claims.insert(prefix.to_ascii_lowercase(), (prefix.into(), file));
            }
        }
    }
    Ok(())
}

pub(super) fn check_dependencies(
    c: &Value,
    assets: &BTreeMap<String, AssetOrigin>,
    components: &BTreeMap<String, Digest>,
) -> Result<()> {
    let mut nodes = BTreeSet::new();
    let packages = index(&c["packages"], "id")?;
    for id in assets
        .keys()
        .chain(components.keys())
        .map(String::as_str)
        .chain(packages.keys().copied())
    {
        if !nodes.insert(id) {
            return Err(invalid(
                "InputInvalid",
                id,
                "ambiguous dependency identity across selected namespaces",
            ));
        }
    }
    let mut edges = BTreeSet::new();
    let mut needed = BTreeMap::<&str, BTreeSet<&str>>::new();
    for id in &nodes {
        needed.insert(*id, BTreeSet::new());
    }
    for dep in c["dependencies"].as_array().expect("checked dependencies") {
        let from = text(&dep["from"])?;
        let to = text(&dep["to"])?;
        let scope = text(&dep["scope"])?;
        if !nodes.contains(from) || !nodes.contains(to) || !edges.insert((from, to, scope)) {
            return Err(invalid(
                "InputInvalid",
                "dependencies",
                "missing, ambiguous or duplicate dependency",
            ));
        }
        needed.get_mut(from).expect("known source").insert(to);
    }
    let mut done = BTreeSet::new();
    while done.len() < nodes.len() {
        let next = needed
            .iter()
            .find(|(id, deps)| !done.contains(**id) && deps.iter().all(|id| done.contains(id)));
        if let Some((id, _)) = next {
            done.insert(*id);
        } else {
            return Err(invalid(
                "InputInvalid",
                "dependencies",
                "composition dependency cycle",
            ));
        }
    }
    Ok(())
}

pub(super) fn resolve_responsibilities(
    c: &Value,
    boundary: &str,
    assets: &BTreeMap<String, AssetOrigin>,
    components: &BTreeMap<String, Digest>,
) -> Result<Vec<ResolvedResponsibility>> {
    let wire_assets = index(&c["assets"], "id")?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for r in c["responsibilities"]
        .as_array()
        .expect("checked responsibilities")
    {
        if r["boundary"] != boundary {
            return Err(invalid(
                "InputInvalid",
                "boundary",
                "responsibility is for a different boundary",
            ));
        }
        let key = ResponsibilityKey {
            boundary: identifier(&r["boundary"])?,
            subject: subject(&r["subject"], assets)?,
            stage: identifier(&r["stage"])?,
            scope: ResolvedScope {
                kind: identifier(&r["scope"]["kind"])?,
                anchor: subject(&r["scope"]["anchor"], assets)?,
                layer: identifier(&r["scope"]["layer"])?,
            },
        };
        if !seen.insert(key.clone()) {
            return Err(invalid(
                "InputInvalid",
                "responsibilities",
                "duplicate normalized responsibility key",
            ));
        }
        let owner = &r["owner"];
        let owner = if owner["kind"] == "component" {
            let id = text(&owner["id"])?;
            ResolvedOwner::Component {
                id: identifier(&owner["id"])?,
                package: components
                    .get(id)
                    .ok_or_else(|| {
                        invalid(
                            "InputInvalid",
                            id,
                            "responsibility component is not selected",
                        )
                    })?
                    .clone(),
            }
        } else {
            let id = text(&owner["asset"])?;
            let asset = wire_assets
                .get(id)
                .ok_or_else(|| invalid("InputInvalid", id, "responsibility asset is absent"))?;
            if asset["package"] != owner["package"] {
                return Err(invalid(
                    "InputInvalid",
                    id,
                    "owner names the wrong asset package",
                ));
            }
            ResolvedOwner::SdkAsset(assets[id].clone())
        };
        rows.push(ResolvedResponsibility { key, owner });
    }
    Ok(rows)
}
fn subject(v: &Value, assets: &BTreeMap<String, AssetOrigin>) -> Result<ResolvedSubject> {
    Ok(match text(&v["kind"])? {
        "public-symbol" => ResolvedSubject::PublicSymbol(
            PublicId::new(text(&v["public_id"])?)
                .map_err(|e| invalid("InputInvalid", "subject", e.to_string()))?,
        ),
        "sdk-export" => {
            let id = text(&v["asset"])?;
            ResolvedSubject::SdkExport {
                origin: assets
                    .get(id)
                    .ok_or_else(|| invalid("InputInvalid", id, "SDK subject asset is absent"))?
                    .clone(),
                member: identifier(&v["name"])?,
            }
        }
        "output" => {
            let path = logical(&v["path"])?;
            if !assets.values().any(|origin| matches!(origin, AssetOrigin::Generated { paths, .. } if paths.contains(&path))) { return Err(invalid("InputInvalid", path.as_str(), "output subject is not a declared generated file")); }
            ResolvedSubject::Output(path)
        }
        _ => return Err(invalid("InputInvalid", "subject", "unknown subject kind")),
    })
}
fn identifier(v: &Value) -> Result<Id> {
    Id::new(text(v)?).map_err(|e| invalid("InputInvalid", "identity", e.to_string()))
}
