use super::*;
use crate::{Digest, ErrorKind, Id, LogicalPath, QueryError, Result};
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{
    CheckLevel, FileIdentity, FileOwnership, ProjectFile, ProjectModule, ProjectPlan,
};
use std::collections::BTreeMap;

/// Encodes observations, never execution authority. Implemented by contracts.
pub trait InitResultFormat {
    fn encode_result(&self, report: &InitReport) -> Result<Vec<u8>>;
}
/// The Host saves exactly these result bytes and independently reads/identifies
/// every required evidence file. It must not echo expected references as proof.
pub trait InitEvidencePort {
    fn save_result(
        &mut self,
        root: &AbsolutePath,
        result_bytes: &[u8],
        required_files: &[FileIdentity],
    ) -> Result<SavedEvidenceObservation>;
}
#[derive(Clone, Debug)]
pub struct SavedEvidenceObservation {
    pub root: AbsolutePath,
    pub files: Vec<FileIdentity>,
}
/// A completion can only escape after policy/facts checks and the evidence port.
#[derive(Debug)]
pub struct CompletedInitialization {
    report: InitReport,
    bytes: Vec<u8>,
}
impl CompletedInitialization {
    pub fn report(&self) -> &InitReport {
        &self.report
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn complete_initialization<F: InitResultFormat, E: InitEvidencePort>(
    request: &FrozenInitRequest,
    project: &PreparedProject,
    committed: Option<&CommittedProject>,
    observations: SuccessObservations,
    format: &F,
    evidence_store: &mut E,
) -> Result<CompletedInitialization> {
    if request != project.request() {
        return Err(invalid(
            "result",
            "prepared project belongs to a different frozen request",
        ));
    }
    let frozen = request.projection();
    validate_observation_context(request, project, &observations)?;
    let creation = if frozen.dry_run {
        if committed.is_some()
            || !observations.checks.is_empty()
            || observations.open.status != ExecutionStatus::Skipped
            || !observations.open.attachments.is_empty()
        {
            return Err(evidence(
                "dry-run",
                "dry-run cannot have a commit, executed checks, or launched IDE",
            ));
        }
        Creation::NotCreated
    } else {
        if !committed.is_some_and(|receipt| receipt.matches(request, project)) {
            return Err(evidence(
                "commit",
                "ordinary success requires this request's actual same-root same-plan CommitNew receipt",
            ));
        }
        check_required(request, &observations.checks)?;
        let expected_open = if frozen.open.is_some() {
            ExecutionStatus::Passed
        } else {
            ExecutionStatus::Skipped
        };
        if observations.open.status != expected_open {
            return Err(evidence("open", "requested IDE launch did not complete"));
        }
        if frozen.open.is_some() && observations.open.attachments.is_empty() {
            return Err(evidence("open", "launch success lacks actual evidence"));
        }
        Creation::Created
    };
    let mut report = progress_report(request, project, observations)?;
    report.creation = creation;
    report.termination = Termination::Completed;
    report.exit_code = 0;
    let bytes = save_report(request, &report, format, evidence_store)?;
    Ok(CompletedInitialization { report, bytes })
}

fn progress_report(
    request: &FrozenInitRequest,
    project: &PreparedProject,
    observations: SuccessObservations,
) -> Result<InitReport> {
    validate_observation_context(request, project, &observations)?;
    let selected = request.selected();
    let ide_entries = selected
        .template()
        .ide_entries
        .iter()
        .filter(|entry| selected.profile().ide_entries.contains(&entry.id))
        .cloned()
        .collect();
    let plan = PlanObservation {
        files: project
            .files()
            .iter()
            .map(|file| PlannedFileObservation {
                module: file.module().clone(),
                path: file.path().clone(),
                ownership: file.ownership(),
                content: file.reference().map(|f| (f.digest, f.size)),
            })
            .collect(),
        task_roots: selected.profile().task_roots.clone(),
        downloads: Vec::new(),
    };
    let report = InitReport {
        request_digest: Some(request.digest().clone()),
        tool: observations.tool,
        template: Some(observations.template),
        template_package: Some(observations.template_package),
        evidence_root: Some(DisplayPath::Absolute(observations.evidence_root)),
        template_profile: Some(selected.profile_id().clone()),
        boundary_profiles: observations.boundary_profiles,
        request: Some(request_observation(request)),
        output: Some(DisplayPath::Absolute(project.root().clone())),
        plan: Some(plan),
        creation: Creation::NotCreated,
        termination: Termination::Failed,
        exit_code: 1,
        checks: observations.checks,
        ide_entries,
        open: observations.open,
        recovery: Vec::new(),
        diagnostics: observations.diagnostics,
        logs: observations.logs,
    };
    Ok(report)
}

fn validate_observation_context(
    request: &FrozenInitRequest,
    project: &PreparedProject,
    observations: &SuccessObservations,
) -> Result<()> {
    let frozen = request.projection();
    let project_prefix = format!("{}/", project.root().as_str());
    if observations.evidence_root == *project.root()
        || observations
            .evidence_root
            .as_str()
            .starts_with(&project_prefix)
    {
        return Err(invalid(
            "evidence_root",
            "report evidence must remain outside the new user project",
        ));
    }
    check_provenance(project, observations)?;
    if observations.open.requested != frozen.open {
        return Err(invalid(
            "open",
            "open observation differs from the frozen request",
        ));
    }
    Ok(())
}

fn save_report<F: InitResultFormat, E: InitEvidencePort>(
    request: &FrozenInitRequest,
    report: &InitReport,
    format: &F,
    evidence_store: &mut E,
) -> Result<Vec<u8>> {
    let root = match &report.evidence_root {
        Some(DisplayPath::Absolute(root)) => root.clone(),
        _ => {
            return Err(invalid(
                "evidence_root",
                "saving requires an actual evidence root",
            ));
        }
    };
    let required = evidence_inventory(request, report)?;
    let bytes = format.encode_result(report)?;
    let actual = evidence_store.save_result(&root, &bytes, &required)?;
    let mut expected = required;
    expected.push(FileIdentity {
        path: logical("init-result.json")?,
        sha256: Digest::of_bytes(&bytes),
        size: bytes.len() as u64,
    });
    let expected = unique_files(expected)?;
    let actual_count = actual.files.len();
    let actual_files = unique_files(actual.files)?;
    if actual.root != root || actual_files.len() != actual_count || actual_files != expected {
        return Err(QueryError::new(
            ErrorKind::Host,
            "FILE001",
            "evidence",
            "saved evidence root/files differ from actual report bytes and required references",
        ));
    }
    Ok(bytes)
}

/// Save the exact observations already made by this attempt on failure. The
/// result remains failed/cancelled and cannot become completion authority.
/// Original inputs, successful predecessor checks and failed-stage logs stay
/// in the same evidence root; creation still comes only from the Host receipt.
pub fn preserve_initialization_failure<F: InitResultFormat, E: InitEvidencePort>(
    project: &PreparedProject,
    created: Option<&CreatedProject>,
    observations: SuccessObservations,
    failure: InitializationFailure,
    format: &F,
    evidence_store: &mut E,
) -> Result<InitReport> {
    let request = project.request();
    check_provenance(project, &observations)?;
    let report = progress_report(request, project, observations)?;
    let mut report = failure_report(report, Some(request), created, failure)?;
    if created.is_some() {
        report.recovery.push(Recovery { kind: RecoveryKind::Inspect,
            message: "The project was created and has been preserved. Inspect its recorded stage diagnostics; do not initialize over it.".into(), arguments: None });
    }
    save_report(request, &report, format, evidence_store)?;
    Ok(report)
}

fn check_provenance(project: &PreparedProject, observed: &SuccessObservations) -> Result<()> {
    let selected = project.request().selected();
    let template = selected.template();
    if observed.template.id != template.id
        || observed.template.version != template.version
        || observed.template.package_sha256 != project.package().sha256
        || observed.template_package.sha256 != project.package().sha256
        || observed.template_package.size != project.package().size
    {
        return Err(invalid(
            "template",
            "result template/package differs from the frozen selected inputs",
        ));
    }
    let prefix = format!("{}/", observed.template.input_root.as_str());
    let expected_manifest = format!("{}{}", prefix, project.manifest().path.as_str());
    if observed.template.manifest.schema.as_str() != "madb.project-template/1"
        || observed.template.manifest.file.path.as_str() != expected_manifest
        || observed.template.manifest.file.sha256 != project.manifest().sha256
        || observed.template.manifest.file.size != project.manifest().size
    {
        return Err(invalid(
            "manifest",
            "result manifest does not preserve the original package root and bytes",
        ));
    }
    let mut expected = Vec::new();
    for boundary in template
        .boundary_refs
        .iter()
        .filter(|b| selected.profile().boundary_refs.contains(&b.id))
    {
        let mut profile = boundary.profile.clone();
        profile.file.path = logical(&format!("{}{}", prefix, profile.file.path.as_str()))?;
        expected.push(profile);
    }
    expected.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    if observed.boundary_profiles != expected {
        return Err(invalid(
            "boundary_profiles",
            "result boundary references differ from the selected original profile bytes",
        ));
    }
    if observed.tool.version != project.request().actual_madb_version() {
        return Err(invalid(
            "tool",
            "result tool version differs from the formal version checked at freeze",
        ));
    }
    Ok(())
}

fn check_required(request: &FrozenInitRequest, checks: &[CheckObservation]) -> Result<()> {
    let level = match request.projection().verify {
        VerifyLevel::Structure => CheckLevel::Structure,
        VerifyLevel::Build => CheckLevel::Build,
        VerifyLevel::Run => CheckLevel::Run,
    };
    let selected = request.selected();
    let required = selected
        .required_checks(level)
        .map_err(|e| invalid(e.subject, e.message))?;
    let mut observed = BTreeMap::new();
    for check in checks {
        if observed.insert((&check.id, &check.target), check).is_some() {
            return Err(invalid("checks", "duplicate check identity/target"));
        }
        let declaration = selected
            .validation()
            .iter()
            .find(|d| d.id == check.id && d.target == check.target)
            .ok_or_else(|| {
                invalid(
                    check.id.as_str(),
                    "result contains an undeclared selected check",
                )
            })?;
        if check.level != declaration.level || check.level > level {
            return Err(invalid(
                check.id.as_str(),
                "check level differs or was not requested",
            ));
        }
        if check.cleanup == Cleanup::Failed {
            return Err(evidence(
                check.id.as_str(),
                "process cleanup did not complete",
            ));
        }
        if (matches!(
            check.status,
            ExecutionStatus::Skipped | ExecutionStatus::Unsupported | ExecutionStatus::Blocked
        ) || check.requirement == Applicability::NotApplicable)
            && check.reason.as_ref().is_none_or(|reason| reason.is_empty())
        {
            return Err(invalid(
                check.id.as_str(),
                "non-execution/applicability requires a reason",
            ));
        }
        if check.status == ExecutionStatus::Passed {
            let actual = check
                .evidence_level
                .ok_or_else(|| evidence(check.id.as_str(), "passed check has no evidence level"))?;
            if check.attachments.is_empty() {
                return Err(evidence(
                    check.id.as_str(),
                    "passed check has no evidence attachment",
                ));
            }
            if actual > EvidenceLevel::Static
                && (check.environment.is_empty() || check.cleanup != Cleanup::Complete)
            {
                return Err(evidence(
                    check.id.as_str(),
                    "executed check lacks tool identities or complete cleanup",
                ));
            }
        }
    }
    for requirement in required {
        let actual = observed
            .get(&(&requirement.id, &requirement.target))
            .ok_or_else(|| evidence(requirement.id.as_str(), "required check is missing"))?;
        if actual.status != ExecutionStatus::Passed
            || actual.requirement != Applicability::Required
            || actual.level != requirement.level
            || actual
                .evidence_level
                .is_none_or(|level| level < requirement.minimum_evidence)
        {
            return Err(evidence(
                requirement.id.as_str(),
                "required check did not pass with the required evidence",
            ));
        }
    }
    Ok(())
}
fn evidence_inventory(
    request: &FrozenInitRequest,
    report: &InitReport,
) -> Result<Vec<FileIdentity>> {
    let template = report
        .template
        .as_ref()
        .expect("constructed complete template");
    let mut files = vec![
        report
            .template_package
            .clone()
            .expect("constructed package"),
        template.manifest.file.clone(),
    ];
    for input in &request.selected().template().files {
        files.push(FileIdentity {
            path: logical(&format!(
                "{}/{}",
                template.input_root.as_str(),
                input.path.as_str()
            ))?,
            sha256: input.sha256.clone(),
            size: input.size,
        });
    }
    files.extend(
        report
            .boundary_profiles
            .iter()
            .map(|profile| profile.file.clone()),
    );
    files.extend(report.logs.iter().cloned());
    files.extend(report.open.attachments.iter().cloned());
    for check in &report.checks {
        files.extend(check.attachments.iter().cloned());
    }
    if files
        .iter()
        .any(|file| file.path.as_str() == "init-result.json")
    {
        return Err(invalid("evidence", "result cannot reference itself"));
    }
    unique_files(files)
}
fn unique_files(files: Vec<FileIdentity>) -> Result<Vec<FileIdentity>> {
    let mut by_path = BTreeMap::new();
    for file in files {
        if let Some(old) = by_path.insert(file.path.clone(), file.clone())
            && old != file
        {
            return Err(invalid(
                file.path.as_str(),
                "conflicting evidence byte identities",
            ));
        }
    }
    let files: Vec<_> = by_path.into_values().collect();
    let module = Id::new("evidence").expect("static id");
    ProjectPlan::new(
        vec![ProjectModule {
            id: module.clone(),
            directory: None,
            dependencies: Vec::new(),
        }],
        files
            .iter()
            .map(|file| ProjectFile {
                module: module.clone(),
                path: file.path.clone(),
                ownership: FileOwnership::Provenance,
            })
            .collect(),
    )
    .map_err(|e| invalid(e.subject, e.message))?;
    Ok(files)
}
fn logical(value: &str) -> Result<LogicalPath> {
    LogicalPath::new(value).map_err(|e| invalid(value, e.to_string()))
}
fn request_observation(request: &FrozenInitRequest) -> RequestObservation {
    let p = request.projection();
    RequestObservation {
        targets: p.targets.clone(),
        verify: p.verify,
        open: p.open.clone(),
        dry_run: p.dry_run,
        offline: p.offline,
    }
}

pub enum InitializationFailure {
    Stage(QueryError),
    Cancelled,
}
/// Preserve the typed stage error even when no request digest could be formed.
/// A failed/cancelled report never grants a commit and never erases an actual one.
pub fn failure_report(
    mut progress: InitReport,
    request: Option<&FrozenInitRequest>,
    created: Option<&CreatedProject>,
    failure: InitializationFailure,
) -> Result<InitReport> {
    if let Some(receipt) = created {
        if request.is_none_or(|request| receipt.request() != request) {
            return Err(invalid(
                "failure",
                "commit receipt is not bound to this frozen request",
            ));
        }
        progress.creation = Creation::Created;
    } else {
        progress.creation = Creation::NotCreated;
    }
    if let Some(request) = request {
        progress.request_digest = Some(request.digest().clone());
        progress.request = Some(request_observation(request));
        progress.output = Some(DisplayPath::Absolute(request.projection().output.clone()));
    }
    match failure {
        InitializationFailure::Cancelled => {
            progress.termination = Termination::Cancelled;
            progress.exit_code = 1;
        }
        InitializationFailure::Stage(error) => {
            progress.termination = Termination::Failed;
            progress.exit_code = match error.kind {
                ErrorKind::InvalidInput => 2,
                ErrorKind::CheckFailed => 1,
                ErrorKind::Host => 3,
                ErrorKind::Conflict => 4,
            };
            let diagnostics = error
                .findings
                .into_iter()
                .map(|finding| {
                    Ok(Diagnostic {
                        code: Id::new(finding.code)
                            .map_err(|error| invalid("diagnostic.code", error.to_string()))?,
                        message: finding.message,
                        location: finding.locations.into_iter().next(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            progress.diagnostics.extend(diagnostics);
        }
    }
    Ok(progress)
}
