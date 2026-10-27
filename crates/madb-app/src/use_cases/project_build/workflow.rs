//! The finite use case owns ordering and observed facts; adapters own effects.
use super::*;
use crate::{Digest, ErrorKind, Id, QueryError};

impl ProjectReport {
    pub fn initialize_stages(&mut self) {
        self.stages = self
            .operation
            .stages()
            .iter()
            .map(|&id| StageObservation {
                id,
                status: StageStatus::Skipped,
                diagnostics: Vec::new(),
                attachments: Vec::new(),
                blocked_by: Vec::new(),
            })
            .collect();
    }
    pub fn stage_passed(&mut self, id: StageId) {
        if let Some(stage) = self.stages.iter_mut().find(|s| s.id == id) {
            stage.status = StageStatus::Passed;
        }
    }
    pub fn record_failure(&mut self, id: StageId, failure: BuildFailure) {
        let (code, termination, diagnostic, status) = match failure {
            BuildFailure::Cancelled => (130, Termination::Cancelled, Diagnostic { code: Id::new("EXEC001").unwrap(),
                message: "Cancellation rejected admission before publication or installation was accepted.".into(), location: None }, StageStatus::Failed),
            BuildFailure::Error(error) => {
                let code = match error.kind { ErrorKind::CheckFailed => 1, ErrorKind::InvalidInput => 2, ErrorKind::Host => 3, ErrorKind::Conflict => 4 };
                let status = if error.kind == ErrorKind::Host { StageStatus::Error } else { StageStatus::Failed };
                (code, Termination::Failed, error_diagnostic(&error), status)
            }
        };
        self.exit_code = code;
        self.termination = termination;
        self.diagnostics.push(diagnostic.clone());
        let mut later = false;
        for stage in &mut self.stages {
            if stage.id == id {
                stage.status = status;
                stage.diagnostics.push(diagnostic.clone());
                later = true;
            } else if later {
                stage.status = StageStatus::Blocked;
                stage.blocked_by = vec![id];
            }
        }
    }
    /// A later evidence/output error cannot erase an already accepted effect.
    pub fn evidence_failed(&mut self, error: QueryError) {
        self.exit_code = 3;
        self.termination = Termination::Failed;
        self.diagnostics.push(error_diagnostic(&error));
        if self.published != Some(false) {
            self.publication_durability = Durability::Uncertain;
            if let Some(stage) = self.stages.iter_mut().find(|s| s.id == StageId::Publish) {
                stage.status = StageStatus::Error;
            }
        }
    }
}
fn error_diagnostic(error: &QueryError) -> Diagnostic {
    let first = error.findings.first();
    Diagnostic {
        code: first
            .and_then(|f| Id::new(f.code).ok())
            .unwrap_or_else(|| Id::new("HOST003").unwrap()),
        message: first.map_or_else(
            || "The operation failed without a diagnostic.".into(),
            |f| {
                let mut end = f.message.len().min(2048);
                while !f.message.is_char_boundary(end) {
                    end -= 1;
                }
                f.message[..end].to_owned()
            },
        ),
        location: None,
    }
}
fn check_tail(
    format: &dyn ProjectBuildFormat,
    prospective: &ProjectReport,
    stage: StageId,
) -> crate::Result<()> {
    let mut worst = prospective.clone();
    let error = || QueryError::new(ErrorKind::Host, "HOST003", "result", "\0".repeat(2048));
    worst.record_failure(stage, error().into());
    for _ in 0..3 {
        worst.evidence_failed(error());
    }
    for report in [prospective, &worst] {
        let bytes = format.encode_report(report).map_err(|e| {
            QueryError::new(
                ErrorKind::CheckFailed,
                "EVD001",
                "result",
                format!("result including bounded failure tail cannot be encoded: {e}"),
            )
        })?;
        if bytes.len() > MAX_WIRE_BYTES - MAX_SECTION_BYTES {
            return Err(QueryError::new(
                ErrorKind::CheckFailed,
                "EVD001",
                "result",
                "complete project result exceeds the pre-publication envelope including reserved failure tail",
            ));
        }
    }
    Ok(())
}

/// Evidence allocation and final encoding belong to the composition root. The
/// caller supplies one report with its actual evidence root before execution.
pub fn run_project(
    workspace: &mut dyn ProjectWorkspace,
    format: &dyn ProjectBuildFormat,
    execution: &mut dyn ProjectBuildExecution,
    control: &dyn ProjectBuildControl,
    operation: Operation,
    evidence_root: AbsolutePath,
) -> ProjectReport {
    // A decoded wire report cannot seed a new execution. The app owns a fresh
    // finite observation stream and its reserved terminal diagnostic slots.
    let mut report = ProjectReport::empty(operation);
    report.evidence_root = Some(evidence_root);
    report.initialize_stages();
    report.root = Some(workspace.root().clone());
    let mut current = StageId::Capture;
    let result = (|| -> BuildResult<()> {
        control.checkpoint()?;
        let original = workspace.descriptor_bytes()?;
        let descriptor = format.decode_descriptor(&original)?;
        let descriptor_file = FileIdentity {
            path: crate::LogicalPath::new(DESCRIPTOR).unwrap(),
            sha256: Digest::of_bytes(&original),
            size: original.len() as u64,
        };
        report.recipe = Some(Id::new(RECIPE).unwrap());
        report.request_digest = Some(format.request_digest(&RequestIdentity {
            operation: report.operation,
            root: workspace.root().clone(),
            descriptor: descriptor_file.clone(),
        })?);
        workspace.check_origin(&descriptor, &original)?;
        let captured = if report.operation == Operation::Build {
            let captured = workspace.capture(&descriptor, &original)?;
            let snapshot = format.input_digest(&descriptor_file, &captured.inventory()?)?;
            Some((captured, snapshot))
        } else {
            None
        };
        report.stage_passed(current);
        current = StageId::Preflight;
        control.checkpoint()?;
        execution.preflight(report.operation, &descriptor)?;
        report.tool = Some(descriptor.tool.clone());
        workspace.revalidate()?;
        report.stage_passed(current);
        if let Some((captured, snapshot)) = captured {
            current = StageId::Prepare;
            control.checkpoint()?;
            let attempt = match workspace.begin_attempt(&captured) {
                Ok(attempt) => attempt,
                Err(error) => {
                    report.artifact_root = workspace.created_attempt_root().cloned();
                    return Err(error.into());
                }
            };
            report.artifact_root = Some(attempt.root.clone());
            report.inputs = attempt.inputs.clone();
            report.previous = attempt.previous.clone();
            let output = match execution.execute(
                &descriptor,
                &attempt,
                &snapshot,
                &mut ExecutionProgress {
                    report: &mut report,
                },
            ) {
                Ok(output) => output,
                Err(error) => {
                    current = report
                        .stages
                        .iter()
                        .find(|s| s.status != StageStatus::Passed)
                        .map_or(current, |s| s.id);
                    return Err(error);
                }
            };
            current = StageId::Publish;
            control.checkpoint()?;
            workspace.revalidate()?;
            if output.identity != format.output_digest(&descriptor.tool, &output.projection)? {
                return Err(invalid(
                    "output",
                    "execution output identity differs from its exact projection",
                )
                .into());
            }
            workspace.verify_outputs(&attempt, &output)?;
            // Validate the complete success report before the irreversible
            // current-index admission, reserving a full bounded error tail.
            let mut prospective = report.clone();
            prospective.published = Some(true);
            prospective.publication_durability = Durability::Confirmed;
            prospective.result = Some(output.clone());
            prospective.stage_passed(StageId::Publish);
            prospective.termination = Termination::Complete;
            prospective.exit_code = 0;
            check_tail(format, &prospective, StageId::Publish)?;
            let observed = workspace.publish(&attempt, &output, control.commit());
            report.published = observed.published;
            report.publication_durability = observed.durability;
            if observed.published == Some(true) {
                report.result = Some(output);
            }
            if let Some(error) = observed.error {
                if observed.published == Some(false) && control.commit().rejected_by_cancellation()
                {
                    return Err(BuildFailure::Cancelled);
                }
                // The published/uncertain terminal window always reports a Host failure.
                if observed.published != Some(false) {
                    return Err(QueryError {
                        kind: ErrorKind::Host,
                        findings: error.findings,
                    }
                    .into());
                }
                return Err(error.into());
            }
            if observed.published != Some(true) {
                return Err(QueryError::new(
                    ErrorKind::Host,
                    "HOST003",
                    "publish",
                    "publication did not establish a confirmed effect",
                )
                .into());
            }
        } else {
            current = StageId::Install;
            control.checkpoint()?;
            workspace.revalidate()?;
            let mut prospective = report.clone();
            prospective.installed_tool = Some(InstalledTool {
                path: AbsolutePath::new(&format!(
                    "{}/.madb/wrapper/madb",
                    workspace.root().as_str()
                ))?,
                sha256: descriptor.tool.sha256.clone(),
                size: descriptor.tool.size,
            });
            prospective.stage_passed(StageId::Install);
            prospective.termination = Termination::Complete;
            prospective.exit_code = 0;
            check_tail(format, &prospective, StageId::Install)?;
            let observed = workspace.install_tool(&descriptor, control.commit());
            report.installed_tool = observed.installed;
            if let Some(error) = observed.error {
                if report.installed_tool.is_none() && control.commit().rejected_by_cancellation() {
                    return Err(BuildFailure::Cancelled);
                }
                if report.installed_tool.is_some() {
                    return Err(QueryError {
                        kind: ErrorKind::Host,
                        findings: error.findings,
                    }
                    .into());
                }
                return Err(error.into());
            }
            if report.installed_tool.is_none() {
                return Err(QueryError::new(
                    ErrorKind::Host,
                    "HOST003",
                    "install",
                    "installation did not establish an accepted image",
                )
                .into());
            }
        }
        report.stage_passed(current);
        report.termination = Termination::Complete;
        report.exit_code = 0;
        Ok(())
    })();
    if let Err(error) = result {
        report.record_failure(current, error);
    }
    report
}

/// Execution adapters can append actual log identities and mark fixed stages.
/// They cannot inject or rewrite public result facts or consume terminal slots.
pub struct ExecutionProgress<'a> {
    report: &'a mut ProjectReport,
}
impl ExecutionProgress<'_> {
    pub fn stage_passed(&mut self, stage: StageId) {
        self.report.stage_passed(stage);
    }
    pub fn record_log(&mut self, file: FileIdentity) -> crate::Result<()> {
        if self.report.logs.len() >= 512
            || self
                .report
                .logs
                .iter()
                .map(|f| f.path.as_str().len() * 6 + 160)
                .sum::<usize>()
                + file.path.as_str().len() * 6
                + 160
                > 120 * 1024
        {
            return Err(QueryError::new(
                ErrorKind::CheckFailed,
                "EVD001",
                "logs",
                "project log index exceeds its reserved metadata budget; raw evidence remains retained",
            ));
        }
        self.report.logs.push(file);
        Ok(())
    }
}
