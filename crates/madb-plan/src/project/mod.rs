//! Pure module and file planning for I01. These are internal values, not a
//! template wire format or permission to write an existing workspace.
mod parameters;
mod template;
mod validation;
pub use parameters::*;
pub use template::*;
pub use validation::*;
mod selection;
pub use selection::{
    ProjectProfile, ProjectTarget, SelectedProjectProfile, select_project_profile,
};

use madb_model::{Id, LogicalPath};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectModule {
    pub id: Id,
    /// None denotes the project root. Paths are logical, never resolved here.
    pub directory: Option<LogicalPath>,
    pub dependencies: Vec<Id>,
}

/// The ownership classes from the initialization design. This classification
/// does not itself authorize updates, deletion, or a filesystem commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileOwnership {
    User,
    Managed,
    LocalState,
    Provenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFile {
    pub module: Id,
    /// A normalized path relative to the project root, including module prefix.
    pub path: LogicalPath,
    pub ownership: FileOwnership,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectPlanError {
    pub subject: String,
    pub message: &'static str,
}

impl ProjectPlanError {
    fn new(subject: impl Into<String>, message: &'static str) -> Self {
        Self {
            subject: subject.into(),
            message,
        }
    }
}

impl std::fmt::Display for ProjectPlanError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}: {}", self.subject, self.message)
    }
}
impl std::error::Error for ProjectPlanError {}

/// A deterministic, immutable module DAG and file-ownership plan. Validation
/// is lexical: host-side symlink checks, base revision checks, rendering,
/// no-overwrite commit and IDE execution are separate responsibilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectPlan {
    modules: Vec<ProjectModule>,
    files: Vec<ProjectFile>,
}

impl ProjectPlan {
    pub fn new(
        modules: Vec<ProjectModule>,
        mut files: Vec<ProjectFile>,
    ) -> Result<Self, ProjectPlanError> {
        if modules.is_empty() {
            return Err(ProjectPlanError::new("project", "project has no modules"));
        }
        let mut by_id = BTreeMap::new();
        for mut module in modules {
            module.dependencies.sort();
            let id = module.id.clone();
            if by_id.insert(id.clone(), module).is_some() {
                return Err(ProjectPlanError::new(
                    id.as_str(),
                    "duplicate module identity",
                ));
            }
        }
        let mut paths = PathClaims::default();
        let mut directories = BTreeMap::new();
        for module in by_id.values() {
            let directory = module.directory.as_ref().map_or("", LogicalPath::as_str);
            if directories.insert(directory, &module.id).is_some() {
                return Err(ProjectPlanError::new(
                    module.id.as_str(),
                    "multiple modules claim the same directory",
                ));
            }
            if let Some(directory) = &module.directory {
                paths.add(directory, false)?;
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        for file in &files {
            let module = by_id.get(&file.module).ok_or_else(|| {
                ProjectPlanError::new(file.path.as_str(), "file owner is not a project module")
            })?;
            if let Some(directory) = &module.directory {
                let prefix = format!("{}/", directory.as_str());
                if !file.path.as_str().starts_with(&prefix) {
                    return Err(ProjectPlanError::new(
                        file.path.as_str(),
                        "file is outside its owning module directory",
                    ));
                }
            }
            paths.add(&file.path, true)?;
        }
        let modules = order_modules(by_id)?;
        Ok(Self { modules, files })
    }

    pub fn modules(&self) -> &[ProjectModule] {
        &self.modules
    }

    pub fn files(&self) -> &[ProjectFile] {
        &self.files
    }
}

fn order_modules(
    modules: BTreeMap<Id, ProjectModule>,
) -> Result<Vec<ProjectModule>, ProjectPlanError> {
    let mut remaining = BTreeMap::new();
    let mut dependents = BTreeMap::<&Id, Vec<&Id>>::new();
    for module in modules.values() {
        let mut seen = BTreeSet::new();
        for dependency in &module.dependencies {
            if !modules.contains_key(dependency) {
                return Err(ProjectPlanError::new(
                    module.id.as_str(),
                    "missing module dependency",
                ));
            }
            if !seen.insert(dependency) {
                return Err(ProjectPlanError::new(
                    module.id.as_str(),
                    "duplicate module dependency",
                ));
            }
            dependents.entry(dependency).or_default().push(&module.id);
        }
        remaining.insert(&module.id, module.dependencies.len());
    }
    let mut ready: BTreeSet<&Id> = remaining
        .iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id))
        .collect();
    let mut ordered = Vec::with_capacity(modules.len());
    while let Some(id) = ready.pop_first() {
        ordered.push(modules[id].clone());
        if let Some(children) = dependents.get(id) {
            for child in children {
                let count = remaining.get_mut(child).expect("validated module identity");
                *count -= 1;
                if *count == 0 {
                    ready.insert(child);
                }
            }
        }
    }
    if ordered.len() != modules.len() {
        let blocked = remaining
            .iter()
            .find_map(|(id, &count)| (count != 0).then_some(id))
            .expect("a cyclic graph has a blocked module");
        return Err(ProjectPlanError::new(
            blocked.as_str(),
            "module dependency cycle",
        ));
    }
    Ok(ordered)
}

/// Index every prefix so distinct filenames cannot hide a directory case alias.
#[derive(Default)]
struct PathClaims(BTreeMap<String, (String, bool)>);

impl PathClaims {
    fn add(&mut self, path: &LogicalPath, file: bool) -> Result<(), ProjectPlanError> {
        let path = path.as_str();
        // Match the existing TaskGraph portable-output subset. A future
        // filesystem-specific adapter must not infer Unicode alias safety here.
        if !path.is_ascii() {
            return Err(ProjectPlanError::new(
                path,
                "portable project assets require ASCII logical paths",
            ));
        }
        for end in path.match_indices('/').map(|(i, _)| i).chain([path.len()]) {
            let prefix = &path[..end];
            let is_file = file && end == path.len();
            match self.0.entry(prefix.to_ascii_lowercase()) {
                std::collections::btree_map::Entry::Occupied(previous) => {
                    let (spelling, was_file) = previous.get();
                    if spelling != prefix {
                        return Err(ProjectPlanError::new(path, "case-aliased project path"));
                    }
                    if *was_file || is_file {
                        return Err(ProjectPlanError::new(
                            path,
                            "duplicate file or file/directory collision",
                        ));
                    }
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((prefix.to_owned(), is_file));
                }
            }
        }
        Ok(())
    }
}
