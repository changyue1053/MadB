use super::{ProjectInitAdapter, format_error, invalid, wire::*};
use madb_app::inputs::FrozenInputs;
use madb_app::{Digest, Id, Result};
use madb_plan::project::{FileIdentity, SelectedTemplate, Version};
use madb_protocol::{Document, DocumentKind, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Identity from the trusted template catalog, separate from archive self-description.
#[derive(Clone, Copy, Debug)]
pub struct CatalogTemplateIdentity<'a> {
    pub id: &'a Id,
    pub version: Version,
}

/// Cross-document byte binding only. This neither proves that an archive was
/// safely extracted nor observes any installed SDK/tool. The composition root
/// must additionally consume the template adapter's CheckedInventory proof.
#[derive(Clone, Debug)]
pub struct BoundTemplateInputs {
    profiles: Vec<Document>,
    manifest: FileIdentity,
    package: FileIdentity,
}
impl BoundTemplateInputs {
    pub fn profiles(&self) -> &[Document] {
        &self.profiles
    }
    pub fn manifest(&self) -> &FileIdentity {
        &self.manifest
    }
    pub fn package(&self) -> &FileIdentity {
        &self.package
    }
}
impl ProjectInitAdapter {
    /// Check actual immutable input bytes against the trusted catalog pin and
    /// template inventory. All nested references retain the one package root.
    pub fn check_template_inputs(
        &self,
        selected: &SelectedTemplate,
        manifest: &FileIdentity,
        package: &FileIdentity,
        package_bytes: &[u8],
        inputs: &FrozenInputs,
        catalog: CatalogTemplateIdentity<'_>,
    ) -> Result<BoundTemplateInputs> {
        if selected.template().id != *catalog.id || selected.template().version != catalog.version {
            return Err(invalid(
                "catalog",
                "manifest identity differs from the trusted catalog record",
            ));
        }
        if Digest::of_bytes(package_bytes) != package.sha256
            || package_bytes.len() as u64 != package.size
        {
            return Err(invalid(
                "package",
                "original archive bytes differ from the expected pin",
            ));
        }
        let actual: BTreeMap<_, _> = inputs
            .files()
            .map(|(r, b)| (r.path.as_str(), (r, b)))
            .collect();
        let mut expected = selected.template().files.clone();
        expected.push(manifest.clone());
        let expected_paths: BTreeSet<_> = expected.iter().map(|f| f.path.as_str()).collect();
        if expected_paths.len() != expected.len()
            || actual.len() != expected.len()
            || actual.keys().copied().collect::<BTreeSet<_>>() != expected_paths
        {
            return Err(invalid(
                "files",
                "captured package inventory differs from manifest plus all files",
            ));
        }
        for f in &expected {
            let (r, b) = actual[f.path.as_str()];
            if r.digest != f.sha256
                || r.size != f.size
                || Digest::of_bytes(b) != f.sha256
                || b.len() as u64 != f.size
            {
                return Err(invalid(
                    f.path.as_str(),
                    "captured bytes differ from original package inventory",
                ));
            }
        }
        let parsed = self.decode_template(actual[manifest.path.as_str()].1)?;
        if parsed.template() != selected.template() {
            return Err(invalid(
                "manifest",
                "selected model differs from the actual frozen manifest bytes",
            ));
        }
        let mut profiles = Vec::new();
        for boundary in selected
            .template()
            .boundary_refs
            .iter()
            .filter(|b| selected.profile().boundary_refs.contains(&b.id))
        {
            if boundary.profile.schema.as_str() != "madb.profile/1" {
                return Err(invalid(
                    boundary.id.as_str(),
                    "the first value initialization recipe does not execute profile/2",
                ));
            }
            let bytes = actual
                .get(boundary.profile.file.path.as_str())
                .ok_or_else(|| {
                    invalid(
                        boundary.id.as_str(),
                        "profile is missing from the original package root",
                    )
                })?
                .1;
            let profile = self
                .registry
                .decode(DocumentKind::Profile, bytes)
                .map_err(format_error)?;
            let p = profile.value();
            if p["route"] != "RUST-C-ABI"
                || p["provider"]["language"] != "rust"
                || p["provider"]["runtime"] != "native"
                || p["consumer"]["language"] != "c"
                || p["consumer"]["runtime"] != "native"
                || p["direction"] != "consumer-to-provider"
                || p["target"] != "aarch64-apple-darwin"
                || p["abi"] != "c11"
                || p["artifact_kind"] != "dynamic-library"
                || p["build_system"] != "rustc-clang"
                || p["deployment_target"] != "14.0"
                || !p["backend_config"].is_null()
            {
                return Err(invalid(
                    boundary.id.as_str(),
                    "profile differs from the finite Rust/C value recipe",
                ));
            }
            crate::capability(&profile).map_err(format_error)?;
            let sdk = reference(&p["sdk"])?;
            if sdk.schema.as_str() != "madb.native-toolchain/1" {
                return Err(invalid(
                    "sdk",
                    "unknown SDK descriptor has no finite validator",
                ));
            }
            let (r, bytes) = actual.get(sdk.file.path.as_str()).ok_or_else(|| {
                invalid(
                    "sdk",
                    "SDK reference is missing from the original package root",
                )
            })?;
            if r.digest != sdk.file.sha256 || r.size != sdk.file.size {
                return Err(invalid("sdk", "SDK FileRef differs from captured bytes"));
            }
            let value = madb_protocol::strict_json(bytes).map_err(format_error)?;
            check_native_toolchain(&value, p)?;
            profiles.push(profile);
        }
        if profiles.is_empty() {
            return Err(invalid(
                "boundary_refs",
                "first value recipe requires an actual boundary profile",
            ));
        }
        Ok(BoundTemplateInputs {
            profiles,
            manifest: manifest.clone(),
            package: package.clone(),
        })
    }
}
fn exact(v: &Value, keys: &[&str]) -> Result<()> {
    let actual = v
        .as_object()
        .ok_or_else(|| invalid("sdk", "expected descriptor object"))?;
    if actual.keys().map(String::as_str).collect::<BTreeSet<_>>() != keys.iter().copied().collect()
    {
        return Err(invalid("sdk", "unknown or missing native descriptor field"));
    }
    Ok(())
}
/// The already existing private native-toolchain/1 observation recipe. This
/// checks the published metadata closure; installed image/SDK bytes are later
/// Host observations and are deliberately not claimed here.
fn check_native_toolchain(s: &Value, p: &Value) -> Result<()> {
    exact(
        s,
        &[
            "schema",
            "target",
            "system",
            "tools",
            "sdk_inputs",
            "compiler",
        ],
    )?;
    if s["schema"] != "madb.native-toolchain/1"
        || s["target"] != p["target"]
        || s["system"] != "Darwin"
    {
        return Err(invalid("sdk", "native observation selector differs"));
    }
    let c = &s["compiler"];
    exact(
        c,
        &[
            "target",
            "language_standard",
            "warnings",
            "deployment_target",
        ],
    )?;
    if c["target"] != "arm64-apple-macosx14.0"
        || c["language_standard"] != "c11"
        || c["deployment_target"] != p["deployment_target"]
        || strings(&c["warnings"])? != ["-Wall", "-Wextra", "-Werror"]
    {
        return Err(invalid(
            "sdk.compiler",
            "compiler observation differs from finite recipe",
        ));
    }
    let mut names = BTreeSet::new();
    let mut tools = BTreeMap::new();
    for t in array(&s["tools"])? {
        exact(t, &["id", "version", "sha256", "size"])?;
        let name = id(&t["id"])?;
        let version = string(&t["version"])?;
        if version.is_empty()
            || version.len() > 128
            || uint(&t["size"])? > 9_007_199_254_740_991
            || tools
                .insert(name.clone(), (version.to_owned(), digest(&t["sha256"])?))
                .is_some()
        {
            return Err(invalid(
                "sdk.tools",
                "invalid or duplicate tool observation",
            ));
        }
    }
    if tools.is_empty() || tools.len() > 4096 {
        return Err(invalid(
            "sdk.tools",
            "tool observations are missing or over budget",
        ));
    }
    let profile_tools = array(&p["tools"])?;
    if tools.len() != profile_tools.len() {
        return Err(invalid(
            "sdk.tools",
            "SDK and profile tool identities differ",
        ));
    }
    for t in profile_tools {
        let key = id(&t["id"])?;
        if t["target"] != p["target"]
            || t["host"] != "aarch64-apple-darwin"
            || tools.get(&key) != Some(&(string(&t["version"])?.to_owned(), digest(&t["sha256"])?))
        {
            return Err(invalid(
                "sdk.tools",
                "SDK and profile tool image/version/target differ",
            ));
        }
    }
    let sdk_inputs = array(&s["sdk_inputs"])?;
    if sdk_inputs.is_empty() || sdk_inputs.len() > 4096 {
        return Err(invalid(
            "sdk.inputs",
            "SDK input closure is missing or over budget",
        ));
    }
    for f in sdk_inputs {
        exact(f, &["name", "sha256", "size"])?;
        let name = path(&f["name"])?;
        digest(&f["sha256"])?;
        if uint(&f["size"])? > 9_007_199_254_740_991 || !names.insert(name) {
            return Err(invalid(
                "sdk.inputs",
                "invalid or duplicate SDK input identity",
            ));
        }
    }
    Ok(())
}
