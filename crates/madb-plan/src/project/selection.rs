//! Selection over already parsed template profiles. No wire, SDK validation or
//! filesystem authority is introduced by these internal planning values.
use super::{ProjectFile, ProjectModule, ProjectPlan, ProjectPlanError};
use madb_model::Id;
use std::collections::{BTreeMap, BTreeSet};

/// An exact target identity and its explicitly declared platform family.
/// ABI, SDK and boundary references remain the owning adapter's responsibility;
/// a family never expands into additional target identities here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTarget {
    pub id: Id,
    pub family: Id,
}

/// The complete module and file inputs for one explicit template combination.
/// Selection does not trim these inputs or fill in missing dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectProfile {
    pub id: Id,
    pub targets: Vec<ProjectTarget>,
    pub modules: Vec<ProjectModule>,
    pub files: Vec<ProjectFile>,
}

/// A selected template combination with a lexically checked ProjectPlan.
/// This is neither a checked SDK/ABI configuration nor permission to create it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedProjectProfile {
    profile_id: Id,
    families: Vec<Id>,
    targets: Vec<ProjectTarget>,
    plan: ProjectPlan,
}

impl SelectedProjectProfile {
    pub fn profile_id(&self) -> &Id {
        &self.profile_id
    }

    /// Sorted, unique platform families resolved from the complete selection.
    pub fn families(&self) -> &[Id] {
        &self.families
    }

    /// All declared exact targets, ordered by identity, without ABI expansion.
    pub fn targets(&self) -> &[ProjectTarget] {
        &self.targets
    }

    pub fn plan(&self) -> &ProjectPlan {
        &self.plan
    }
}

/// Apply ADR-0019's four profile/targets branches, then validate the selected
/// combination through ProjectPlan. Explicit empty or duplicate families fail
/// before profile lookup. Repeated families in distinct exact targets are valid.
///
/// The default is required when both selectors are absent; with targets alone it
/// only disambiguates multiple exact matches. No array-order fallback is used.
/// Unselected module/file plans are not validated or executed by this function.
pub fn select_project_profile(
    profiles: &[ProjectProfile],
    default_profile: Option<&Id>,
    profile: Option<&Id>,
    targets: Option<&[Id]>,
) -> Result<SelectedProjectProfile, ProjectPlanError> {
    let requested = targets.map(requested_families).transpose()?;
    let mut index = BTreeMap::new();
    for candidate in profiles {
        if index.contains_key(&candidate.id) {
            return Err(ProjectPlanError::new(
                candidate.id.as_str(),
                "duplicate template profile identity",
            ));
        }
        index.insert(&candidate.id, (candidate, profile_families(candidate)?));
    }

    let (selected, families) = match (profile, requested.as_ref()) {
        (Some(id), requested) => {
            let selected = index
                .get(id)
                .ok_or_else(|| ProjectPlanError::new(id.as_str(), "unknown template profile"))?;
            if requested.is_some_and(|requested| requested != &selected.1) {
                return Err(ProjectPlanError::new(
                    id.as_str(),
                    "template profile and requested platform families differ",
                ));
            }
            selected
        }
        (None, None) => default_profile
            .and_then(|id| index.get(id))
            .ok_or_else(|| {
                ProjectPlanError::new("default_profile", "missing or unknown default profile")
            })?,
        (None, Some(requested)) => {
            let matches: Vec<_> = index
                .values()
                .filter(|(_, families)| families == requested)
                .collect();
            match matches.as_slice() {
                [] => {
                    return Err(ProjectPlanError::new(
                        "targets",
                        "no template profile has the requested platform families",
                    ));
                }
                [selected] => selected,
                _ => matches
                    .into_iter()
                    .find(|(candidate, _)| Some(&candidate.id) == default_profile)
                    .ok_or_else(|| {
                        ProjectPlanError::new("targets", "ambiguous template profile selection")
                    })?,
            }
        }
    };
    let plan = ProjectPlan::new(selected.modules.clone(), selected.files.clone())?;
    let mut targets = selected.targets.clone();
    targets.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(SelectedProjectProfile {
        profile_id: selected.id.clone(),
        families: families.iter().cloned().collect(),
        targets,
        plan,
    })
}

fn requested_families(targets: &[Id]) -> Result<BTreeSet<Id>, ProjectPlanError> {
    if targets.is_empty() {
        return Err(ProjectPlanError::new(
            "targets",
            "explicit targets are empty",
        ));
    }
    let mut families = BTreeSet::new();
    for family in targets {
        if !families.insert(family.clone()) {
            return Err(ProjectPlanError::new(
                family.as_str(),
                "duplicate requested platform family",
            ));
        }
    }
    Ok(families)
}

fn profile_families(profile: &ProjectProfile) -> Result<BTreeSet<Id>, ProjectPlanError> {
    if profile.targets.is_empty() {
        return Err(ProjectPlanError::new(
            profile.id.as_str(),
            "template profile has no targets",
        ));
    }
    let mut identities = BTreeSet::new();
    let mut families = BTreeSet::new();
    for target in &profile.targets {
        if !identities.insert(&target.id) {
            return Err(ProjectPlanError::new(
                target.id.as_str(),
                "duplicate target identity in template profile",
            ));
        }
        families.insert(target.family.clone());
    }
    Ok(families)
}
