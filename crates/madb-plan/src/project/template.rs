//! Typed initialization declarations. These values carry no decoding, file IO or
//! authority to execute an unknown recipe. CheckedTemplate adds lexical rules.
use super::parameters::Parameter;
use super::validation::ValidationCheck;
use super::{FileOwnership, ProjectPlan, ProjectPlanError, SelectedProjectProfile};
use madb_model::{Digest, Id, LogicalPath};

pub const MAX_TEMPLATE_FILES: usize = 4096;
pub const MAX_TEMPLATE_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_TEMPLATE_CONTENT_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TEMPLATE_DIRECTORIES: usize = 8192;
pub const VALUES_RECIPE: &str = "madb.init.rust-c-sdk.values@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionRange {
    pub min_inclusive: Version,
    pub max_exclusive: Version,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    pub path: LogicalPath,
    pub sha256: Digest,
    pub size: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReference {
    pub file: FileIdentity,
    pub schema: Id,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentIdentity {
    pub id: Id,
    pub version: String,
    pub sha256: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template {
    pub id: Id,
    pub version: Version,
    pub madb_compatibility: VersionRange,
    pub default_profile: Id,
    pub parameters: Vec<Parameter>,
    pub profiles: Vec<TemplateProfile>,
    pub modules: Vec<TemplateModule>,
    pub boundary_refs: Vec<BoundaryRef>,
    pub assets: Vec<TemplateAsset>,
    pub ide_entries: Vec<IdeEntry>,
    pub task_roots: Vec<TemplateTaskRoot>,
    pub validation: Vec<ValidationCheck>,
    /// Complete package inventory except the externally pinned manifest itself.
    pub files: Vec<FileIdentity>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateModule {
    pub id: Id,
    pub role: Id,
    pub directory: Option<LogicalPath>,
    pub dependencies: Vec<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundaryRef {
    pub id: Id,
    pub profile: FileReference,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateTarget {
    pub id: Id,
    pub family: Id,
    pub boundary_refs: Vec<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateProfile {
    pub id: Id,
    pub targets: Vec<TemplateTarget>,
    pub modules: Vec<Id>,
    pub boundary_refs: Vec<Id>,
    pub assets: Vec<Id>,
    pub ide_entries: Vec<Id>,
    pub task_roots: Vec<Id>,
    pub validation: Vec<Id>,
    pub requirements: Requirements,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Requirements {
    pub structure: RequirementSet,
    pub build: RequirementSet,
    pub run: RequirementSet,
    pub open: RequirementSet,
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RequirementSet {
    pub hosts: Vec<HostRequirement>,
    pub tools: Vec<ComponentIdentity>,
    pub ide_entries: Vec<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostRequirement {
    pub os: Id,
    pub arch: Id,
    pub versions: VersionRange,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateAsset {
    pub id: Id,
    pub module: Id,
    pub source: Option<FileIdentity>,
    pub output: LogicalPath,
    pub ownership: FileOwnership,
    pub renderer: Option<Renderer>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Renderer {
    Copy,
    JsonValues {
        bindings: Vec<JsonBinding>,
    },
    Identifiers {
        language: IdentifierLanguage,
        parameters: Vec<Id>,
    },
    /// Identity survives decoding; it never implies copy semantics.
    Unknown {
        id: Id,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonBinding {
    pub parameter: Id,
    pub pointer: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentifierLanguage {
    C,
    Toml,
    Cmake,
    Unknown(Id),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeEntry {
    pub id: Id,
    pub module: Id,
    pub ide: IdeRequirement,
    pub path: LogicalPath,
    pub run_target: Option<Id>,
    pub extensions: Vec<IdeExtension>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeRequirement {
    pub id: Id,
    pub min_inclusive: String,
    pub max_exclusive: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeExtension {
    pub id: Id,
    pub version: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateTaskRoot {
    pub id: Id,
    pub module: Id,
    pub recipe: Id,
    pub boundary_refs: Vec<Id>,
    pub targets: Vec<Id>,
}

/// Semantic structure only: no SDK bytes, archive completeness or tool receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTemplate {
    template: Template,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedTemplate {
    template: CheckedTemplate,
    selection: SelectedProjectProfile,
    validation: Vec<ValidationCheck>,
}

impl Version {
    /// Finite formal version grammar; prerelease development builds fail.
    pub fn parse(value: &str) -> Result<Self, ProjectPlanError> {
        let parts: Vec<_> = value.split('.').collect();
        if parts.len() != 3 {
            return Err(ProjectPlanError::new(
                value,
                "expected a formal three-part version",
            ));
        }
        let mut numbers = [0_u16; 3];
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(ProjectPlanError::new(
                    value,
                    "expected a formal three-part version",
                ));
            }
            numbers[index] = part
                .parse()
                .map_err(|_| ProjectPlanError::new(value, "version component exceeds 65535"))?;
        }
        Ok(Self {
            major: numbers[0],
            minor: numbers[1],
            patch: numbers[2],
        })
    }
}
impl std::fmt::Display for Version {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
impl VersionRange {
    pub fn validate(&self) -> Result<(), ProjectPlanError> {
        if self.min_inclusive >= self.max_exclusive {
            return Err(ProjectPlanError::new(
                "version range",
                "version range is empty or reversed",
            ));
        }
        Ok(())
    }
    pub fn contains(&self, value: Version) -> bool {
        self.min_inclusive <= value && value < self.max_exclusive
    }
}
impl CheckedTemplate {
    pub fn new(template: Template) -> Result<Self, ProjectPlanError> {
        super::validation::check_template(&template)?;
        Ok(Self { template })
    }
    pub fn template(&self) -> &Template {
        &self.template
    }
    pub fn select(
        &self,
        profile: Option<&Id>,
        targets: Option<&[Id]>,
    ) -> Result<SelectedTemplate, ProjectPlanError> {
        super::validation::select_template(self, profile, targets)
    }
    pub fn check_madb_version(&self, actual: &str) -> Result<Version, ProjectPlanError> {
        let actual = Version::parse(actual)?;
        if !self.template.madb_compatibility.contains(actual) {
            return Err(ProjectPlanError::new(
                actual.to_string(),
                "MadB version is outside template compatibility",
            ));
        }
        Ok(actual)
    }
}
impl SelectedTemplate {
    pub(super) fn new(
        template: CheckedTemplate,
        selection: SelectedProjectProfile,
        validation: Vec<ValidationCheck>,
    ) -> Self {
        Self {
            template,
            selection,
            validation,
        }
    }
    pub fn template(&self) -> &Template {
        self.template.template()
    }
    pub fn profile(&self) -> &TemplateProfile {
        self.template()
            .profiles
            .iter()
            .find(|p| &p.id == self.selection.profile_id())
            .expect("checked selected identity")
    }
    pub fn profile_id(&self) -> &Id {
        self.selection.profile_id()
    }
    pub fn families(&self) -> &[Id] {
        self.selection.families()
    }
    pub fn plan(&self) -> &ProjectPlan {
        self.selection.plan()
    }
    /// The complete initial source plan includes immutable package provenance.
    /// This is only a lexical plan; callers must supply the exact bytes and
    /// verify the archive identity separately before any Host commit.
    pub fn plan_with_provenance(
        &self,
        manifest: &FileIdentity,
    ) -> Result<ProjectPlan, ProjectPlanError> {
        let mut package_files = self.template().files.clone();
        package_files.push(manifest.clone());
        package_files.sort_by(|left, right| left.path.cmp(&right.path));
        super::validation::check_inventory_paths(&package_files)?;
        let root = self
            .plan()
            .modules()
            .iter()
            .find(|module| module.directory.is_none())
            .ok_or_else(|| {
                ProjectPlanError::new(
                    "provenance",
                    "initial provenance requires a selected root module",
                )
            })?;
        let mut files = self.plan().files().to_vec();
        files.push(super::ProjectFile {
            module: root.id.clone(),
            path: LogicalPath::new(".madb/template-package.tar").expect("fixed path"),
            ownership: super::FileOwnership::Provenance,
        });
        for file in package_files {
            let output = LogicalPath::new(format!(".madb/template-input/{}", file.path.as_str()))
                .map_err(|_| {
                ProjectPlanError::new(
                    file.path.as_str(),
                    "provenance path exceeds logical path limit",
                )
            })?;
            // Output paths obey the same admitted template path subset.
            super::validation::check_template_path(&output)?;
            files.push(super::ProjectFile {
                module: root.id.clone(),
                path: output,
                ownership: super::FileOwnership::Provenance,
            });
        }
        ProjectPlan::new(self.plan().modules().to_vec(), files)
    }
    pub fn validation(&self) -> &[ValidationCheck] {
        &self.validation
    }
    /// Declared target identities are retained; SDK target meaning is not inferred.
    pub fn targets(&self) -> &[TemplateTarget] {
        &self.profile().targets
    }
}
