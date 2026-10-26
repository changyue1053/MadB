//! Lexical template and selected-combination rules. Foreign document bytes and
//! target/SDK semantics remain contracts/adapters' responsibility.
use super::parameters::{
    Parameter, ParameterValue, ParameterValues, StringPattern, check_parameters,
    validate_identifier,
};
use super::template::*;
use super::{
    PathClaims, ProjectFile, ProjectModule, ProjectPlan, ProjectPlanError, ProjectProfile,
    ProjectTarget, select_project_profile,
};
use madb_model::{Id, LogicalPath, evidence::EvidenceLevel};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CheckLevel {
    Structure,
    Build,
    Run,
    Import,
    Debug,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationCheck {
    pub id: Id,
    pub level: CheckLevel,
    pub target: Id,
    pub task_root: Option<Id>,
    pub required: bool,
    pub minimum_evidence: EvidenceLevel,
    pub prerequisites: Vec<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredCheck {
    pub id: Id,
    pub target: Id,
    pub level: CheckLevel,
    pub minimum_evidence: EvidenceLevel,
}

pub(super) fn check_template(template: &Template) -> Result<(), ProjectPlanError> {
    template.madb_compatibility.validate()?;
    check_parameters(&template.parameters)?;
    check_inventory_paths(&template.files)?;
    let profiles = identities(&template.profiles, |p| &p.id, "profile")?;
    let modules = identities(&template.modules, |p| &p.id, "module")?;
    let boundaries = identities(&template.boundary_refs, |p| &p.id, "boundary reference")?;
    let assets = identities(&template.assets, |p| &p.id, "asset")?;
    let ides = identities(&template.ide_entries, |p| &p.id, "IDE entry")?;
    let tasks = identities(&template.task_roots, |p| &p.id, "task root")?;
    let checks = identities(&template.validation, |p| &p.id, "validation")?;
    let parameters = identities(&template.parameters, Parameter::id, "parameter")?;
    if profiles.is_empty() || modules.is_empty() || checks.is_empty() {
        return err(
            "template",
            "profiles, modules and validation must be nonempty",
        );
    }
    if !profiles.contains_key(&template.default_profile) {
        return err("default_profile", "unknown default profile");
    }
    for module in &template.modules {
        if let Some(path) = &module.directory {
            check_template_path(path)?;
        }
        references(&module.dependencies, &modules, "module dependencies")?;
    }
    // Global DAG identity is meaningful even when modules are alternatives.
    super::order_modules(
        template
            .modules
            .iter()
            .map(|m| (m.id.clone(), project_module(m)))
            .collect(),
    )?;
    let inventory: BTreeMap<_, _> = template.files.iter().map(|f| (&f.path, f)).collect();
    for boundary in &template.boundary_refs {
        check_file_reference(&boundary.profile.file, &inventory)?;
        if !matches!(
            boundary.profile.schema.as_str(),
            "madb.profile/1" | "madb.profile/2"
        ) {
            return err(
                boundary.id.as_str(),
                "boundary reference requires a supported profile schema",
            );
        }
    }
    for asset in &template.assets {
        if !modules.contains_key(&asset.module) {
            return err(asset.id.as_str(), "unknown asset module");
        }
        check_template_path(&asset.output)?;
        match (&asset.ownership, &asset.source, &asset.renderer) {
            (super::FileOwnership::LocalState, None, None) => {}
            (super::FileOwnership::LocalState, _, _) => {
                return err(
                    asset.id.as_str(),
                    "local-state assets cannot have source or renderer",
                );
            }
            (_, Some(source), Some(renderer)) => {
                check_file_reference(source, &inventory)?;
                check_renderer(renderer, &parameters)?;
            }
            _ => {
                return err(
                    asset.id.as_str(),
                    "materialized asset requires source and renderer",
                );
            }
        }
    }
    for ide in &template.ide_entries {
        if !modules.contains_key(&ide.module) {
            return err(ide.id.as_str(), "unknown IDE owner module");
        }
        check_template_path(&ide.path)?;
        check_short_text(&ide.ide.min_inclusive, "IDE minimum version")?;
        check_short_text(&ide.ide.max_exclusive, "IDE maximum version")?;
        identities(&ide.extensions, |e| &e.id, "IDE extension")?;
        for extension in &ide.extensions {
            check_short_text(&extension.version, "IDE extension version")?;
        }
        if ide
            .run_target
            .as_ref()
            .is_some_and(|id| !tasks.contains_key(id))
        {
            return err(ide.id.as_str(), "unknown IDE task root");
        }
    }
    let all_targets: BTreeSet<_> = template
        .profiles
        .iter()
        .flat_map(|p| p.targets.iter().map(|t| &t.id))
        .collect();
    for task in &template.task_roots {
        if task.targets.iter().any(|id| !all_targets.contains(id)) {
            return err(task.id.as_str(), "unknown task target identity");
        }
        if !modules.contains_key(&task.module) {
            return err(task.id.as_str(), "unknown task owner module");
        }
        references(&task.boundary_refs, &boundaries, "task boundaries")?;
        unique(&task.targets, "task targets")?;
    }
    for check in &template.validation {
        if !all_targets.contains(&check.target) {
            return err(check.id.as_str(), "unknown validation target identity");
        }
        references(&check.prerequisites, &checks, "validation prerequisites")?;
        if check
            .task_root
            .as_ref()
            .is_some_and(|id| !tasks.contains_key(id))
        {
            return err(check.id.as_str(), "unknown validation task root");
        }
    }
    for profile in &template.profiles {
        if profile.modules.is_empty() || profile.validation.is_empty() || profile.targets.is_empty()
        {
            return err(
                profile.id.as_str(),
                "profile modules, targets and validation must be nonempty",
            );
        }
        references(&profile.modules, &modules, "profile modules")?;
        references(&profile.boundary_refs, &boundaries, "profile boundaries")?;
        references(&profile.assets, &assets, "profile assets")?;
        references(&profile.ide_entries, &ides, "profile IDE entries")?;
        references(&profile.task_roots, &tasks, "profile task roots")?;
        references(&profile.validation, &checks, "profile validation")?;
        identities(&profile.targets, |t| &t.id, "profile target")?;
        for target in &profile.targets {
            subset(
                &target.boundary_refs,
                &profile.boundary_refs,
                "target boundary is outside selected profile",
            )?;
        }
        for requirements in [
            &profile.requirements.structure,
            &profile.requirements.build,
            &profile.requirements.run,
            &profile.requirements.open,
        ] {
            if requirements.hosts.len() > 4096 {
                return err("hosts", "host array exceeds item budget");
            }
            identities(&requirements.tools, |t| &t.id, "required tool")?;
            for tool in &requirements.tools {
                check_short_text(&tool.version, "tool version")?;
            }
            for host in &requirements.hosts {
                host.versions.validate()?;
            }
            subset(
                &requirements.ide_entries,
                &profile.ide_entries,
                "required IDE is outside selected profile",
            )?;
        }
    }
    Ok(())
}

pub fn check_template_path(path: &LogicalPath) -> Result<(), ProjectPlanError> {
    if !path.as_str().is_ascii() || path.as_str().split('/').count() > 32 {
        return err(
            path.as_str(),
            "template path requires ASCII and at most 32 components",
        );
    }
    Ok(())
}
/// This also checks unselected files. Include the manifest when validating the
/// actual package so it counts toward the same complete-package budgets.
pub fn check_inventory_paths(files: &[FileIdentity]) -> Result<(), ProjectPlanError> {
    if files.len() > MAX_TEMPLATE_FILES {
        return err("files", "template file count exceeds budget");
    }
    let mut claims = PathClaims::default();
    let mut directories = BTreeSet::new();
    let mut total = 0_u64;
    let mut previous: Option<&LogicalPath> = None;
    for file in files {
        check_template_path(&file.path)?;
        if previous.is_some_and(|p| p >= &file.path) {
            return err(
                file.path.as_str(),
                "template inventory must be sorted and unique by ASCII path",
            );
        }
        previous = Some(&file.path);
        if file.size > MAX_TEMPLATE_FILE_BYTES {
            return err(file.path.as_str(), "template file exceeds byte budget");
        }
        total = total.checked_add(file.size).ok_or_else(|| {
            ProjectPlanError::new("files", "template content exceeds byte budget")
        })?;
        if total > MAX_TEMPLATE_CONTENT_BYTES {
            return err("files", "template content exceeds byte budget");
        }
        claims.add(&file.path, true)?;
        for (index, _) in file.path.as_str().match_indices('/') {
            directories.insert(&file.path.as_str()[..index]);
        }
        if directories.len() > MAX_TEMPLATE_DIRECTORIES {
            return err("files", "template parent directory count exceeds budget");
        }
    }
    Ok(())
}
fn check_file_reference(
    file: &FileIdentity,
    inventory: &BTreeMap<&LogicalPath, &FileIdentity>,
) -> Result<(), ProjectPlanError> {
    if inventory.get(&file.path).copied() != Some(file) {
        return err(
            file.path.as_str(),
            "file reference does not match complete inventory identity",
        );
    }
    Ok(())
}
fn check_renderer(
    renderer: &Renderer,
    parameters: &BTreeMap<&Id, &Parameter>,
) -> Result<(), ProjectPlanError> {
    match renderer {
        Renderer::Copy | Renderer::Unknown { .. } => Ok(()),
        Renderer::JsonValues { bindings } => {
            if bindings.len() > 4096 {
                return err("bindings", "binding array exceeds item budget");
            }
            let mut pointers = BTreeSet::new();
            for binding in bindings {
                if !parameters.contains_key(&binding.parameter) {
                    return err(
                        binding.parameter.as_str(),
                        "renderer references unknown parameter",
                    );
                }
                // Source-node type and array-index semantics require source bytes.
                let pointer = pointer_tokens(&binding.pointer)?;
                for previous in &pointers {
                    let previous: &Vec<String> = previous;
                    if pointer.starts_with(previous) || previous.starts_with(&pointer) {
                        return err(&binding.pointer, "JSON bindings overlap");
                    }
                }
                pointers.insert(pointer);
            }
            Ok(())
        }
        Renderer::Identifiers {
            parameters: references,
            ..
        } => {
            if references.is_empty() {
                return err("renderer", "identifier renderer parameters are empty");
            }
            unique(references, "renderer parameters")?;
            for id in references {
                if !matches!(
                    parameters.get(id),
                    Some(Parameter::String {
                        pattern: Some(StringPattern::Identifier),
                        ..
                    })
                ) {
                    return err(
                        id.as_str(),
                        "identifier renderer requires a constrained string parameter",
                    );
                }
            }
            Ok(())
        }
    }
}
/// Decode RFC 6901 escapes without using a JSON parser or guessing array shape.
pub fn pointer_tokens(pointer: &str) -> Result<Vec<String>, ProjectPlanError> {
    if pointer.is_empty() {
        return Ok(vec![]);
    }
    if !pointer.starts_with('/') {
        return err(pointer, "JSON pointer must be empty or start with slash");
    }
    pointer[1..]
        .split('/')
        .map(|token| {
            let mut out = String::new();
            let mut chars = token.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    match chars.next() {
                        Some('0') => out.push('~'),
                        Some('1') => out.push('/'),
                        _ => return err(pointer, "invalid JSON pointer escape"),
                    }
                } else {
                    out.push(ch);
                }
            }
            Ok(out)
        })
        .collect()
}

pub(super) fn select_template(
    template: &CheckedTemplate,
    profile: Option<&Id>,
    targets: Option<&[Id]>,
) -> Result<SelectedTemplate, ProjectPlanError> {
    let declaration = template.template();
    let projections: Vec<_> = declaration
        .profiles
        .iter()
        .map(|profile| ProjectProfile {
            id: profile.id.clone(),
            targets: profile
                .targets
                .iter()
                .map(|t| ProjectTarget {
                    id: t.id.clone(),
                    family: t.family.clone(),
                })
                .collect(),
            modules: declaration
                .modules
                .iter()
                .filter(|m| profile.modules.contains(&m.id))
                .map(project_module)
                .collect(),
            files: declaration
                .assets
                .iter()
                .filter(|a| profile.assets.contains(&a.id))
                .map(|a| ProjectFile {
                    module: a.module.clone(),
                    path: a.output.clone(),
                    ownership: a.ownership,
                })
                .collect(),
        })
        .collect();
    let selected = select_project_profile(
        &projections,
        Some(&declaration.default_profile),
        profile,
        targets,
    )?;
    let profile = declaration
        .profiles
        .iter()
        .find(|p| &p.id == selected.profile_id())
        .expect("selected declared identity");
    let selected_checks = check_profile_closure(declaration, profile, selected.plan())?;
    Ok(SelectedTemplate::new(
        template.clone(),
        selected,
        selected_checks,
    ))
}
fn check_profile_closure(
    template: &Template,
    profile: &TemplateProfile,
    plan: &ProjectPlan,
) -> Result<Vec<ValidationCheck>, ProjectPlanError> {
    let targets: Vec<_> = profile.targets.iter().map(|t| t.id.clone()).collect();
    for task in template
        .task_roots
        .iter()
        .filter(|t| profile.task_roots.contains(&t.id))
    {
        if !profile.modules.contains(&task.module) {
            return err(task.id.as_str(), "task owner is outside selected modules");
        }
        subset(
            &task.boundary_refs,
            &profile.boundary_refs,
            "task boundary is outside selected profile",
        )?;
        subset(
            &task.targets,
            &targets,
            "task target is outside selected profile",
        )?;
    }
    for ide in template
        .ide_entries
        .iter()
        .filter(|i| profile.ide_entries.contains(&i.id))
    {
        if !profile.modules.contains(&ide.module) {
            return err(ide.id.as_str(), "IDE owner is outside selected modules");
        }
        if ide
            .run_target
            .as_ref()
            .is_some_and(|id| !profile.task_roots.contains(id))
        {
            return err(ide.id.as_str(), "IDE task is outside selected profile");
        }
        let path = ide.path.as_str();
        let owned = plan
            .modules()
            .iter()
            .find(|m| m.id == ide.module)
            .is_some_and(|m| {
                m.directory.as_ref().is_none_or(|d| {
                    path == d.as_str() || path.starts_with(&format!("{}/", d.as_str()))
                })
            });
        let planned = plan
            .files()
            .iter()
            .any(|f| f.path == ide.path || f.path.as_str().starts_with(&format!("{path}/")))
            || plan
                .modules()
                .iter()
                .filter_map(|m| m.directory.as_ref())
                .any(|d| d == &ide.path || d.as_str().starts_with(&format!("{path}/")));
        if !owned || !planned {
            return err(
                ide.id.as_str(),
                "IDE entry is not a planned file or directory in its owner",
            );
        }
    }
    let checks: Vec<_> = template
        .validation
        .iter()
        .filter(|c| profile.validation.contains(&c.id))
        .cloned()
        .collect();
    for check in &checks {
        if !targets.contains(&check.target) {
            return err(
                check.id.as_str(),
                "validation target is outside selected profile",
            );
        }
        if check
            .task_root
            .as_ref()
            .is_some_and(|id| !profile.task_roots.contains(id))
        {
            return err(
                check.id.as_str(),
                "validation task is outside selected profile",
            );
        }
        subset(
            &check.prerequisites,
            &profile.validation,
            "validation prerequisite is outside selected profile",
        )?;
        if let Some(task_id) = &check.task_root {
            let task = template
                .task_roots
                .iter()
                .find(|task| &task.id == task_id)
                .expect("checked task root");
            if !task.targets.contains(&check.target) {
                return err(
                    check.id.as_str(),
                    "validation target is outside its task root",
                );
            }
        }
    }
    order_validation(&checks)
}
pub fn order_validation(
    checks: &[ValidationCheck],
) -> Result<Vec<ValidationCheck>, ProjectPlanError> {
    let index = identities(checks, |c| &c.id, "validation")?;
    let mut remaining = BTreeMap::new();
    let mut dependents = BTreeMap::<&Id, Vec<&Id>>::new();
    for check in checks {
        references(&check.prerequisites, &index, "validation prerequisites")?;
        remaining.insert(&check.id, check.prerequisites.len());
        for prerequisite in &check.prerequisites {
            dependents.entry(prerequisite).or_default().push(&check.id);
        }
    }
    let mut ready: BTreeSet<_> = remaining
        .iter()
        .filter_map(|(&id, &n)| (n == 0).then_some(id))
        .collect();
    let mut ordered = Vec::new();
    while let Some(id) = ready.pop_first() {
        ordered.push((*index[id]).clone());
        if let Some(children) = dependents.get(id) {
            for child in children {
                let n = remaining.get_mut(child).expect("checked identity");
                *n -= 1;
                if *n == 0 {
                    ready.insert(child);
                }
            }
        }
    }
    if ordered.len() != checks.len() {
        return err("validation", "validation prerequisite cycle");
    }
    Ok(ordered)
}

impl SelectedTemplate {
    /// Finite shape check, deliberately separate from tool pins, host and SDK
    /// evidence and formal MadB version admission. This never authorizes a run.
    pub fn check_values_recipe_shape(
        &self,
        parameters: &ParameterValues,
    ) -> Result<(), ProjectPlanError> {
        let profile = self.profile();
        let template = self.template();
        if self.validation().iter().any(|check| {
            check.required && matches!(check.level, CheckLevel::Import | CheckLevel::Debug)
        }) {
            return err(
                profile.id.as_str(),
                "values recipe cannot execute required import or debug checks",
            );
        }
        if profile.targets.len() != 1
            || profile.targets[0].family.as_str() != "macos"
            || profile.targets[0].boundary_refs != profile.boundary_refs
        {
            return err(
                profile.id.as_str(),
                "values recipe requires one macOS target bound to its complete boundary set",
            );
        }

        for asset in template
            .assets
            .iter()
            .filter(|a| profile.assets.contains(&a.id))
        {
            match &asset.renderer {
                Some(Renderer::Unknown { id }) => {
                    return err(id.as_str(), "unknown renderer cannot execute");
                }
                Some(Renderer::Identifiers {
                    language: IdentifierLanguage::Unknown(id),
                    ..
                }) => return err(id.as_str(), "unknown identifier language cannot execute"),
                _ => {}
            }
        }
        let tasks: Vec<_> = template
            .task_roots
            .iter()
            .filter(|t| profile.task_roots.contains(&t.id))
            .collect();
        if tasks.is_empty() || tasks.iter().any(|t| t.recipe.as_str() != VALUES_RECIPE) {
            return err(
                profile.id.as_str(),
                "unknown or missing finite initialization recipe",
            );
        }
        if template.id.as_str() != "org.madb.template/rust-c-sdk"
            || profile.id.as_str() != "macos-arm64-values"
        {
            return err(
                profile.id.as_str(),
                "unsupported template/profile for values recipe",
            );
        }
        if !profile.ide_entries.is_empty() {
            return err(
                profile.id.as_str(),
                "values recipe has no supported IDE entries",
            );
        }
        for module in template
            .modules
            .iter()
            .filter(|m| profile.modules.contains(&m.id))
        {
            if !matches!(
                module.role.as_str(),
                "project" | "rust-library" | "c-consumer" | "generated-sdk"
            ) {
                return err(module.role.as_str(), "unknown module role cannot execute");
            }
        }
        if profile.boundary_refs.len() != 1 {
            return err(
                profile.id.as_str(),
                "values recipe requires one boundary reference",
            );
        }
        let boundary = template
            .boundary_refs
            .iter()
            .find(|b| b.id == profile.boundary_refs[0])
            .expect("checked reference");
        if boundary.profile.schema.as_str() != "madb.profile/1" {
            return err(
                boundary.id.as_str(),
                "values recipe supports profile/1 only",
            );
        }
        if template.parameters.len() != 2
            || !["name", "namespace"]
                .iter()
                .all(|name| template.parameters.iter().any(|p| p.id().as_str() == *name))
        {
            return err(
                profile.id.as_str(),
                "values recipe requires exactly name and namespace parameters",
            );
        }
        for declaration in &template.parameters {
            let Parameter::String {
                default, choices, ..
            } = declaration
            else {
                return err(
                    declaration.id().as_str(),
                    "values recipe identifiers must be string parameters",
                );
            };
            if let Some(default) = default {
                validate_identifier(default)?;
            }
            for choice in choices {
                validate_identifier(choice)?;
            }
            if let Some(value) = parameters.get(declaration.id()) {
                declaration.validate_value(value)?;
            }
        }
        for name in ["name", "namespace"] {
            let (id, value) = parameters
                .iter()
                .find(|(id, _)| id.as_str() == name)
                .ok_or_else(|| ProjectPlanError::new(name, "missing values recipe identifier"))?;
            if let ParameterValue::String(value) = value {
                validate_identifier(value)?;
            } else {
                return err(id.as_str(), "values recipe identifier is not a string");
            }
        }
        if parameters.len() != 2 {
            return err("parameters", "values recipe has unexpected parameters");
        }
        Ok(())
    }
    /// Independently derive immutable recipe minima, then add template-required
    /// checks. A template cannot erase or lower a built-in requirement.
    pub fn required_checks(
        &self,
        level: CheckLevel,
    ) -> Result<Vec<RequiredCheck>, ProjectPlanError> {
        if !matches!(
            level,
            CheckLevel::Structure | CheckLevel::Build | CheckLevel::Run
        ) {
            return err("verify", "unsupported initialization verification level");
        }
        let template = self.template();
        let profile = self.profile();
        if profile.task_roots.is_empty()
            || template
                .task_roots
                .iter()
                .filter(|t| profile.task_roots.contains(&t.id))
                .any(|t| t.recipe.as_str() != VALUES_RECIPE)
        {
            return err(
                profile.id.as_str(),
                "unknown recipe has no required check table",
            );
        }
        if self.validation().iter().any(|check| {
            check.required && matches!(check.level, CheckLevel::Import | CheckLevel::Debug)
        }) {
            return err(
                profile.id.as_str(),
                "values recipe cannot execute required import or debug checks",
            );
        }
        let mut required = BTreeMap::new();
        for target in &profile.targets {
            for (id, check_level, evidence) in [
                ("structure", CheckLevel::Structure, EvidenceLevel::Static),
                (
                    "native-generate",
                    CheckLevel::Build,
                    EvidenceLevel::Generated,
                ),
                ("c-link", CheckLevel::Build, EvidenceLevel::Linked),
                ("ordinary-run", CheckLevel::Run, EvidenceLevel::HostRun),
            ] {
                if check_level <= level {
                    let id = Id::new(id).expect("fixed identity");
                    required.insert(
                        (id.clone(), target.id.clone()),
                        RequiredCheck {
                            id,
                            target: target.id.clone(),
                            level: check_level,
                            minimum_evidence: evidence,
                        },
                    );
                }
            }
        }
        for built_in in required.values() {
            let declared = self
                .validation()
                .iter()
                .find(|check| check.id == built_in.id && check.target == built_in.target)
                .ok_or_else(|| {
                    ProjectPlanError::new(
                        built_in.id.as_str(),
                        "template omits a built-in required check",
                    )
                })?;
            if !declared.required
                || declared.level != built_in.level
                || declared.minimum_evidence < built_in.minimum_evidence
            {
                return err(
                    built_in.id.as_str(),
                    "template weakens a built-in required check",
                );
            }
        }
        for check in self
            .validation()
            .iter()
            .filter(|c| c.required && c.level <= level)
        {
            match required.entry((check.id.clone(), check.target.clone())) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(RequiredCheck {
                        id: check.id.clone(),
                        target: check.target.clone(),
                        level: check.level,
                        minimum_evidence: check.minimum_evidence,
                    });
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if entry.get().level != check.level {
                        return err(
                            check.id.as_str(),
                            "template changes a built-in required check level",
                        );
                    }
                    entry.get_mut().minimum_evidence =
                        entry.get().minimum_evidence.max(check.minimum_evidence);
                }
            }
        }
        // Required checks also require their declared prerequisite closure.
        let by_id: BTreeMap<_, _> = self.validation().iter().map(|c| (&c.id, c)).collect();
        let mut queue: Vec<_> = required
            .keys()
            .filter_map(|(id, _)| by_id.get(id).copied())
            .collect();
        let mut visited = BTreeSet::new();
        while let Some(check) = queue.pop() {
            if !visited.insert(&check.id) {
                continue;
            }
            for id in &check.prerequisites {
                let prior = by_id[id];
                if prior.level > level {
                    return err(
                        id.as_str(),
                        "required prerequisite exceeds requested verification level",
                    );
                }
                let key = (prior.id.clone(), prior.target.clone());
                required.entry(key).or_insert_with(|| RequiredCheck {
                    id: prior.id.clone(),
                    target: prior.target.clone(),
                    level: prior.level,
                    minimum_evidence: prior.minimum_evidence,
                });
                queue.push(prior);
            }
        }
        Ok(required.into_values().collect())
    }
}
/// Template tools add to independently derived profile/recipe tools; duplicate
/// ids agree in both opaque version and exact image digest or fail.
pub fn merge_tool_requirements(
    groups: &[&[ComponentIdentity]],
) -> Result<Vec<ComponentIdentity>, ProjectPlanError> {
    let mut result = BTreeMap::new();
    for group in groups {
        identities(group, |t| &t.id, "required tool")?;
        for tool in *group {
            check_short_text(&tool.version, "tool version")?;
            if let Some(previous) = result.insert(tool.id.clone(), tool.clone())
                && previous != *tool
            {
                return err(tool.id.as_str(), "conflicting required tool identity");
            }
        }
    }
    Ok(result.into_values().collect())
}
impl RequirementSet {
    /// Declared host alternatives only. The owning recipe adds its own host
    /// conditions independently; an empty list cannot erase those conditions.
    pub fn matches_declared_host(&self, os: &Id, arch: &Id, version: Version) -> bool {
        self.hosts.is_empty()
            || self
                .hosts
                .iter()
                .any(|h| &h.os == os && &h.arch == arch && h.versions.contains(version))
    }
}
fn project_module(module: &TemplateModule) -> ProjectModule {
    ProjectModule {
        id: module.id.clone(),
        directory: module.directory.clone(),
        dependencies: module.dependencies.clone(),
    }
}
fn check_short_text(value: &str, subject: &str) -> Result<(), ProjectPlanError> {
    if value.is_empty() || value.chars().count() > 128 {
        return err(subject, "version string length is out of range");
    }
    Ok(())
}
fn identities<'a, T>(
    values: &'a [T],
    id: impl Fn(&'a T) -> &'a Id,
    subject: &str,
) -> Result<BTreeMap<&'a Id, &'a T>, ProjectPlanError> {
    if values.len() > 4096 {
        return err(subject, "declaration array exceeds item budget");
    }
    let mut index = BTreeMap::new();
    for value in values {
        let key = id(value);
        if index.insert(key, value).is_some() {
            return err(key.as_str(), "duplicate declaration identity");
        }
    }
    Ok(index)
}
fn unique(values: &[Id], subject: &str) -> Result<(), ProjectPlanError> {
    if values.len() > 4096 {
        return err(subject, "reference array exceeds item budget");
    }
    let mut set = BTreeSet::new();
    for value in values {
        if !set.insert(value) {
            return err(subject, "duplicate reference identity");
        }
    }
    Ok(())
}
fn references<T>(
    values: &[Id],
    index: &BTreeMap<&Id, &T>,
    subject: &str,
) -> Result<(), ProjectPlanError> {
    unique(values, subject)?;
    for value in values {
        if !index.contains_key(value) {
            return err(value.as_str(), "unknown referenced identity");
        }
    }
    Ok(())
}
fn subset(values: &[Id], allowed: &[Id], message: &'static str) -> Result<(), ProjectPlanError> {
    unique(values, message)?;
    for value in values {
        if !allowed.contains(value) {
            return err(value.as_str(), message);
        }
    }
    Ok(())
}
fn err<T>(subject: impl Into<String>, message: &'static str) -> Result<T, ProjectPlanError> {
    Err(ProjectPlanError::new(subject, message))
}
