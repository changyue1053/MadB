//! The explicitly selected template enters the same project service after
//! CommitNew. Its optional ordinary run remains an initialization check.
use super::{ProjectService, evidence, invalid};
use crate::bootstrap::init::{AttemptError, control::InitControl};
use crate::{bootstrap::execution::RetainedExecution, project::ProjectOptions};
use madb_app::{
    Digest, ErrorKind, Id, QueryError, Result,
    ports::ReadWorkspace,
    process::{ProcessPolicy, ResultAdmission},
    use_cases::{
        initialize::*,
        project_build::{BuildFailure, BuildResult, Operation, StageId, StageStatus},
    },
};
use madb_host::{
    LocalWorkspace,
    initialization::LocalInitEvidence,
    initialization_execution::{
        FinalProjectExecution, InitializationProcess, InstalledValuesTools,
        LockedInitializationTool, capture_generated_executable,
    },
};
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::CheckLevel;
use std::{collections::BTreeMap, path::Path};

pub(in crate::bootstrap) fn execute(
    project: &PreparedProject,
    observations: &mut SuccessObservations,
    store: &mut LocalInitEvidence,
    retained: &mut Vec<RetainedExecution>,
    control: &InitControl,
) -> BuildResult<()> {
    let options = || {
        Ok(ProjectOptions {
            root: project.root().as_str().into(),
        })
    };
    let step = control.project().map_err(init_failure)?;
    let (install, bytes) =
        ProjectService::run_controlled(Operation::InstallTool, options(), step.project())?;
    drop(step);
    let file = evidence::save_file(store, "project/install-result.json", &bytes)?;
    observations.logs.push(file);
    if install.exit_code != 0 {
        return Err(project_error(&install));
    }
    if project.request().projection().verify == VerifyLevel::Structure {
        return Ok(());
    }
    let step = control.project().map_err(init_failure)?;
    let (build, bytes) =
        ProjectService::run_controlled(Operation::Build, options(), step.project())?;
    drop(step);
    let file = evidence::save_file(store, "project/build-result.json", &bytes)?;
    observations.logs.push(file.clone());
    let tools = InstalledValuesTools::capture()?;
    let requirements = project
        .request()
        .selected()
        .required_checks(
            if project.request().projection().verify == VerifyLevel::Run {
                CheckLevel::Run
            } else {
                CheckLevel::Build
            },
        )
        .map_err(|e| invalid(e.subject, e.message))?;
    for requirement in requirements.iter().filter(|r| r.level == CheckLevel::Build) {
        let passed = match requirement.id.as_str() {
            "native-generate" => build
                .stages
                .iter()
                .any(|s| s.id == StageId::Generate && s.status == StageStatus::Passed),
            "c-link" => build
                .stages
                .iter()
                .any(|s| s.id == StageId::ConsumerLink && s.status == StageStatus::Passed),
            _ => {
                return Err(
                    invalid("initialization", "unimplemented required project check").into(),
                );
            }
        };
        let stage = if requirement.id.as_str() == "native-generate" {
            StageId::Generate
        } else {
            StageId::ConsumerLink
        };
        let status = build.stages.iter().find(|s| s.id == stage).map_or(
            ExecutionStatus::Blocked,
            |s| match s.status {
                StageStatus::Passed => ExecutionStatus::Passed,
                StageStatus::Failed | StageStatus::Error => ExecutionStatus::Failed,
                _ => ExecutionStatus::Blocked,
            },
        );
        observations.checks.push(CheckObservation {
            id: requirement.id.clone(),
            level: requirement.level,
            target: requirement.target.clone(),
            status,
            requirement: Applicability::Required,
            environment: tools.observations(),
            command: None,
            evidence_level: passed.then_some(if requirement.id.as_str() == "native-generate" {
                EvidenceLevel::Generated
            } else {
                EvidenceLevel::Linked
            }),
            cleanup: if passed {
                Cleanup::Complete
            } else {
                Cleanup::NotRun
            },
            diagnostics: build.diagnostics.clone(),
            attachments: vec![file.clone()],
            reason: (status == ExecutionStatus::Blocked)
                .then(|| "The required preceding project stage did not complete.".into()),
        });
    }
    if build.exit_code != 0 {
        return Err(project_error(&build));
    }
    if project.request().projection().verify != VerifyLevel::Run {
        return Ok(());
    }
    // Freeze the initialized source inventory too: this run is specifically for
    // the just-created template sample, not an arbitrary later user revision.
    FinalProjectExecution::open(project)?.verify_initial()?;
    let log_start = observations.logs.len();
    let outcome = ordinary_run(&build, store, retained, &mut observations.logs, control);
    let required = requirements
        .iter()
        .find(|r| r.id.as_str() == "ordinary-run")
        .ok_or_else(|| invalid("run", "ordinary run requirement absent"))?;
    let diagnostics = outcome
        .as_ref()
        .err()
        .map(|error| match error {
            BuildFailure::Error(error) => Diagnostic {
                code: Id::new(error.findings.first().map_or("EXEC010", |f| f.code)).unwrap(),
                message: error.to_string(),
                location: None,
            },
            BuildFailure::Cancelled => Diagnostic {
                code: Id::new("EXEC001").unwrap(),
                message: "Ordinary run admission was rejected by cancellation.".into(),
                location: None,
            },
        })
        .into_iter()
        .collect();
    observations.checks.push(CheckObservation {
        id: required.id.clone(),
        level: required.level,
        target: required.target.clone(),
        status: if outcome.is_ok() {
            ExecutionStatus::Passed
        } else {
            ExecutionStatus::Failed
        },
        requirement: Applicability::Required,
        environment: tools.observations(),
        command: None,
        evidence_level: outcome.is_ok().then_some(EvidenceLevel::HostRun),
        cleanup: if retained.iter().any(RetainedExecution::pending) {
            Cleanup::Failed
        } else {
            Cleanup::Complete
        },
        diagnostics,
        attachments: observations.logs[log_start..].to_vec(),
        reason: None,
    });
    outcome?;
    FinalProjectExecution::open(project)?.verify_initial()?;
    Ok(())
}
fn project_error(report: &madb_app::use_cases::project_build::ProjectReport) -> BuildFailure {
    if report.exit_code == 130 {
        return BuildFailure::Cancelled;
    }
    QueryError::new(
        match report.exit_code {
            1 => ErrorKind::CheckFailed,
            2 => ErrorKind::InvalidInput,
            4 => ErrorKind::Conflict,
            _ => ErrorKind::Host,
        },
        "EXEC010",
        "project",
        format!(
            "project {} exited {}; actual result and retained effects are in its evidence",
            report.operation.as_str(),
            report.exit_code
        ),
    )
    .into()
}
fn ordinary_run(
    report: &madb_app::use_cases::project_build::ProjectReport,
    store: &mut LocalInitEvidence,
    retained: &mut Vec<RetainedExecution>,
    logs: &mut Vec<FileIdentity>,
    control: &InitControl,
) -> BuildResult<()> {
    let root = report
        .artifact_root
        .as_ref()
        .ok_or_else(|| invalid("run", "attempt root absent"))?;
    let output = report
        .result
        .as_ref()
        .ok_or_else(|| invalid("run", "published result absent"))?;
    let initial = report
        .inputs
        .files
        .iter()
        .map(|f| madb_app::ports::FileReference {
            path: f.file.path.clone(),
            digest: f.file.sha256.clone(),
            size: f.file.size,
        })
        .collect();
    let filesystem = FinalProjectExecution::open_frozen(root, initial)?;
    let candidate = LocalWorkspace::open(
        &Path::new(root.as_str()).join(output.projection.candidate.root.as_str()),
    )?;
    let validate = || -> Result<()> {
        for f in &output.projection.outputs {
            let workspace = LocalWorkspace::open(Path::new(root.as_str()))?;
            let bytes = workspace
                .read(&f.path, f.size as usize)?
                .ok_or_else(|| invalid("run", "published output disappeared"))?;
            if bytes.len() as u64 != f.size || Digest::of_bytes(&bytes) != f.sha256 {
                return Err(invalid("run", "published output changed"));
            }
        }
        let _ = &candidate;
        Ok(())
    };
    validate()?;
    let file = capture_generated_executable(
        &Path::new(root.as_str()).join(output.projection.consumer.path.as_str()),
    )?;
    if file.digest != output.projection.consumer.sha256
        || file.size != output.projection.consumer.size
    {
        return Err(invalid("run", "consumer differs from published identity").into());
    }
    let tool = LockedInitializationTool {
        observation: ToolObservation {
            id: Id::new("i01-ordinary-consumer").unwrap(),
            version: "0.1.0".into(),
            sha256: file.digest.clone(),
            host: Id::new("aarch64-apple-darwin").unwrap(),
            target: Id::new("aarch64-apple-darwin").unwrap(),
            source: "generated://project/consumer".into(),
            arguments: vec![],
        },
        file,
    };
    let stage = control.process().map_err(init_failure)?;
    let output = filesystem.run(
        InitializationProcess {
            tool: &tool,
            arguments: vec![],
            environment: BTreeMap::new(),
            policy: ProcessPolicy::default(),
        },
        stage.process(),
        |run| {
            validate()?;
            if run.report.stdout.bytes != b"7:x\n" {
                return Err(QueryError::new(
                    ErrorKind::CheckFailed,
                    "EVD001",
                    "ordinary-run",
                    "actual stdout differs from the initial template expectation",
                ));
            }
            Ok(())
        },
    )?;
    drop(stage);
    let admission = output.execution.admission;
    retained.push(RetainedExecution::Process(output.execution));
    let RetainedExecution::Process(execution) = retained.last().unwrap() else {
        unreachable!()
    };
    let files = logs;
    if let Some(run) = &execution.process {
        files.push(evidence::save_file(
            store,
            "project/run.stdout",
            &run.report.stdout.bytes,
        )?);
        files.push(evidence::save_file(
            store,
            "project/run.stderr",
            &run.report.stderr.bytes,
        )?);
        files.push(evidence::save_file(store,"project/run.txt",format!("tool={} sha256={} size={}\nadmission={admission:?}\ntermination={:?}\ncleanup={:?}\npending={}\n",tool.file.path.display(),tool.file.digest.as_str(),tool.file.size,run.report.termination,run.report.cleanup,run.pending_cleanup.is_some()).as_bytes())?);
    }
    if let Some(error) = output.validation_error {
        return Err(error.into());
    }
    if admission == ResultAdmission::RejectedByCancellation {
        return Err(BuildFailure::Cancelled);
    }
    if admission != ResultAdmission::EligibleForValidation {
        return Err(QueryError::new(
            ErrorKind::CheckFailed,
            "EXEC010",
            "run",
            "ordinary consumer did not produce an admitted result",
        )
        .into());
    }
    validate()?;
    Ok(())
}

fn init_failure(error: AttemptError) -> BuildFailure {
    match error {
        AttemptError::Cancelled => BuildFailure::Cancelled,
        AttemptError::Stage(error) => error.into(),
    }
}
