//! Installed initialization composition. Format/plan/rendering remain pure;
//! only Host obtains identities, creates directories, and saves evidence.
mod catalog;
mod control;
mod evidence;
mod prepare;
mod recipe;

use crate::init::InitOptions;
use control::{InitControl, install_signals};
use madb_app::use_cases::initialize::*;
use madb_app::{ErrorKind, QueryError, Result};
use madb_contracts::project_init::ProjectInitAdapter;
use madb_host::initialization::{LocalInitEvidence, LocalProjectCreator};
use madb_host::initialization_source::{
    FrozenInitializationTool, normalize_output, observe_initialization_host,
};
use madb_plan::project::{CheckLevel, Version};

pub struct InitializationService {
    format: ProjectInitAdapter,
    tool: FrozenInitializationTool,
}
impl InitializationService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            format: ProjectInitAdapter::new()?,
            tool: FrozenInitializationTool::capture(env!("CARGO_PKG_VERSION"))?,
        })
    }
    pub fn encode(&self, report: &InitReport) -> Result<Vec<u8>> {
        self.format.encode_result(report)
    }
    pub fn initialize(&self, options: Result<InitOptions>) -> InitReport {
        let mut progress = Progress::default();
        let control = InitControl::default();
        let attempt = options
            .map_err(AttemptError::Stage)
            .and_then(|options| self.attempt(options, &control, &mut progress));
        match attempt {
            Ok(completed) => completed.report().clone(),
            Err(error) => self.failure(progress, error),
        }
    }
    fn attempt(
        &self,
        options: InitOptions,
        control: &InitControl,
        progress: &mut Progress,
    ) -> AttemptResult<CompletedInitialization> {
        let _signals = install_signals(control)?;
        control.checkpoint()?;
        self.tool.revalidate()?;
        let output = normalize_output(&options.output)?;
        progress.output = Some(output.clone());
        Version::parse(&self.tool.identity().version).map_err(|_|invalid("tool.version",format!(
            "actual MadB version {} is not a formal N.N.N initialization version; no compatibility substitution is permitted",self.tool.identity().version)))?;
        let record = catalog::installed(self.tool.identity(), options.template.as_deref())?;
        record.select(options.template.as_deref())?;
        let (template, package) = prepare::load(&record, &self.format)?;
        control.checkpoint()?;
        let request = InitRequest {
            template_selector: options.template,
            template_profile_selector: options.profile,
            parameter_values: options.parameters,
            targets: options.targets,
            verify: options.verify,
            open: options.open,
            dry_run: options.dry_run,
            offline: options.offline,
            output,
        };
        let (frozen, _) = freeze_request(
            &template,
            request,
            &self.tool.identity().version,
            &self.format,
        )?;
        progress.request = Some(frozen.clone());
        preflight(&frozen)?;
        let recipe = recipe::ReadyRecipe::preflight(&frozen)?;
        control.checkpoint()?;
        self.tool.revalidate()?;
        let project = prepare::project(&record, &package, &frozen, &self.format)?;
        control.checkpoint()?;
        self.tool.revalidate()?;
        let committed = if frozen.projection().dry_run {
            None
        } else {
            let mut creator = LocalProjectCreator::open(project.root(), control.commit.clone())?;
            let attempt = match commit_new(&mut creator, &frozen, &project) {
                Ok(attempt) => attempt,
                Err(_error) if creator.cancellation_observed() => {
                    return Err(AttemptError::Cancelled);
                }
                Err(error) => return Err(AttemptError::Stage(error)),
            };
            let (created, validated) = attempt.into_parts();
            progress.created = Some(created);
            let committed = validated?;
            control.checkpoint()?;
            Some(committed)
        };
        let mut store = evidence::allocate()?;
        control.checkpoint()?;
        let observations = evidence::observations(
            &record,
            &package,
            &project,
            self.tool.identity(),
            &mut store,
            committed.as_ref(),
        )?;
        progress.saved = Some(EvidenceProgress {
            project: project.clone(),
            observations,
            store,
        });
        control.checkpoint()?;
        self.tool.revalidate()?;
        let saved = progress.saved.as_mut().expect("observations retained");
        if let Some(recipe) = &recipe {
            recipe.execute(
                &project,
                &self.tool,
                control,
                &mut saved.observations,
                &mut saved.store,
                &mut progress.retained,
            )?;
        }
        let completed = complete_initialization(
            &frozen,
            &project,
            committed.as_ref(),
            saved.observations.clone(),
            &self.format,
            &mut saved.store,
        )?;
        self.tool.revalidate()?;
        control.checkpoint()?;
        Ok(completed)
    }
    fn failure(&self, mut progress: Progress, error: AttemptError) -> InitReport {
        let cleanup = settle_owners(&mut progress.retained);
        let mut retained_checks = Vec::new();
        let preservation_error = if let Some(mut saved) = progress.saved.take() {
            retained_checks = saved.observations.checks.clone();
            let preservation = (|| {
                if saved.store.result_was_saved() {
                    saved.store = saved.store.fork_verified()?;
                    saved.observations.evidence_root = saved.store.root().clone();
                }
                if let Some(bytes) = cleanup {
                    let file =
                        evidence::save_file(&mut saved.store, "cleanup/final-poll.txt", &bytes)?;
                    saved.observations.logs.push(file);
                }
                preserve_initialization_failure(
                    &saved.project,
                    progress.created.as_ref(),
                    saved.observations.clone(),
                    error.observation(),
                    &self.format,
                    &mut saved.store,
                )
            })();
            match preservation {
                Ok(report) => return report,
                Err(error) => Some(error),
            }
        } else {
            None
        };
        let mut report = evidence::empty_report(self.tool.identity());
        report.output = progress.output.map(DisplayPath::Absolute);
        if let Some(request) = &progress.request {
            report.open = OpenObservation::skipped(request.projection().open.clone());
        }
        let failure = error.observation();
        let mut report = match failure_report(
            report,
            progress.request.as_ref(),
            progress.created.as_ref(),
            failure,
        ) {
            Ok(report) => report,
            Err(error) => {
                let mut report = evidence::empty_report(self.tool.identity());
                evidence::append_failure(&mut report, error);
                return report;
            }
        };
        if let Some(error) = preservation_error {
            evidence::append_failure(&mut report, error);
            // Keep observed stage facts even when their saved attachments can
            // no longer be proved. The failed report carries no phantom refs.
            for check in &mut retained_checks {
                check.attachments.clear();
            }
            report.checks = retained_checks;
        }
        if progress.created.is_some() {
            report.recovery.push(Recovery{kind:RecoveryKind::Inspect,
                message:"The project was created and has been preserved. Inspect the recorded diagnostic and existing project; do not initialize over it.".into(),arguments:None});
        }
        // Argument/version failures need no write. Once a request was frozen,
        // attempt a separate failure result without claiming unsaved FileRefs.
        if progress.request.is_some()
            && let Err(error) = self.save_failure(&mut report)
        {
            report.evidence_root = None;
            evidence::append_failure(&mut report, error);
        }
        report
    }
    fn save_failure(&self, report: &mut InitReport) -> Result<()> {
        let mut store = evidence::allocate()?;
        report.evidence_root = Some(DisplayPath::Absolute(store.root().clone()));
        let bytes = self.format.encode_result(report)?;
        let root = store.root().clone();
        let saved = store.save_result(&root, &bytes, &[])?;
        let actual = madb_app::Digest::of_bytes(&bytes);
        if saved.root != root
            || saved.files.len() != 1
            || saved.files[0].path.as_str() != "init-result.json"
            || saved.files[0].sha256 != actual
            || saved.files[0].size != bytes.len() as u64
        {
            return Err(QueryError::new(
                ErrorKind::Host,
                "FILE001",
                "failure result",
                "saved failure result differs from actual report bytes",
            ));
        }
        Ok(())
    }
}
#[derive(Default)]
struct Progress {
    request: Option<FrozenInitRequest>,
    created: Option<CreatedProject>,
    output: Option<AbsolutePath>,
    saved: Option<EvidenceProgress>,
    retained: Vec<crate::bootstrap::execution::RetainedExecution>,
}
struct EvidenceProgress {
    project: PreparedProject,
    observations: SuccessObservations,
    store: LocalInitEvidence,
}
#[derive(Debug)]
enum AttemptError {
    Stage(QueryError),
    Cancelled,
}
impl AttemptError {
    fn observation(&self) -> InitializationFailure {
        match self {
            Self::Stage(error) => {
                InitializationFailure::Stage(evidence::bounded_error(QueryError {
                    kind: error.kind,
                    findings: error.findings.clone(),
                }))
            }
            Self::Cancelled => InitializationFailure::Cancelled,
        }
    }
}
impl From<QueryError> for AttemptError {
    fn from(value: QueryError) -> Self {
        Self::Stage(value)
    }
}
type AttemptResult<T> = std::result::Result<T, AttemptError>;
fn preflight(request: &FrozenInitRequest) -> Result<()> {
    let selected = request.selected();
    let requirements = &selected.profile().requirements.structure;
    let host = observe_initialization_host()?;
    if !requirements.hosts.iter().any(|entry| {
        entry.os == host.os && entry.arch == host.arch && entry.versions.contains(host.version)
    }) {
        return Err(invalid(
            "host",
            "this installed template does not support structure initialization on the observed host",
        ));
    }
    if !requirements.tools.is_empty() || !requirements.ide_entries.is_empty() {
        return Err(invalid(
            "structure requirements",
            "this installed structure recipe has no external tool or IDE preparation",
        ));
    }
    let level = if request.projection().dry_run {
        CheckLevel::Structure
    } else {
        match request.projection().verify {
            VerifyLevel::Structure => CheckLevel::Structure,
            VerifyLevel::Build => CheckLevel::Build,
            VerifyLevel::Run => CheckLevel::Run,
        }
    };
    let mut sets = vec![&selected.profile().requirements.structure];
    if level >= CheckLevel::Build {
        sets.push(&selected.profile().requirements.build);
    }
    if level >= CheckLevel::Run {
        sets.push(&selected.profile().requirements.run);
    }
    for set in sets {
        if !set.hosts.iter().any(|entry| {
            entry.os == host.os && entry.arch == host.arch && entry.versions.contains(host.version)
        }) || !set.ide_entries.is_empty()
        {
            return Err(invalid(
                "requirements",
                "requested verification has no supported host/IDE recipe",
            ));
        }
    }
    for check in selected
        .required_checks(level)
        .map_err(|e| invalid(&e.subject, e.message))?
    {
        if !matches!(
            (check.id.as_str(), check.minimum_evidence),
            ("structure", madb_model::evidence::EvidenceLevel::Static)
                | (
                    "native-generate",
                    madb_model::evidence::EvidenceLevel::Generated
                )
                | ("c-link", madb_model::evidence::EvidenceLevel::Linked)
                | ("ordinary-run", madb_model::evidence::EvidenceLevel::HostRun)
        ) {
            return Err(invalid(
                check.id.as_str(),
                "selected required check has no installed finite implementation",
            ));
        }
    }
    Ok(())
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}

fn settle_owners(owners: &mut [crate::bootstrap::execution::RetainedExecution]) -> Option<Vec<u8>> {
    if !owners.iter().any(|owner| owner.pending()) {
        return None;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut observations = String::new();
    loop {
        for (index, owner) in owners
            .iter_mut()
            .enumerate()
            .filter(|(_, owner)| owner.pending())
        {
            for result in owner.poll() {
                observations.push_str(&format!("owner={index} {result}\n"));
            }
        }
        let pending = owners.iter().filter(|owner| owner.pending()).count();
        observations.push_str(&format!("pending_owners={pending}\n"));
        if pending == 0 || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Some(observations.into_bytes())
}

#[cfg(all(test, target_os = "macos"))]
mod late_failure_tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn saved_completion_then_cancel_or_drift_preserves_actual_stage_evidence() {
        let parent = madb_host::locations::temporary_parent()
            .unwrap()
            .join(madb_host::locations::unique_run_name().as_str());
        std::fs::create_dir(&parent).unwrap();
        let service = InitializationService::new().unwrap();
        for (index, error) in [
            AttemptError::Cancelled,
            AttemptError::Stage(QueryError::new(
                ErrorKind::Conflict,
                "FILE001",
                "tool",
                "actual frozen tool drift",
            )),
        ]
        .into_iter()
        .enumerate()
        {
            let mut progress = Progress::default();
            let control = InitControl::default();
            let options = InitOptions {
                template: None,
                profile: None,
                parameters: BTreeMap::new(),
                targets: None,
                verify: VerifyLevel::Structure,
                open: None,
                dry_run: false,
                offline: true,
                output: parent.join(format!("late_{index}")),
            };
            let completed = service.attempt(options, &control, &mut progress).unwrap();
            let old = progress.saved.as_ref().unwrap().store.root().clone();
            assert_eq!(completed.report().exit_code, 0);
            let report = service.failure(progress, error);
            assert_eq!(report.creation, Creation::Created);
            assert_eq!(report.exit_code, if index == 0 { 1 } else { 4 });
            assert_eq!(report.checks.len(), 1);
            assert_eq!(report.checks[0].id.as_str(), "structure");
            assert!(!report.checks[0].attachments.is_empty());
            let Some(DisplayPath::Absolute(new)) = &report.evidence_root else {
                panic!("saved failure root")
            };
            assert_ne!(new, &old);
            assert_eq!(
                std::fs::read(std::path::Path::new(new.as_str()).join("init-result.json")).unwrap(),
                service.encode(&report).unwrap()
            );
            let old_bytes =
                std::fs::read(std::path::Path::new(old.as_str()).join("init-result.json")).unwrap();
            assert_eq!(old_bytes, completed.bytes());
            for file in &report.checks[0].attachments {
                let bytes =
                    std::fs::read(std::path::Path::new(new.as_str()).join(file.path.as_str()))
                        .unwrap();
                assert_eq!(madb_app::Digest::of_bytes(&bytes), file.sha256);
            }
            std::fs::remove_dir_all(old.as_str()).unwrap();
            std::fs::remove_dir_all(new.as_str()).unwrap();
        }
        std::fs::remove_dir_all(parent).unwrap();
    }
}
