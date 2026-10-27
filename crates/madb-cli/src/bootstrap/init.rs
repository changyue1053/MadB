//! Installed initialization composition. Format/plan/rendering remain pure;
//! only Host obtains identities, creates directories, and saves evidence.
mod catalog;
mod evidence;
mod prepare;

use crate::init::InitOptions;
use madb_app::transaction::CommitControl;
use madb_app::use_cases::initialize::*;
use madb_app::{ErrorKind, QueryError, Result};
use madb_contracts::project_init::ProjectInitAdapter;
use madb_host::initialization::LocalProjectCreator;
use madb_host::initialization_source::{
    FrozenInitializationTool, normalize_output, observe_initialization_host,
};
use madb_plan::project::{CheckLevel, Version};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

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
        let record = catalog::installed(self.tool.identity())?;
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
        control.checkpoint()?;
        self.tool.revalidate()?;
        let completed = complete_initialization(
            &frozen,
            &project,
            committed.as_ref(),
            observations,
            &self.format,
            &mut store,
        )?;
        self.tool.revalidate()?;
        control.checkpoint()?;
        Ok(completed)
    }
    fn failure(&self, progress: Progress, error: AttemptError) -> InitReport {
        let mut report = evidence::empty_report(self.tool.identity());
        report.output = progress.output.map(DisplayPath::Absolute);
        if let Some(request) = &progress.request {
            report.open = OpenObservation::skipped(request.projection().open.clone());
        }
        let failure = match error {
            AttemptError::Stage(error) => {
                InitializationFailure::Stage(evidence::bounded_error(error))
            }
            AttemptError::Cancelled => InitializationFailure::Cancelled,
        };
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
}
#[derive(Clone, Default)]
struct InitControl {
    commit: CommitControl,
    requested: Arc<AtomicBool>,
}
impl InitControl {
    fn checkpoint(&self) -> AttemptResult<()> {
        if self.requested.load(Ordering::Acquire) {
            Err(AttemptError::Cancelled)
        } else {
            Ok(())
        }
    }
}
enum AttemptError {
    Stage(QueryError),
    Cancelled,
}
impl From<QueryError> for AttemptError {
    fn from(value: QueryError) -> Self {
        Self::Stage(value)
    }
}
type AttemptResult<T> = std::result::Result<T, AttemptError>;
#[cfg(unix)]
fn install_signals(control: &InitControl) -> Result<madb_host::cancellation::CancellationSignals> {
    let control = control.clone();
    madb_host::cancellation::CancellationSignals::install_action(move || {
        control.requested.store(true, Ordering::Release);
        control.commit.cancel();
    })
}
#[cfg(not(unix))]
fn install_signals(_: &InitControl) -> Result<madb_host::cancellation::CancellationSignals> {
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST006",
        "initialization",
        "signal delivery is not implemented on this host",
    ))
}
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
    if !request.projection().dry_run && request.projection().verify != VerifyLevel::Structure {
        return Err(invalid(
            "verify",
            "this producer supports initialization structure and dry-run only; build/run execution is an unsupported initialization mode, not a tool or SDK availability result",
        ));
    }
    for check in selected
        .required_checks(CheckLevel::Structure)
        .map_err(|e| invalid(&e.subject, e.message))?
    {
        if check.id.as_str() != "structure"
            || check.minimum_evidence != madb_model::evidence::EvidenceLevel::Static
        {
            return Err(invalid(
                check.id.as_str(),
                "selected required check has no installed structure implementation",
            ));
        }
    }
    Ok(())
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
