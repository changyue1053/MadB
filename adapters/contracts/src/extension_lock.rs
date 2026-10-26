//! Checks for the component/image mapping already expressible by v1 locks.
//!
//! This entry point does not install, execute, authenticate a publisher, or
//! resolve a package archive. The host supplies confined bytes and an observed
//! source; string equality with a locked source is not proof of its provenance.
//! Every dependency component needs its own checked pins. No protocol/IR lock
//! selection is invented: v1 lock has no dedicated fields for those choices.
use crate::{
    Document, DocumentKind, FormatError, SchemaRegistry,
    artifacts::{ProvidedFile, verify_reference},
    extension::{ExtensionManifest, require_component_features, validate_registration},
    profile_digest, project_digest,
};
use madb_app::{
    Digest, LogicalPath,
    ports::FileReference,
    process::{MAX_TOOL_BYTES, ProcessPolicy},
};
use madb_protocol::{Value, byte_digest};
use std::collections::BTreeSet;

pub struct ExtensionPinContext<'a> {
    pub project: &'a Document,
    pub lock: &'a Document,
    pub profile: ProvidedFile<'a>,
    pub boundary_id: &'a str,
    pub policy: &'a ProcessPolicy,
}

/// Explicit host selection. Paths use the caller's existing locked namespace;
/// the image path must be exactly the manifest's relative process entrypoint.
pub struct ComponentPinRequest<'a> {
    pub registration: &'a [ExtensionManifest],
    pub manifest: ProvidedFile<'a>,
    pub image: ProvidedFile<'a>,
    pub component_id: &'a str,
    pub observed_host: &'a str,
    pub observed_source: &'a str,
    pub required_features: &'a [&'a str],
}

/// A checked selection, not execution permission or package certification.
/// The caller must still use the host's confined process and result admission.
pub struct CheckedExtensionPins {
    manifest: ExtensionManifest,
    component_id: String,
    image: FileReference,
    locked_tool: Value,
    profile_digest: String,
}
impl CheckedExtensionPins {
    pub fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }
    pub fn component_id(&self) -> &str {
        &self.component_id
    }
    pub fn image(&self) -> &FileReference {
        &self.image
    }
    pub fn locked_tool(&self) -> &Value {
        &self.locked_tool
    }
    pub fn profile_digest(&self) -> &str {
        &self.profile_digest
    }
}

pub fn check_extension_pins(
    context: ExtensionPinContext<'_>,
    request: ComponentPinRequest<'_>,
) -> Result<CheckedExtensionPins, FormatError> {
    if context.project.kind() != DocumentKind::Project || context.lock.kind() != DocumentKind::Lock
    {
        return Err(error(
            "FMT003",
            "context",
            "expected project and lock documents",
        ));
    }
    let profile = SchemaRegistry::new()?.decode(DocumentKind::Profile, context.profile.bytes)?;
    let lock = context.lock.value();
    check_unique(lock)?;
    let digest = check_profile(&context, &profile)?;
    validate_registration(request.registration)?;
    let manifest = ExtensionManifest::decode(request.manifest.bytes)?;
    let registered = request
        .registration
        .iter()
        .find(|item| item.id() == manifest.id())
        .ok_or_else(|| {
            error(
                "InputInvalid",
                "registration",
                "selected manifest is not registered",
            )
        })?;
    if registered.manifest_digest() != manifest.manifest_digest() {
        return Err(error(
            "ExtensionIdentityMismatch",
            "manifest",
            "registered manifest bytes differ",
        ));
    }
    let manifest_pin = one(&lock["inputs"], "path", request.manifest.path, "LOCK005")?;
    verify_reference(manifest_pin, &request.manifest)?;
    require_component_features(&manifest, request.component_id, request.required_features)?;
    let component = one(
        &manifest.value()["components"],
        "id",
        request.component_id,
        "InputInvalid",
    )?;
    let full_id = format!("{}/{}", manifest.id(), request.component_id);
    let component_pin = one(&lock["components"], "id", &full_id, "LOCK004")?;
    if component_pin["version"] != manifest.version() {
        return Err(error(
            "ExtensionIdentityMismatch",
            &full_id,
            "component and manifest versions differ",
        ));
    }
    // This narrow entry point supports the existing single-image mapping used
    // by task binding. It never guesses how a different tool ID maps to a package.
    let tool = one(&profile.value()["tools"], "id", &full_id, "LOCK004")?;
    for key in ["id", "version", "sha256"] {
        if tool[key] != component_pin[key] {
            return Err(error(
                "LOCK004",
                key,
                "component has no exact selected tool image mapping",
            ));
        }
    }
    let locked_tool = one(&lock["tools"], "id", &full_id, "LOCK004")?;
    if tool != locked_tool {
        return Err(error(
            "LOCK004",
            &full_id,
            "profile tool tuple differs from lock",
        ));
    }
    if tool["target"] != profile.value()["target"] || tool["host"] != request.observed_host {
        return Err(error(
            "LOCK004",
            "host/target",
            "image host or selected target differs",
        ));
    }
    if tool["source"] != request.observed_source {
        return Err(error(
            "LOCK004",
            "source",
            "observed source differs from locked source",
        ));
    }
    if tool["arguments"] != component["entrypoint"]["args"]
        || component["entrypoint"]["path"] != request.image.path
    {
        return Err(error(
            "ExtensionIdentityMismatch",
            "entrypoint",
            "manifest entrypoint differs from selected image or arguments",
        ));
    }
    if request.image.bytes.is_empty()
        || request.image.bytes.len() as u64 > MAX_TOOL_BYTES
        || tool["sha256"] != byte_digest(request.image.bytes)
    {
        return Err(error(
            "LOCK004",
            "image",
            "installed image bytes differ or exceed budget",
        ));
    }
    if component["entrypoint"]["args"]
        .as_array()
        .expect("checked args")
        .iter()
        .any(|arg| arg.as_str().expect("checked arg").contains('\0'))
    {
        return Err(error(
            "InputInvalid",
            "arguments",
            "process arguments contain NUL",
        ));
    }
    if manifest.value()["execution"]["network"] != "none" {
        return Err(error(
            "CapabilityUnsupported",
            "network",
            "this host policy has no network-request grant",
        ));
    }
    for name in manifest.value()["execution"]["env"]
        .as_array()
        .expect("checked env")
    {
        if !context
            .policy
            .allowed_environment
            .contains(name.as_str().expect("checked name"))
        {
            return Err(error(
                "InputInvalid",
                "environment",
                "requested environment name is not allowed by host policy",
            ));
        }
    }
    let image = FileReference {
        path: LogicalPath::new(request.image.path)
            .map_err(|e| error("InputInvalid", "image", e.to_string()))?,
        digest: Digest::of_bytes(request.image.bytes),
        size: request.image.bytes.len() as u64,
    };
    Ok(CheckedExtensionPins {
        manifest,
        component_id: request.component_id.to_owned(),
        image,
        locked_tool: locked_tool.clone(),
        profile_digest: digest,
    })
}

fn check_profile(
    context: &ExtensionPinContext<'_>,
    profile: &Document,
) -> Result<String, FormatError> {
    let lock = context.lock.value();
    let project = context.project.value();
    if lock["project_digest"] != project_digest(context.project)? {
        return Err(error(
            "LOCK002",
            "project_digest",
            "project differs from lock",
        ));
    }
    unique(&project["boundaries"], "id")?;
    unique(&profile.value()["tools"], "id")?;
    let boundary = one(&project["boundaries"], "id", context.boundary_id, "LOCK001")?;
    let pin = one(
        &lock["profiles"],
        "id",
        profile.value()["id"].as_str().expect("checked id"),
        "LOCK001",
    )?;
    if pin["file"] != boundary["profile"]
        || pin["file"]["schema"] != DocumentKind::Profile.schema_id()
    {
        return Err(error(
            "LOCK003",
            "profile",
            "boundary and locked profile file differ",
        ));
    }
    verify_reference(&pin["file"], &context.profile)?;
    for key in ["route", "direction", "target"] {
        if boundary[key] != profile.value()[key] {
            return Err(error(
                "LOCK001",
                key,
                "boundary and profile selection differ",
            ));
        }
    }
    let digest = profile_digest(profile)?;
    if pin["semantic_digest"] != digest {
        return Err(error(
            "LOCK003",
            "profile",
            "profile semantic identity differs",
        ));
    }
    Ok(digest)
}

fn check_unique(lock: &Value) -> Result<(), FormatError> {
    for key in ["profiles", "tools", "components"] {
        unique(&lock[key], "id")?;
    }
    let mut paths = BTreeSet::new();
    for value in ["inputs", "schemas"]
        .into_iter()
        .flat_map(|key| lock[key].as_array().expect("checked array"))
    {
        let path = value["path"].as_str().expect("checked path");
        LogicalPath::new(path).map_err(|e| error("InputInvalid", path, e.to_string()))?;
        if !paths.insert(path.to_ascii_lowercase()) {
            return Err(error(
                "LOCK006",
                path,
                "duplicate or case-alias tracked path",
            ));
        }
    }
    Ok(())
}

fn unique(values: &Value, key: &str) -> Result<(), FormatError> {
    let mut keys = BTreeSet::new();
    for value in values.as_array().expect("schema checked array") {
        if !keys.insert(value[key].as_str().expect("schema checked key")) {
            return Err(error(
                "LOCK006",
                key,
                "duplicate locked or selected identity",
            ));
        }
    }
    Ok(())
}

fn one<'a>(
    values: &'a Value,
    key: &str,
    expected: &str,
    code: &'static str,
) -> Result<&'a Value, FormatError> {
    let matches = values
        .as_array()
        .expect("schema checked array")
        .iter()
        .filter(|value| value[key] == expected)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(error(
            code,
            expected,
            "selected identity is missing or ambiguous",
        ));
    }
    Ok(matches[0])
}
fn error(code: &'static str, subject: &str, message: impl Into<String>) -> FormatError {
    FormatError::new(code, subject, message)
}
