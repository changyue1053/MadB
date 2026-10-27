use super::ownership::{ResolvedResponsibility, resolve_responsibilities};
use super::{Result, composition_digest, invalid, profile_v2_digest, text};
use crate::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::{ProvidedFile, verify_reference},
    extension::{ExtensionManifest, validate_registration},
    project_digest, source_digest,
};
use madb_app::{
    inputs::{FrozenInputs, check_input_inventory},
    ports::FileReference,
};
use madb_model::{
    Digest, Id, LogicalPath,
    contract::{CORE_FEATURES, Direction, EXTENSION_COMPOSITION},
};
use madb_protocol::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Facts from the host's fixed recipe, never inferred from the submitted claims.
pub struct RecipeRequirements {
    pub id: Id,
    pub selection: RecipeSelection,
    pub components: Vec<ComponentRequirement>,
    pub generated_slots: Vec<GeneratedSlot>,
    /// Exact dependency uses established by this host recipe's sources/wiring.
    /// Never derive this list from the submitted composition dependency rows.
    pub dependencies: Vec<DependencyRequirement>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRequirement {
    pub from: Id,
    pub to: Id,
    pub scope: DependencyScope,
}
/// Existing composition/1 scopes, without an unvalidated string fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyScope {
    HostTool,
    AuthorPlugin,
    ConsumerCompile,
    TargetRuntime,
    OptionalIntegration,
    ValidationDev,
}
impl DependencyScope {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::HostTool => "host-tool",
            Self::AuthorPlugin => "author-plugin",
            Self::ConsumerCompile => "consumer-compile",
            Self::TargetRuntime => "target-runtime",
            Self::OptionalIntegration => "optional-integration",
            Self::ValidationDev => "validation-dev",
        }
    }
}
pub struct RecipeSelection {
    pub profile_id: Id,
    pub route: Id,
    pub direction: Direction,
    pub target: Id,
}
pub struct ComponentRequirement {
    pub id: Id,
    pub port: Id,
    /// Additional interpreters (e.g. a semantic packager). Readers/generators
    /// always need the composition feature in this explicit /2 entry point.
    pub interprets_composition: bool,
}
pub struct GeneratedSlot {
    pub producer: Id,
    pub slot: Id,
    pub paths: Vec<LogicalPath>,
}
/// Host-selected logical locations of the bundled schemas actually used here.
/// These paths do not let a caller substitute schema contents or validators.
pub struct CompositionSchemaPaths {
    pub common: LogicalPath,
    pub project: LogicalPath,
    pub lock: LogicalPath,
    pub profile: LogicalPath,
    pub composition: LogicalPath,
    pub extension: LogicalPath,
    pub contract_ir: LogicalPath,
}
pub struct CompositionInputContext<'a> {
    pub project: &'a Document,
    pub lock: &'a Document,
    pub profile: ProvidedFile<'a>,
    pub boundary_id: &'a str,
    /// Exact union of lock.inputs and lock.schemas. Host path confinement and
    /// dependency completeness are not established by this byte snapshot.
    pub inputs: &'a FrozenInputs,
    pub schema_paths: &'a CompositionSchemaPaths,
}

/// Read-only observations. This deliberately implements no execution/commit
/// interface and contains no claim that a real recipe or SDK was accepted.
pub struct BoundCompositionInputs {
    pub(super) locked_document: Document,
    pub(super) profile_reference: FileReference,
    pub(super) frozen_inputs: FrozenInputs,
    composition: Document,
    profile: Document,
    profile_digest: String,
    source_digest: String,
    namespace: String,
    boundary: String,
    responsibilities: Vec<ResolvedResponsibility>,
}
impl BoundCompositionInputs {
    pub(super) fn boundary_id(&self) -> &str {
        &self.boundary
    }
    pub fn composition(&self) -> &Document {
        &self.composition
    }
    pub fn profile(&self) -> &Document {
        &self.profile
    }
    pub fn profile_digest(&self) -> &str {
        &self.profile_digest
    }
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
    pub fn responsibilities(&self) -> &[ResolvedResponsibility] {
        &self.responsibilities
    }
}

pub fn bind_composition_inputs(
    context: CompositionInputContext<'_>,
    recipe: &RecipeRequirements,
) -> Result<BoundCompositionInputs> {
    if context.project.kind() != DocumentKind::Project || context.lock.kind() != DocumentKind::Lock
    {
        return Err(invalid("FMT003", "context", "expected project and lock"));
    }
    let registry = SchemaRegistry::new()?;
    let profile = registry.decode(DocumentKind::ProfileV2, context.profile.bytes)?;
    let p = profile.value();
    check_features(p)?;
    // Backend-specific semantics need their own explicit adapter. This entry
    // point must not turn a bound FileRef into an ignored configuration.
    if !p["backend_config"].is_null() {
        return Err(invalid(
            "CAP005",
            "backend_config",
            "registered backend validation is required",
        ));
    }
    let lock = context.lock.value();
    let project = context.project.value();
    if lock["project_digest"] != project_digest(context.project)? {
        return Err(invalid(
            "LOCK002",
            "project_digest",
            "project differs from lock",
        ));
    }
    let boundaries = index(&project["boundaries"], "id")?;
    let boundary = boundaries
        .get(context.boundary_id)
        .ok_or_else(|| invalid("LOCK001", "boundary", "selected boundary is absent"))?;
    let locked_profiles = index(&lock["profiles"], "id")?;
    let pinned = locked_profiles
        .get(text(&p["id"])?)
        .ok_or_else(|| invalid("LOCK003", "profile", "profile is not locked"))?;
    let profile_digest = profile_v2_digest(&profile)?;
    if pinned["file"] != boundary["profile"]
        || pinned["file"]["schema"] != "madb.profile/2"
        || pinned["semantic_digest"] != profile_digest
    {
        return Err(invalid(
            "LOCK003",
            "profile",
            "profile reference or semantic identity differs",
        ));
    }
    verify_reference(&pinned["file"], &context.profile)?;
    for key in ["route", "direction", "target"] {
        if boundary[key] != p[key] {
            return Err(invalid("LOCK001", key, "boundary and profile differ"));
        }
    }
    let tools = index(&lock["tools"], "id")?;
    for (id, tool) in index(&p["tools"], "id")? {
        if tools.get(id).copied() != Some(tool) || tool["target"] != p["target"] {
            return Err(invalid(
                "LOCK004",
                id,
                "profile tool tuple differs from lock or target",
            ));
        }
    }
    let locked_files = verify_frozen(lock, context.inputs)?;
    check_schemas(lock, context.inputs, context.schema_paths)?;
    if !locked_files.contains_key(text(&boundary["source"]["path"])?) {
        return Err(invalid(
            "LOCK005",
            "source",
            "authoritative source is not locked",
        ));
    }
    require_locked_file(&p["sdk"], &locked_files)?;
    frozen_reference(&p["sdk"], context.inputs)?;
    let reference = &p["composition"]["file"];
    let composition_bytes = frozen_reference(reference, context.inputs)?;
    require_locked_file(reference, &locked_files)?;
    let composition = registry.decode(DocumentKind::Composition, composition_bytes)?;
    let c = composition.value();
    // SDK input is upstream data, not this composition, its referring profile,
    // or a future generated output. Opaque SDK formats still need their own
    // adapter; this check does not claim to traverse arbitrary SDK references.
    if p["sdk"]["path"] == reference["path"]
        || p["sdk"]["sha256"] == reference["sha256"]
        || p["sdk"]["path"] == pinned["file"]["path"]
        || p["sdk"]["sha256"] == pinned["file"]["sha256"]
        || matches!(
            p["sdk"]["schema"].as_str(),
            Some("madb.profile/1" | "madb.profile/2" | "madb.composition/1")
        )
    {
        return Err(invalid(
            "InputInvalid",
            "sdk",
            "SDK input refers back to profile or composition",
        ));
    }
    if reference["schema"] != "madb.composition/1"
        || p["composition"]["id"] != c["id"]
        || p["composition"]["semantic_digest"] != composition_digest(&composition)?
    {
        return Err(invalid(
            "LOCK003",
            "composition",
            "composition reference or semantic identity differs",
        ));
    }
    if c["recipe"] != recipe.id.as_str()
        || c["selection"]["profile_id"] != p["id"]
        || ["route", "direction", "target"]
            .iter()
            .any(|key| c["selection"][key] != p[key])
    {
        return Err(invalid(
            "LOCK001",
            "selection",
            "composition differs from host recipe or profile",
        ));
    }
    let expected = &recipe.selection;
    let direction = match expected.direction {
        Direction::ConsumerToProvider => "consumer-to-provider",
        Direction::HostCallback => "host-callback",
        Direction::ReverseExport => "reverse-export",
    };
    if p["id"] != expected.profile_id.as_str()
        || p["route"] != expected.route.as_str()
        || p["direction"] != direction
        || p["target"] != expected.target.as_str()
    {
        return Err(invalid(
            "LOCK001",
            "selection",
            "selection is outside the host's fixed recipe",
        ));
    }
    let manifests = bind_packages(c, context.inputs, &locked_files)?;
    let components = bind_components(c, lock, recipe, &manifests)?;
    let assets = bind_assets(c, recipe, &manifests, &components)?;
    let mut all_paths = context
        .inputs
        .references()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let profile_path = logical(&pinned["file"]["path"])?;
    if let Some((_, bytes)) = context
        .inputs
        .files()
        .find(|(file, _)| file.path == profile_path)
    {
        if bytes != context.profile.bytes {
            return Err(invalid(
                "FILE004",
                profile_path.as_str(),
                "frozen profile input differs from selected profile bytes",
            ));
        }
    } else {
        all_paths.push(profile_path);
    }
    for asset in c["assets"].as_array().expect("checked assets") {
        if asset["source"]["kind"] == "generated" {
            for path in asset["source"]["paths"].as_array().expect("checked paths") {
                all_paths.push(logical(path)?);
            }
        }
    }
    super::ownership::check_output_paths(&all_paths)?;
    check_dependencies(c, &assets, &components, &recipe.dependencies)?;
    let responsibilities = resolve_responsibilities(c, context.boundary_id, &assets, &components)?;
    Ok(BoundCompositionInputs {
        locked_document: context.lock.clone(),
        profile_reference: file_reference(&pinned["file"])?,
        frozen_inputs: context.inputs.clone(),
        composition,
        profile,
        profile_digest,
        source_digest: source_digest(&lock["inputs"])?,
        namespace: text(&project["sdk_namespace"])?.into(),
        boundary: context.boundary_id.into(),
        responsibilities,
    })
}

/// Only snapshot/input identity correspondence. Type, shape, policy, SDK
/// exports and actual recipe coverage still need their own semantic checks.
pub fn check_composition_snapshot_binding(
    inputs: &BoundCompositionInputs,
    snapshot: &Document,
) -> Result<()> {
    if snapshot.kind() != DocumentKind::ContractIr {
        return Err(invalid("FMT003", "snapshot", "expected ContractIR"));
    }
    let s = snapshot.value();
    let p = inputs.profile.value();
    if s["sdk_namespace"] != inputs.namespace
        || s["boundary_id"] != inputs.boundary
        || s["selector"]["profile_id"] != p["id"]
        || s["selector"]["profile_digest"] != inputs.profile_digest
        || ["route", "target", "direction"]
            .iter()
            .any(|key| s["selector"][key] != p[key])
    {
        return Err(invalid(
            "LOCK001",
            "snapshot",
            "snapshot selector differs from bound composition",
        ));
    }
    if s["source_digest"] != inputs.source_digest {
        return Err(invalid(
            "LOCK005",
            "source_digest",
            "snapshot does not bind the whole locked source inventory",
        ));
    }
    let symbols = index(&s["symbols"], "public_id")?;
    for r in &inputs.responsibilities {
        for subject in [&r.key.subject, &r.key.scope.anchor] {
            if let super::ResolvedSubject::PublicSymbol(id) = subject
                && !symbols.contains_key(id.as_str())
            {
                return Err(invalid(
                    "InputInvalid",
                    id.as_str(),
                    "responsibility names an absent IR symbol",
                ));
            }
        }
    }
    Ok(())
}

fn check_features(p: &Value) -> Result<()> {
    let required = strings(&p["required_features"])?;
    let supported = strings(&p["supported_features"])?;
    if !required.contains(EXTENSION_COMPOSITION)
        || !supported.contains(EXTENSION_COMPOSITION)
        || required.iter().any(|f| {
            !supported.contains(f) || (*f != EXTENSION_COMPOSITION && !CORE_FEATURES.contains(f))
        })
    {
        return Err(invalid(
            "CapabilityUnsupported",
            "required_features",
            "unknown or unsupported composition profile feature",
        ));
    }
    Ok(())
}
fn check_schemas(
    lock: &Value,
    frozen: &FrozenInputs,
    paths: &CompositionSchemaPaths,
) -> Result<()> {
    macro_rules! bundled {
        ($name:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/schemas/",
                $name,
                ".schema.json"
            ))
            .as_slice()
        };
    }
    let schemas = index(&lock["schemas"], "path")?;
    let expected = [
        (&paths.common, bundled!("common")),
        (&paths.project, bundled!("project")),
        (&paths.lock, bundled!("lock")),
        (&paths.profile, bundled!("profile-v2")),
        (&paths.composition, bundled!("composition")),
        (&paths.extension, bundled!("extension-manifest")),
        (&paths.contract_ir, bundled!("contract-ir")),
    ];
    let mut seen = BTreeSet::new();
    for (path, bytes) in expected {
        if !seen.insert(path) {
            return Err(invalid(
                "LOCK006",
                path.as_str(),
                "schema locations must be distinct",
            ));
        }
        let file = schemas.get(path.as_str()).ok_or_else(|| {
            invalid(
                "LOCK005",
                path.as_str(),
                "used bundled schema is not locked",
            )
        })?;
        if frozen_reference(file, frozen)? != bytes {
            return Err(invalid(
                "FILE004",
                path.as_str(),
                "locked schema differs from the actual bundled decoder",
            ));
        }
    }
    Ok(())
}
fn verify_frozen<'a>(
    lock: &'a Value,
    frozen: &FrozenInputs,
) -> Result<BTreeMap<&'a str, &'a Value>> {
    let inputs = index(&lock["inputs"], "path")?;
    let mut all = inputs.clone();
    for (path, file) in index(&lock["schemas"], "path")? {
        if all.insert(path, file).is_some() {
            return Err(invalid("LOCK006", path, "duplicate inputs/schema path"));
        }
    }
    let refs = all
        .values()
        .map(|f| file_reference(f))
        .collect::<Result<Vec<_>>>()?;
    check_input_inventory(&refs).map_err(|e| invalid("LOCK006", "inputs", e.to_string()))?;
    if frozen.references().count() != all.len() {
        return Err(invalid(
            "FILE005",
            "inputs",
            "frozen inventory differs from lock",
        ));
    }
    for file in all.values() {
        frozen_reference(file, frozen)?;
    }
    Ok(inputs)
}
fn frozen_reference<'a>(file: &Value, frozen: &'a FrozenInputs) -> Result<&'a [u8]> {
    let path = text(&file["path"])?;
    let (_, bytes) = frozen
        .files()
        .find(|(r, _)| r.path.as_str() == path)
        .ok_or_else(|| invalid("FILE003", path, "frozen input is missing"))?;
    verify_reference(file, &ProvidedFile { path, bytes })?;
    Ok(bytes)
}
fn require_locked_file(file: &Value, inputs: &BTreeMap<&str, &Value>) -> Result<()> {
    let path = text(&file["path"])?;
    let locked = inputs
        .get(path)
        .ok_or_else(|| invalid("LOCK005", path, "input is not locked"))?;
    if ["path", "size", "sha256"]
        .iter()
        .any(|key| file[key] != locked[key])
    {
        return Err(invalid("LOCK005", path, "locked input identity differs"));
    }
    Ok(())
}
fn bind_packages(
    c: &Value,
    frozen: &FrozenInputs,
    locked: &BTreeMap<&str, &Value>,
) -> Result<BTreeMap<String, ExtensionManifest>> {
    let mut manifests = BTreeMap::new();
    for (id, package) in index(&c["packages"], "id")? {
        let files = index(&package["source"]["files"], "path")?;
        if package["source_digest"] != source_digest(&package["source"]["files"])? {
            return Err(invalid(
                "ExtensionIdentityMismatch",
                id,
                "package source digest differs",
            ));
        }
        let local_refs = files
            .values()
            .map(|f| file_reference(f))
            .collect::<Result<Vec<_>>>()?;
        check_input_inventory(&local_refs)
            .map_err(|e| invalid("InputInvalid", id, e.to_string()))?;
        for file in files.values() {
            let mapped = qualified(package, file)?;
            require_locked_file(&mapped, locked)?;
            frozen_reference(&mapped, frozen)?;
        }
        let manifest_ref = &package["manifest"];
        let local = files
            .get(text(&manifest_ref["path"])?)
            .ok_or_else(|| invalid("FILE003", id, "manifest is not in package sources"))?;
        if manifest_ref["schema"] != "madb.extension/1"
            || ["sha256", "size"]
                .iter()
                .any(|key| manifest_ref[key] != local[key])
        {
            return Err(invalid(
                "ExtensionIdentityMismatch",
                id,
                "manifest source reference differs",
            ));
        }
        let manifest = ExtensionManifest::decode(frozen_reference(
            &qualified(package, manifest_ref)?,
            frozen,
        )?)?;
        if manifest.id() != id || package["version"] != manifest.version() {
            return Err(invalid(
                "ExtensionIdentityMismatch",
                id,
                "package and manifest identity differ",
            ));
        }
        manifests.insert(id.into(), manifest);
    }
    validate_registration(&manifests.values().cloned().collect::<Vec<_>>())?;
    // Logical external dependency names do not identify a checked executable.
    // This finite /2 entry has no host mapping for them; matching a tool name
    // in the lock cannot establish that missing mapping or its provenance.
    for (id, manifest) in &manifests {
        for dependency in manifest.value()["dependencies"]
            .as_array()
            .expect("checked dependencies")
        {
            if dependency["kind"] != "extension" {
                return Err(invalid(
                    "CapabilityUnsupported",
                    &format!("{id}/dependencies/{}", text(&dependency["id"])?),
                    format!(
                        "host mapping is not registered for {} dependency",
                        text(&dependency["kind"])?
                    ),
                ));
            }
        }
    }
    Ok(manifests)
}
fn bind_components(
    c: &Value,
    lock: &Value,
    recipe: &RecipeRequirements,
    manifests: &BTreeMap<String, ExtensionManifest>,
) -> Result<BTreeMap<String, Digest>> {
    let selected = index(&c["components"], "id")?;
    let pinned = index(&lock["components"], "id")?;
    let packages = index(&c["packages"], "id")?;
    let mut required = BTreeMap::new();
    for r in &recipe.components {
        if required.insert(r.id.as_str(), r).is_some() {
            return Err(invalid(
                "InputInvalid",
                "recipe",
                "duplicate recipe component",
            ));
        }
    }
    if required.keys().ne(selected.keys()) {
        return Err(invalid(
            "InputInvalid",
            "components",
            "selected component closure differs from host recipe",
        ));
    }
    let mut result = BTreeMap::new();
    for (id, component) in selected {
        let (package_id, local) = id
            .split_once('/')
            .ok_or_else(|| invalid("InputInvalid", id, "expected qualified component"))?;
        let package = packages
            .get(package_id)
            .ok_or_else(|| invalid("InputInvalid", id, "component package is absent"))?;
        let manifest = &manifests[package_id];
        let declared = index(&manifest.value()["components"], "id")?;
        let m = declared
            .get(local)
            .ok_or_else(|| invalid("InputInvalid", id, "component is not exported"))?;
        if m["port"] != required[id].port.as_str() {
            return Err(invalid(
                "InputInvalid",
                id,
                "component port differs from host recipe",
            ));
        }
        let interprets = required[id].interprets_composition
            || matches!(text(&m["port"])?, "contract-reader" | "generator");
        if interprets && !strings(&m["supported_features"])?.contains(EXTENSION_COMPOSITION) {
            return Err(invalid(
                "CapabilityUnsupported",
                id,
                "component omits mandatory composition semantics",
            ));
        }
        if component["version"] != package["version"]
            || component["sha256"] != package["source_digest"]
            || pinned.get(id).copied() != Some(component)
        {
            return Err(invalid(
                "LOCK004",
                id,
                "component package identity is not exactly locked",
            ));
        }
        if !index(&package["source"]["files"], "path")?
            .contains_key(text(&m["entrypoint"]["path"])?)
        {
            return Err(invalid(
                "FILE003",
                id,
                "component entrypoint is absent from package sources",
            ));
        }
        result.insert(id.into(), digest(&component["sha256"])?);
    }
    Ok(result)
}

pub(super) fn index<'a>(value: &'a Value, key: &str) -> Result<BTreeMap<&'a str, &'a Value>> {
    let mut result = BTreeMap::new();
    for row in value
        .as_array()
        .ok_or_else(|| invalid("DTO001", key, "expected array"))?
    {
        let id = text(&row[key])?;
        if result.insert(id, row).is_some() {
            return Err(invalid("LOCK006", id, "duplicate logical identity"));
        }
    }
    Ok(result)
}
fn strings(value: &Value) -> Result<BTreeSet<&str>> {
    let mut result = BTreeSet::new();
    for v in value
        .as_array()
        .ok_or_else(|| invalid("DTO001", "", "expected string array"))?
    {
        if !result.insert(text(v)?) {
            return Err(invalid("InputInvalid", "", "duplicate string identity"));
        }
    }
    Ok(result)
}
pub(super) fn logical(v: &Value) -> Result<LogicalPath> {
    LogicalPath::new(text(v)?).map_err(|e| invalid("InputInvalid", "path", e.to_string()))
}
pub(super) fn digest(v: &Value) -> Result<Digest> {
    Digest::new(text(v)?).map_err(|e| invalid("InputInvalid", "digest", e.to_string()))
}
fn file_reference(v: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: logical(&v["path"])?,
        digest: digest(&v["sha256"])?,
        size: v["size"]
            .as_u64()
            .ok_or_else(|| invalid("DTO001", "size", "expected file size"))?,
    })
}
fn qualified(package: &Value, file: &Value) -> Result<Value> {
    let path = format!(
        "packages/{}/{}/{}",
        text(&package["id"])?,
        text(&package["version"])?,
        logical(&file["path"])?.as_str()
    );
    LogicalPath::new(&path).map_err(|e| invalid("InputInvalid", "package path", e.to_string()))?;
    let mut mapped = file.clone();
    mapped["path"] = Value::String(path);
    Ok(mapped)
}

use super::ownership::{bind_assets, check_dependencies};
