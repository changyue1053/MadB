use super::invalid;
use crate::{Digest, Id, Result};
use madb_plan::project::{CheckedTemplate, ParameterValues, SelectedTemplate, resolve_parameters};

/// Lexical absolute path for the first POSIX initialization recipe. No IO or
/// symlink resolution occurs here. Host must bind the actual parent descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AbsolutePath(String);
impl AbsolutePath {
    pub fn new(value: &str) -> Result<Self> {
        if !value.starts_with('/') || value.chars().any(char::is_control) || value.len() > 4096 {
            return Err(invalid("output", "expected a bounded POSIX absolute path"));
        }
        let mut parts = Vec::new();
        for part in value.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(invalid("output", "path escapes its absolute root"));
                    }
                }
                part => parts.push(part),
            }
        }
        if parts.is_empty() {
            return Err(invalid(
                "output",
                "filesystem root is not a new project directory",
            ));
        }
        Ok(Self(format!("/{}", parts.join("/"))))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn basename(&self) -> &str {
        self.0.rsplit('/').next().expect("validated non-root path")
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum VerifyLevel {
    Structure,
    Build,
    Run,
}
impl VerifyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Build => "build",
            Self::Run => "run",
        }
    }
}
/// Unfrozen user input. It does not confer a selected template or write authority.
#[derive(Clone, Debug)]
pub struct InitRequest {
    pub template_selector: Option<String>,
    pub template_profile_selector: Option<Id>,
    pub parameter_values: ParameterValues,
    pub targets: Option<Vec<Id>>,
    pub verify: VerifyLevel,
    pub open: Option<Id>,
    pub dry_run: bool,
    pub offline: bool,
    pub output: AbsolutePath,
}
/// Complete projection input. Adapters encode it; app never carries JSON Value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRequest {
    pub template_selector: Option<String>,
    pub template_profile_selector: Option<Id>,
    pub parameter_values: ParameterValues,
    pub targets: Vec<Id>,
    pub verify: VerifyLevel,
    pub open: Option<Id>,
    pub dry_run: bool,
    pub offline: bool,
    pub output: AbsolutePath,
}
pub trait InitRequestDigest {
    fn digest(&self, projection: &ResolvedRequest) -> Result<Digest>;
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenInitRequest {
    projection: ResolvedRequest,
    digest: Digest,
    selected: SelectedTemplate,
    actual_madb_version: String,
}
impl FrozenInitRequest {
    pub fn projection(&self) -> &ResolvedRequest {
        &self.projection
    }
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
    pub fn selected(&self) -> &SelectedTemplate {
        &self.selected
    }
    pub fn actual_madb_version(&self) -> &str {
        &self.actual_madb_version
    }
}
/// Apply the existing approved selector/default rules and formal-version gate
/// before asking the protocol adapter for the sole request digest projection.
pub fn freeze_request<D: InitRequestDigest>(
    template: &CheckedTemplate,
    request: InitRequest,
    actual_madb_version: &str,
    digester: &D,
) -> Result<(FrozenInitRequest, SelectedTemplate)> {
    template
        .check_madb_version(actual_madb_version)
        .map_err(|e| invalid(e.subject, e.message))?;
    if let Some(selector) = &request.template_selector {
        let exact = format!("{}@{}", template.template().id, template.template().version);
        if selector != template.template().id.as_str() && selector != &exact {
            return Err(invalid(
                "template",
                "requested selector differs from the checked template",
            ));
        }
    }
    let selected = template
        .select(
            request.template_profile_selector.as_ref(),
            request.targets.as_deref(),
        )
        .map_err(|e| invalid(e.subject, e.message))?;
    if request.open.as_ref().is_some_and(|id| {
        !selected
            .template()
            .ide_entries
            .iter()
            .any(|entry| selected.profile().ide_entries.contains(&entry.id) && &entry.ide.id == id)
    }) {
        return Err(invalid(
            "open",
            "IDE entry is absent from the selected profile",
        ));
    }
    let parameters = resolve_parameters(
        &template.template().parameters,
        &request.parameter_values,
        request.output.basename(),
    )
    .map_err(|e| invalid(e.subject, e.message))?;
    let projection = ResolvedRequest {
        template_selector: request.template_selector,
        template_profile_selector: request.template_profile_selector,
        parameter_values: parameters,
        targets: selected.families().to_vec(),
        verify: request.verify,
        open: request.open,
        dry_run: request.dry_run,
        offline: request.offline,
        output: request.output,
    };
    selected
        .check_values_recipe_shape(&projection.parameter_values)
        .map_err(|e| invalid(e.subject, e.message))?;
    let check_level = match projection.verify {
        VerifyLevel::Structure => madb_plan::project::CheckLevel::Structure,
        VerifyLevel::Build => madb_plan::project::CheckLevel::Build,
        VerifyLevel::Run => madb_plan::project::CheckLevel::Run,
    };
    selected
        .required_checks(check_level)
        .map_err(|e| invalid(e.subject, e.message))?;
    let digest = digester.digest(&projection)?;
    Ok((
        FrozenInitRequest {
            projection,
            digest,
            selected: selected.clone(),
            actual_madb_version: actual_madb_version.to_owned(),
        },
        selected,
    ))
}
