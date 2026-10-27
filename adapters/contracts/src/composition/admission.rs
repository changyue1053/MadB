//! Explicit /2 execution-image admission. Package and image hashes stay distinct.
//! This binds captured bytes and a host-selected recipe; it does not authenticate
//! remote publishers or grant final commit authority.
use super::{BoundCompositionInputs, Result, invalid, pins::index, text};
use crate::{
    artifacts::ProvidedFile,
    execution::{ExecutionRecipe, ExecutionTool},
    extension::ExtensionManifest,
};
use madb_app::{
    Digest,
    ports::FileReference,
    process::{MAX_TOOL_BYTES, ProcessPolicy, ProcessRequest},
};
use madb_model::contract::{CORE_FEATURES, EXTENSION_COMPOSITION};
use madb_protocol::Value;

pub struct CompositionImageRequest<'a> {
    pub entrypoint_source: &'a str,
    pub component_id: &'a str,
    pub recipe: &'a ExecutionRecipe,
    pub installed_image: ProvidedFile<'a>,
    /// Observations are supplied by the host after selecting the local source.
    /// Source text is an audit locator, never a replacement for captured bytes.
    pub observed_host: &'a str,
    pub observed_source: &'a str,
    pub required_features: &'a [&'a str],
    pub policy: &'a ProcessPolicy,
}

pub struct CheckedCompositionPins {
    manifest: ExtensionManifest,
    component_id: String,
    profile_digest: String,
    process: ProcessRequest,
    execution_tool: ExecutionTool,
    features: Vec<String>,
}
impl CheckedCompositionPins {
    pub fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }
    pub fn component_id(&self) -> &str {
        &self.component_id
    }
    pub fn process(&self) -> &ProcessRequest {
        &self.process
    }
    pub fn required_features(&self) -> &[String] {
        &self.features
    }
    pub(super) fn matches(
        &self,
        inputs: &BoundCompositionInputs,
        recipe: &ExecutionRecipe,
        producer: &str,
    ) -> bool {
        self.profile_digest == inputs.profile_digest()
            && producer == format!("{}/{}", self.manifest.id(), self.component_id)
            && self.execution_tool.id == recipe.tool.id
            && self.execution_tool.version == recipe.tool.version
            && self.execution_tool.digest == recipe.tool.digest
            && self.process.tool == recipe.installed_tool
            && self.process.arguments == recipe.arguments
            && self.process.environment == recipe.environment
            && self.process.working_directory == recipe.working_directory
    }
}

pub fn check_composition_image(
    inputs: &BoundCompositionInputs,
    request: CompositionImageRequest<'_>,
) -> Result<CheckedCompositionPins> {
    let recipe = request.recipe;
    let selected = index(&inputs.composition().value()["components"], "id")?;
    let component = selected.get(request.component_id).ok_or_else(|| {
        invalid(
            "ExtensionIdentityMismatch",
            "component",
            "component is not selected",
        )
    })?;
    let (package_id, local_id) = request
        .component_id
        .split_once('/')
        .ok_or_else(|| invalid("InputInvalid", "component", "expected qualified component"))?;
    let packages = index(&inputs.composition().value()["packages"], "id")?;
    let package = packages
        .get(package_id)
        .ok_or_else(|| invalid("InputInvalid", "package", "component package is absent"))?;
    if component["sha256"] != package["source_digest"] || component["version"] != package["version"]
    {
        return Err(invalid(
            "ExtensionIdentityMismatch",
            "component",
            "component must identify its source package",
        ));
    }
    let prefix = format!("packages/{package_id}/{}/", text(&package["version"])?);
    let source_bytes = |path: &str| -> Result<&[u8]> {
        let qualified = format!("{prefix}{path}");
        inputs
            .frozen_inputs
            .files()
            .find(|(f, _)| f.path.as_str() == qualified)
            .map(|(_, b)| b)
            .ok_or_else(|| {
                invalid(
                    "InputInvalid",
                    path,
                    "package file absent from captured source",
                )
            })
    };
    let manifest = ExtensionManifest::decode(source_bytes(text(&package["manifest"]["path"])?)?)?;
    if manifest.value()["example"] == true || manifest.id() != package_id {
        return Err(invalid(
            "ExtensionIdentityMismatch",
            "manifest",
            "design or different package cannot execute",
        ));
    }
    let components = index(&manifest.value()["components"], "id")?;
    let declared = components.get(local_id).ok_or_else(|| {
        invalid(
            "ExtensionIdentityMismatch",
            "component",
            "component absent from manifest",
        )
    })?;
    let features = declared["supported_features"]
        .as_array()
        .expect("checked features");
    let mut required = std::collections::BTreeSet::new();
    for feature in request.required_features {
        if !required.insert(*feature)
            || (!CORE_FEATURES.contains(feature) && *feature != EXTENSION_COMPOSITION)
            || !features.iter().any(|f| f.as_str() == Some(feature))
        {
            return Err(invalid(
                "CapabilityUnsupported",
                "features",
                "unknown, duplicate or unsupported mandatory feature",
            ));
        }
    }
    if matches!(
        declared["port"].as_str(),
        Some("contract-reader" | "generator")
    ) && !required.contains(EXTENSION_COMPOSITION)
    {
        return Err(invalid(
            "CapabilityUnsupported",
            "features",
            "composition interpreter requires explicit composition semantics",
        ));
    }
    let tools = index(&inputs.profile().value()["tools"], "id")?;
    let tool = tools
        .get(recipe.tool.id.as_str())
        .ok_or_else(|| invalid("LOCK004", "tool", "selected execution tool is absent"))?;
    let locked = index(&inputs.locked_document.value()["tools"], "id")?;
    if locked.get(recipe.tool.id.as_str()).copied() != Some(*tool)
        || tool["version"] != recipe.tool.version
        || tool["sha256"] != recipe.tool.digest.as_str()
        || tool["host"] != request.observed_host
        || tool["source"] != request.observed_source
        || tool["target"] != inputs.profile().value()["target"]
        || tool["arguments"] != serde_json::to_value(&recipe.arguments).expect("string arguments")
    {
        return Err(invalid(
            "LOCK004",
            "tool",
            "host, source, target or exact execution tool tuple differs",
        ));
    }
    let entry = text(&declared["entrypoint"]["path"])?;
    if entry != request.entrypoint_source
        || request.installed_image.path != recipe.installed_tool.path.as_str()
        || declared["entrypoint"]["args"] != tool["arguments"]
        || recipe.arguments.iter().any(|a| a.contains('\0'))
    {
        return Err(invalid(
            "ExtensionIdentityMismatch",
            "entrypoint",
            "installed path or arguments differ from selected component",
        ));
    }
    let image = request.installed_image.bytes;
    if image.is_empty()
        || image.len() as u64 > MAX_TOOL_BYTES
        || recipe.installed_tool.size != image.len() as u64
        || recipe.installed_tool.digest != Digest::of_bytes(image)
        || recipe.tool.digest != recipe.installed_tool.digest
        || source_bytes(entry)?.is_empty()
    {
        return Err(invalid(
            "LOCK004",
            "image",
            "installed image differs from locked image or selected source entrypoint is empty",
        ));
    }
    if manifest.value()["execution"]["network"] != "none" {
        return Err(invalid(
            "CapabilityUnsupported",
            "network",
            "host does not grant network requests",
        ));
    }
    let environment = manifest.value()["execution"]["env"]
        .as_array()
        .expect("checked env");
    for name in environment {
        if !request.policy.allowed_environment.contains(text(name)?) {
            return Err(invalid(
                "InputInvalid",
                "environment",
                "manifest requests an ungranted environment name",
            ));
        }
    }
    if recipe.environment.iter().any(|(name, value)| {
        !environment.contains(&Value::String(name.clone())) || value.contains('\0')
    }) {
        return Err(invalid(
            "InputInvalid",
            "environment",
            "recipe uses undeclared or invalid environment",
        ));
    }
    if recipe.working_directory.as_str() != "work" {
        return Err(invalid(
            "InputInvalid",
            "working_directory",
            "session working directory must be work",
        ));
    }
    Ok(CheckedCompositionPins {
        manifest,
        component_id: local_id.into(),
        profile_digest: inputs.profile_digest().into(),
        execution_tool: recipe.tool.clone(),
        process: ProcessRequest {
            tool: FileReference {
                ..recipe.installed_tool.clone()
            },
            working_directory: recipe.working_directory.clone(),
            arguments: recipe.arguments.clone(),
            environment: recipe.environment.clone(),
            stdin: Vec::new(),
        },
        features: request
            .required_features
            .iter()
            .map(|s| (*s).into())
            .collect(),
    })
}
