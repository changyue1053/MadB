//! Admission checks for the existing extension manifest format.
//!
//! These checks do not install packages, grant execution permissions, resolve
//! tool versions, verify package provenance, or certify declared routes. The
//! caller must still bind the selected packages, tools and files to a lock.
use madb_model::{LogicalPath, contract::CORE_FEATURES};
use madb_protocol::{FormatError, SchemaRegistry, Value, byte_digest};
use std::collections::{BTreeMap, BTreeSet};

/// Immutable, schema-checked manifest bytes. Decoding also accepts design
/// specimens; registration rejects `example: true` before using their claims.
#[derive(Clone, Debug)]
pub struct ExtensionManifest {
    value: Value,
    manifest_digest: String,
}

impl ExtensionManifest {
    pub fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        let schema = serde_json::from_str(include_str!(
            "../../../docs/schemas/extension-manifest.schema.json"
        ))
        .map_err(|error| FormatError::new("FMT000", "", format!("{error}")))?;
        Ok(Self {
            value: SchemaRegistry::validate_bundled(&schema, bytes)?,
            manifest_digest: byte_digest(bytes),
        })
    }

    pub fn id(&self) -> &str {
        text(&self.value["id"])
    }

    pub fn version(&self) -> &str {
        text(&self.value["version"])
    }

    /// Hash of the exact manifest bytes, not a new semantic projection.
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

/// Validate one explicitly selected closure. No discovery, global registry or
/// installation-order fallback occurs. External toolchain/generator dependency
/// identities are retained for the separate lock and permission admission.
pub fn validate_registration(manifests: &[ExtensionManifest]) -> Result<(), FormatError> {
    let mut packages = BTreeMap::new();
    let mut components = BTreeMap::new();
    let mut routes = BTreeSet::new();
    let mut languages = BTreeSet::new();

    for manifest in manifests {
        let id = manifest.id();
        let value = manifest.value();
        if value.get("example").and_then(Value::as_bool) == Some(true) {
            return Err(invalid(id, "design example cannot be registered"));
        }
        if packages.insert(id, manifest).is_some() {
            return Err(invalid(id, "duplicate extension identity"));
        }
        for component in items(&value["components"]) {
            let reference = format!("{id}/{}", text(&component["id"]));
            if components.insert(reference.clone(), component).is_some() {
                return Err(invalid(&reference, "duplicate component identity"));
            }
            valid_path(&component["entrypoint"]["path"], &reference)?;
        }
        for route in items(&value["routes"]) {
            let route_id = text(&route["id"]);
            if !routes.insert(route_id) {
                return Err(invalid(route_id, "duplicate route identity"));
            }
        }
        for language in items(&value["languages"]) {
            let language_id = text(&language["id"]);
            if !languages.insert(language_id) {
                return Err(invalid(language_id, "duplicate language declaration"));
            }
        }
        for asset in items(&value["assets"]) {
            valid_path(&asset["path"], id)?;
        }
    }

    let mut dependencies = BTreeMap::new();
    for manifest in manifests {
        let id = manifest.id();
        let mut all_dependencies = BTreeSet::new();
        let mut package_dependencies = BTreeSet::new();
        for dependency in items(&manifest.value()["dependencies"]) {
            let dependency_id = text(&dependency["id"]);
            let kind = text(&dependency["kind"]);
            if !all_dependencies.insert((kind, dependency_id)) {
                return Err(invalid(id, "duplicate dependency identity"));
            }
            if kind == "extension" {
                if !packages.contains_key(dependency_id) {
                    return Err(invalid(dependency_id, "missing extension dependency"));
                }
                package_dependencies.insert(dependency_id);
            }
        }
        for route in items(&manifest.value()["routes"]) {
            for (port, reference) in route["bindings"].as_object().expect("validated bindings") {
                let reference = text(reference);
                let (owner, _) = reference
                    .split_once('/')
                    .expect("validated component reference");
                if owner != id && !package_dependencies.contains(owner) {
                    return Err(invalid(
                        reference,
                        "component owner is not a declared extension dependency",
                    ));
                }
                let component = components
                    .get(reference)
                    .ok_or_else(|| invalid(reference, "component reference does not resolve"))?;
                if text(&component["port"]) != port {
                    return Err(invalid(
                        reference,
                        "component port does not match route binding",
                    ));
                }
            }
        }
        dependencies.insert(id, package_dependencies);
    }

    // Iteration avoids a stack-depth dependency on a caller's package graph.
    let mut resolved = BTreeSet::new();
    while resolved.len() < dependencies.len() {
        let next = dependencies.iter().find(|(id, needed)| {
            !resolved.contains(**id) && needed.iter().all(|id| resolved.contains(id))
        });
        match next {
            Some((id, _)) => {
                resolved.insert(*id);
            }
            None => return Err(invalid("dependencies", "extension dependency cycle")),
        }
    }
    Ok(())
}

/// Check the mandatory semantics of a particular component operation. A
/// builder does not have to claim every semantic feature of a complete route.
/// Unknown optional declarations are not promoted into recognized semantics.
pub fn require_component_features(
    manifest: &ExtensionManifest,
    component_id: &str,
    required: &[&str],
) -> Result<(), FormatError> {
    let component = items(&manifest.value()["components"])
        .iter()
        .find(|component| text(&component["id"]) == component_id)
        .ok_or_else(|| invalid(component_id, "component reference does not resolve"))?;
    // A decoded document alone can contain two structurally different entries
    // with the same ID. Do not let this API select the first one silently.
    if items(&manifest.value()["components"])
        .iter()
        .filter(|component| text(&component["id"]) == component_id)
        .count()
        != 1
    {
        return Err(invalid(component_id, "ambiguous component identity"));
    }
    for feature in required {
        if !CORE_FEATURES.contains(feature)
            || !items(&component["supported_features"])
                .iter()
                .any(|value| text(value) == *feature)
        {
            return Err(FormatError::new(
                "CapabilityUnsupported",
                format!("{}/{component_id}", manifest.id()),
                format!("unknown or unsupported mandatory feature: {feature}"),
            ));
        }
    }
    Ok(())
}

fn valid_path(value: &Value, subject: &str) -> Result<(), FormatError> {
    LogicalPath::new(text(value))
        .map(|_| ())
        .map_err(|error| invalid(subject, &format!("invalid package path: {error}")))
}

fn invalid(subject: &str, message: &str) -> FormatError {
    FormatError::new("InputInvalid", subject, message)
}

fn text(value: &Value) -> &str {
    value.as_str().expect("schema-validated string")
}

fn items(value: &Value) -> &[Value] {
    value.as_array().expect("schema-validated array")
}
