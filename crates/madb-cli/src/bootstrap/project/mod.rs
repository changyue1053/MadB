//! One composition entry shared by the CLI, wrapper and approved initializer.
pub(super) mod control;
mod evidence;
pub(super) mod initialize;
mod recipe;
use super::execution::RetainedExecution;
use crate::project::ProjectOptions;
use control::{BuildControl, install_signals};
use madb_app::{Digest, ErrorKind, QueryError, Result, use_cases::project_build::*};
use madb_contracts::project_build::ProjectBuildAdapter;
use madb_host::{initialization_source::normalize_output, project_build::LocalProjectBuild};

pub struct ProjectService;
impl ProjectService {
    pub fn run(
        operation: Operation,
        options: Result<ProjectOptions>,
    ) -> Result<(ProjectReport, Vec<u8>)> {
        let control = BuildControl::default();
        Self::run_inner(operation, options, &control, true)
    }
    pub(in crate::bootstrap) fn run_controlled(
        operation: Operation,
        options: Result<ProjectOptions>,
        control: &BuildControl,
    ) -> Result<(ProjectReport, Vec<u8>)> {
        Self::run_inner(operation, options, control, false)
    }
    fn run_inner(
        operation: Operation,
        options: Result<ProjectOptions>,
        control: &BuildControl,
        standalone: bool,
    ) -> Result<(ProjectReport, Vec<u8>)> {
        let format = ProjectBuildAdapter::new()?;
        let mut report = ProjectReport::empty(operation);
        let options = match options {
            Ok(v) => v,
            Err(e) => {
                report.record_failure(StageId::Capture, BuildFailure::Error(e));
                let bytes = format.encode_report(&report)?;
                return Ok((report, bytes));
            }
        };
        let mut retained = Vec::new();
        let mut store = None;
        let attempt = (|| -> Result<()> {
            let _signals = if standalone {
                Some(install_signals(control)?)
            } else {
                None
            };
            let root = normalize_output(&options.root)?;
            report.root = Some(root.clone());
            let mut workspace = LocalProjectBuild::open(&root)?;
            let evidence = evidence::allocate()?;
            report.evidence_root = Some(evidence.root().clone());
            store = Some(evidence);
            let mut execution = recipe::NativeExecution::new(
                control,
                store.as_mut().expect("allocated evidence"),
                &mut retained,
            );
            report = run_project(
                &mut workspace,
                &format,
                &mut execution,
                control,
                operation,
                report
                    .evidence_root
                    .clone()
                    .expect("allocated evidence root"),
            );
            Ok(())
        })();
        if let Err(error) = attempt {
            report.record_failure(StageId::Capture, error.into());
        }
        recovery(&mut report);
        if let Some(store) = &mut store {
            if let Some(bytes) = settle_owners(&mut retained) {
                match evidence::save_file(store, "cleanup.txt", &bytes) {
                    Ok(file) => report.logs.push(file),
                    Err(error) => report.evidence_failed(error),
                }
            }
            // Owners survive all result persistence below; a pending cleanup is
            // never converted into a passing build.
            if retained.iter().any(RetainedExecution::pending) {
                report.evidence_failed(QueryError::new(
                    ErrorKind::Host,
                    "EXEC010",
                    "cleanup",
                    "owned descendant cleanup remains incomplete",
                ));
            }
            let saved = (|| -> Result<()> {
                let bytes = format.encode_report(&report)?;
                let root = store.root().clone();
                let saved = store.save_project_result(&root, &bytes, &report.logs)?;
                let result = saved
                    .files
                    .iter()
                    .find(|f| f.path.as_str() == "project-result.json")
                    .ok_or_else(|| invalid("evidence", "saved result is absent"))?;
                if result.sha256 != Digest::of_bytes(&bytes) || result.size != bytes.len() as u64 {
                    return Err(invalid(
                        "evidence",
                        "saved result differs from emitted bytes",
                    ));
                }
                Ok(())
            })();
            if let Err(error) = saved {
                report.evidence_failed(error);
            }
        }
        recovery(&mut report);
        let bytes = format.encode_report(&report)?;
        Ok((report, bytes))
    }
}
fn recovery(report: &mut ProjectReport) {
    if report.exit_code == 0 || !report.recovery.is_empty() {
        return;
    }
    let (kind, message) = if report.published != Some(false) {
        (
            "inspect",
            "Publication was accepted or remains uncertain. Inspect the recorded artifact_root, result/previous identities, evidence_root, and the current index under root/.madb/project-state before consuming or retrying. Preserve the retained attempts; this command does not roll back or repair the index.",
        )
    } else if report.installed_tool.is_some() {
        (
            "inspect",
            "The image at installed_tool.path was accepted. Preserve that image and inspect the recorded evidence_root and diagnostic; a later error does not mean installation was undone.",
        )
    } else if report.artifact_root.is_some() {
        (
            "retry",
            "The retained artifact_root identifies this failed attempt. Inspect its recorded diagnostics and evidence_root, correct the author input or missing prerequisite, then invoke the same frozen project build again. The previous complete batch is preserved.",
        )
    } else {
        (
            "manual",
            "Inspect the diagnostic and recorded root. Resolve the stated source, ownership, or installation mismatch before retrying; existing foreign bytes are not overwritten by this command.",
        )
    };
    report.recovery.push(Recovery {
        kind: kind.into(),
        message: message.into(),
        command: None,
    });
}
pub(super) fn source_revision() -> Result<&'static str> {
    option_env!("MADB_SOURCE_REVISION")
        .filter(|v| {
            v.len() == 40
                && v.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        .ok_or_else(|| {
            QueryError::new(
                ErrorKind::Host,
                "HOST005",
                "tool.source",
                "this compiled image lacks an exact source revision",
            )
        })
}
pub(super) fn bootstrap_check() -> Result<()> {
    use madb_host::initialization_execution::LockedInitializationFile;
    use madb_host::initialization_source::{
        observe_initialization_host, observe_initialization_system_build,
    };
    let host = observe_initialization_host()?;
    if host.os.as_str() != "macos"
        || host.arch.as_str() != "aarch64"
        || host.version.to_string() != "26.6.2"
        || observe_initialization_system_build()? != "25G83"
    {
        return Err(invalid(
            "bootstrap",
            "this finite project launcher requires the fixed macOS 26.6.2 (25G83) aarch64 installation",
        ));
    }
    for (name, sha, size) in [
        (
            "/bin/sh",
            "c0eaf44f9242d5bbc2e14f4e8b7dccc1eff7f2976d64dc18914d5ef9f373e100",
            101040,
        ),
        (
            "/usr/bin/env",
            "75690864f0e7397db05bcc0f4439915559ce24c2d834d530e4e619c14b938556",
            102176,
        ),
        (
            "/usr/bin/plutil",
            "3d4086a77e9bc2eb3eab7dc707ff844a8949b78c80daa2ffaa2e643e022c1c2f",
            661952,
        ),
        (
            "/usr/bin/openssl",
            "f81b4fcec5048ea1eceb40fc08f4e06f1668181f0b5bacfa8be27d0da181dcb7",
            1130832,
        ),
    ] {
        LockedInitializationFile {
            path: name.into(),
            digest: Digest::new(format!("sha256:{sha}")).expect("fixed hash"),
            size,
        }
        .verify()?;
    }
    Ok(())
}
fn invalid(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
fn settle_owners(owners: &mut [RetainedExecution]) -> Option<Vec<u8>> {
    if !owners.iter().any(RetainedExecution::pending) {
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

#[cfg(test)]
mod tests;
