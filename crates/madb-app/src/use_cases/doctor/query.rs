use super::*;
use crate::ports::{ConfigurationBytes, ReadWorkspace, SnapshotFormat};
use crate::query::{DOCUMENT_LIMIT, Reads, SOURCE_LIMIT, matches};
use crate::{Digest, ErrorKind, LogicalPath, QueryError, Result};

/// Pure projection of the same adapter-owned prepared state used by check.
/// `project` and `request` identify the bytes actually captured by this call.
pub trait DoctorFormat: SnapshotFormat {
    fn context(
        &self,
        prepared: &Self::Prepared,
        project: FileIdentity,
        encoding: ProjectEncoding,
        request: FileIdentity,
    ) -> Result<Context>;
    fn sdk_reference(&self, prepared: &Self::Prepared) -> Result<FileReference>;
}
pub struct CheckedConfiguration {
    pub profile_bytes: Vec<u8>,
    pub sdk: FileReference,
}
/// Keeps all actual observations until the caller finishes environment reads.
/// Even failed queries recheck what they managed to read before returning.
pub struct DoctorQuery<'a, W> {
    reads: Reads<'a, W>,
}
impl<'a, W: ReadWorkspace> DoctorQuery<'a, W> {
    pub fn new(workspace: &'a W) -> Self {
        Self {
            reads: Reads::new(workspace),
        }
    }
    pub fn check<F: DoctorFormat>(
        &mut self,
        format: &F,
        request: &Request,
        report: &mut Report,
    ) -> Option<CheckedConfiguration> {
        let mut stage = "context.configuration";
        let result = (|| -> Result<Option<CheckedConfiguration>> {
            let project = self.reads.required(&request.project, DOCUMENT_LIMIT)?;
            let read = self.reads.required(&request.read_request, DOCUMENT_LIMIT)?;
            let refs = format.request_files(&read)?;
            let lock = self.reads.required(&refs.lock.path, DOCUMENT_LIMIT)?;
            let profile = self.reads.required(&refs.profile.path, DOCUMENT_LIMIT)?;
            for (reference, bytes) in [(&refs.lock, &lock), (&refs.profile, &profile)] {
                if !matches(reference, bytes) {
                    return Err(QueryError::new(
                        ErrorKind::InvalidInput,
                        "FILE004",
                        reference.path.as_str(),
                        "configuration bytes differ from the read request; prepare matching inputs explicitly",
                    ));
                }
            }
            let (prepared, tracked) = format.prepare(ConfigurationBytes {
                request: &read,
                project: &project,
                lock: &lock,
                profile: &profile,
            })?;
            report.context = Some(format.context(
                &prepared,
                identity(&request.project, &project),
                if project.iter().find(|v| !v.is_ascii_whitespace()) == Some(&b'{') {
                    ProjectEncoding::Json
                } else {
                    ProjectEncoding::Toml
                },
                identity(&request.read_request, &read),
            )?);
            stage = "context.declared-inputs";
            let mut stale = Vec::new();
            for reference in tracked {
                let bytes = self.reads.optional(&reference.path, SOURCE_LIMIT)?;
                if bytes.as_ref().is_none_or(|v| !matches(&reference, v)) && stale.len() < 16 {
                    stale.push(crate::Finding::new(
                        "META001",
                        reference.path.as_str(),
                        "tracked input is missing or changed; metadata preparation is required",
                    ));
                }
            }
            if !stale.is_empty() {
                return Err(QueryError {
                    kind: ErrorKind::InvalidInput,
                    findings: stale,
                });
            }
            report.record(Check::passed(
                stage,
                CheckKind::Input,
                "declared input inventory",
                None,
                Fact::State(State::Checked),
            ));
            stage = "context.snapshot";
            let snapshot = self
                .reads
                .optional(&request.snapshot, DOCUMENT_LIMIT)?
                .ok_or_else(|| {
                    QueryError::new(
                        ErrorKind::InvalidInput,
                        "META001",
                        request.snapshot.as_str(),
                        "snapshot is missing; metadata preparation is required",
                    )
                })?;
            if let Err(mut error) = format.check(&prepared, &snapshot) {
                if error
                    .findings
                    .iter()
                    .any(|f| f.code == "LOCK005" || f.code == "PORT002")
                {
                    error.kind = ErrorKind::InvalidInput;
                }
                return Err(error);
            }
            let context = report.context.as_mut().expect("prepared context");
            context.snapshot = Some(FileReference {
                file: identity(&request.snapshot, &snapshot),
                schema: id("madb.contract-ir/1"),
            });
            report.record(Check::passed(
                stage,
                CheckKind::Input,
                request.snapshot.as_str(),
                None,
                Fact::bytes(&snapshot),
            ));
            report.record(Check::passed(
                "context.configuration",
                CheckKind::Configuration,
                "complete project/boundary/profile/lock/IR binding",
                None,
                Fact::State(State::Checked),
            ));
            Ok(Some(CheckedConfiguration {
                profile_bytes: profile,
                sdk: format.sdk_reference(&prepared)?,
            }))
        })();
        match result {
            Ok(value) => value,
            Err(error) => {
                let kind = if stage == "context.configuration" {
                    CheckKind::Configuration
                } else {
                    CheckKind::Input
                };
                let mut failed = Check::failed(stage, kind, stage, &error);
                if stage == "context.snapshot" && error.findings.iter().any(|f| f.code == "META001")
                {
                    failed.observed = Some(Fact::State(State::Missing));
                }
                report.record(failed);
                match stage {
                    "context.configuration" => {
                        report.context = None;
                        report.record(Check::blocked(
                            "context.declared-inputs",
                            CheckKind::Input,
                            stage,
                        ));
                        report.record(Check::blocked("context.snapshot", CheckKind::Input, stage));
                    }
                    "context.declared-inputs" => {
                        report.record(Check::blocked("context.snapshot", CheckKind::Input, stage));
                        report.record(Check::blocked(
                            "context.configuration",
                            CheckKind::Configuration,
                            "context.snapshot",
                        ));
                    }
                    _ => report.record(Check::blocked(
                        "context.configuration",
                        CheckKind::Configuration,
                        stage,
                    )),
                }
                None
            }
        }
    }
    pub fn checked_file(&mut self, reference: &FileReference) -> Result<Vec<u8>> {
        let bytes = self.reads.required(&reference.file.path, DOCUMENT_LIMIT)?;
        if identity(&reference.file.path, &bytes) != reference.file {
            return Err(QueryError::new(
                ErrorKind::InvalidInput,
                "FILE004",
                reference.file.path.as_str(),
                "SDK document differs from the exact profile reference",
            ));
        }
        Ok(bytes)
    }
    pub fn revalidate(&self, report: &mut Report) {
        if let Err(error) = self.reads.recheck() {
            report.record(Check::failed(
                "context.configuration",
                CheckKind::Configuration,
                "captured query inputs",
                &error,
            ));
        }
    }
}
fn identity(path: &LogicalPath, bytes: &[u8]) -> FileIdentity {
    FileIdentity {
        path: path.clone(),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn id(value: &str) -> crate::Id {
    crate::Id::new(value).expect("fixed id")
}
