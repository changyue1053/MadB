use madb_model::contract::ResolvedSelector;
use madb_model::evidence::*;
use madb_model::{Finding, Id};
use std::collections::BTreeSet;

pub fn evidence_freshness(
    expected: &EvidenceIdentity,
    actual: Option<&EvidenceIdentity>,
) -> Freshness {
    let Some(actual) = actual else {
        return Freshness::Missing;
    };
    let fields = [
        ("input", expected.input == actual.input),
        ("tools", expected.tools == actual.tools),
        ("profile", expected.profile == actual.profile),
        ("candidate", expected.candidate == actual.candidate),
        ("lab_revision", expected.lab_revision == actual.lab_revision),
        ("case_catalog", expected.case_catalog == actual.case_catalog),
    ];
    let changed: Vec<_> = fields
        .into_iter()
        .filter_map(|(name, equal)| (!equal).then_some(name))
        .collect();
    if changed.is_empty() {
        Freshness::Current
    } else {
        Freshness::Stale(changed)
    }
}

/// Every catalog problem needs an explicit applicability decision. This creates
/// requirement instances, not executed cases or a manufactured coverage percentage.
/// The application must first validate the supplied selector against its profile.
pub fn instantiate_requirements(
    catalog: &[Id],
    selector: &ResolvedSelector,
    choices: &[Requirement],
) -> Result<Vec<RequirementInstance>, Vec<Finding>> {
    let expected: BTreeSet<_> = catalog.iter().collect();
    let mut seen = BTreeSet::new();
    let mut errors = Vec::new();
    if expected.len() != catalog.len() {
        errors.push(Finding::new(
            "COV001",
            "catalog",
            "duplicate problem identity",
        ));
    }
    for choice in choices {
        if !expected.contains(&choice.problem_id) || !seen.insert(&choice.problem_id) {
            errors.push(Finding::new(
                "COV001",
                choice.problem_id.as_str(),
                "unknown or repeated applicability decision",
            ));
        }
        if choice.reason.trim().is_empty()
            || (choice.applicability != Applicability::NotApplicable && choice.case_ids.is_empty())
            || (choice.applicability == Applicability::NotApplicable && !choice.case_ids.is_empty())
        {
            errors.push(Finding::new(
                "COV002",
                choice.problem_id.as_str(),
                "applicability requires a reason and appropriate case set",
            ));
        }
        let ids: BTreeSet<_> = choice.case_ids.iter().collect();
        if ids.len() != choice.case_ids.len() {
            errors.push(Finding::new(
                "COV003",
                choice.problem_id.as_str(),
                "repeated case identity",
            ));
        }
    }
    for missing in expected.difference(&seen) {
        errors.push(Finding::new(
            "COV004",
            missing.as_str(),
            "missing problem applicability decision",
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut result: Vec<_> = choices
        .iter()
        .map(|requirement| RequirementInstance {
            selector: selector.clone(),
            requirement: requirement.clone(),
        })
        .collect();
    result.sort_by(|a, b| a.requirement.problem_id.cmp(&b.requirement.problem_id));
    Ok(result)
}

/// Evaluate results against independently supplied case definitions and identity.
/// A producer's `passed` string never substitutes for evidence or an exact negative.
pub fn validate_required_results(
    expected_identity: &EvidenceIdentity,
    actual_identity: &EvidenceIdentity,
    required: &[RequiredCase],
    results: &[CaseResult],
) -> Vec<Finding> {
    let mut errors = Vec::new();
    if evidence_freshness(expected_identity, Some(actual_identity)) != Freshness::Current {
        errors.push(Finding::new(
            "EVD001",
            "report",
            "evidence dependencies changed",
        ));
    }
    let mut seen = BTreeSet::new();
    for result in results {
        if !seen.insert(&result.case_id) {
            errors.push(Finding::new(
                "EVD002",
                result.case_id.as_str(),
                "duplicate result",
            ));
        }
    }
    let mut definitions = BTreeSet::new();
    for case in required {
        if !definitions.insert(&case.id) {
            errors.push(Finding::new(
                "EVD002",
                case.id.as_str(),
                "duplicate required definition",
            ));
        }
        let Some(result) = results.iter().find(|result| result.case_id == case.id) else {
            errors.push(Finding::new(
                "EVD003",
                case.id.as_str(),
                "required case has no execution evidence",
            ));
            continue;
        };
        if result.case_digest != case.digest || result.expected != case.expected {
            errors.push(Finding::new(
                "EVD004",
                case.id.as_str(),
                "case identity or independent expectation differs",
            ));
        }
        if result.status != ExecutionStatus::Passed
            || !matches_expected(&case.expected, &result.actual)
        {
            errors.push(Finding::new(
                "EVD005",
                case.id.as_str(),
                "required case did not produce its precise expected result",
            ));
        }
        if result.level < case.minimum_level
            || !result.cleanup_complete
            || result.attachment_count == 0
        {
            errors.push(Finding::new(
                "EVD006",
                case.id.as_str(),
                "required evidence level, cleanup or attachment missing",
            ));
        }
        let mut counters = BTreeSet::new();
        for observation in &result.resources {
            if !counters.insert(&observation.name)
                || observation.before != observation.after
                || u128::from(observation.before) + u128::from(observation.created)
                    != u128::from(observation.after) + u128::from(observation.released)
            {
                errors.push(Finding::new(
                    "EVD007",
                    case.id.as_str(),
                    "resource cleanup counters contradict a completed case",
                ));
            }
        }
    }
    errors
}
fn matches_expected(expected: &ExpectedOutcome, actual: &ActualOutcome) -> bool {
    match (expected, actual) {
        (ExpectedOutcome::Success, ActualOutcome::Success) => true,
        (
            ExpectedOutcome::Rejection { code: a, stage: x },
            ActualOutcome::Rejection { code: b, stage: y },
        ) => a == b && x == y,
        (
            ExpectedOutcome::IsolatedAbort {
                exit_code: a,
                reason: x,
            },
            ActualOutcome::IsolatedAbort {
                exit_code: b,
                reason: y,
            },
        ) => a == b && x == y,
        _ => false,
    }
}
