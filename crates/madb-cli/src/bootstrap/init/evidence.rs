//! Assemble actual saved files and result observations; application predicates
//! remain authoritative for the commit receipt and final evidence inventory.
use super::catalog::{BundledSource, TrustedTemplateRecord, path};
use madb_app::ports::FileReference as PortFile;
use madb_app::use_cases::initialize::*;
use madb_app::{Digest, ErrorKind, Id, QueryError, Result};
use madb_host::initialization::LocalInitEvidence;
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{CheckLevel, ComponentIdentity, FileIdentity, FileReference};
use madb_templates::CheckedInventory;

pub(super) fn allocate() -> Result<LocalInitEvidence> {
    let parent = madb_host::locations::temporary_parent()?;
    let parent = parent.to_str().ok_or_else(|| {
        QueryError::new(
            ErrorKind::Host,
            "HOST005",
            "evidence",
            "temporary directory is not UTF-8",
        )
    })?;
    LocalInitEvidence::allocate(&AbsolutePath::new(parent)?)
}
pub(super) fn save_file(
    store: &mut LocalInitEvidence,
    name: &str,
    bytes: &[u8],
) -> Result<FileIdentity> {
    let file = FileIdentity {
        path: path(name),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    };
    store.store_file(
        &PortFile {
            path: file.path.clone(),
            digest: file.sha256.clone(),
            size: file.size,
        },
        bytes,
    )?;
    Ok(file)
}

pub(super) fn observations(
    record: &TrustedTemplateRecord,
    package: &CheckedInventory,
    project: &PreparedProject,
    tool: &ComponentIdentity,
    store: &mut LocalInitEvidence,
    committed: Option<&CommittedProject>,
) -> Result<SuccessObservations> {
    let request = project.request();
    let archive = store_file_identity(
        store,
        "template-package.tar",
        record.bytes(),
        &record.archive,
    )?;
    for entry in package.archive().entries() {
        let original = entry.identity();
        let name = format!("template-input/{}", original.path.as_str());
        store_file_identity(
            store,
            &name,
            package
                .archive()
                .entry_bytes(&original.path)
                .expect("checked entry"),
            original,
        )?;
    }
    let input_root = path("template-input");
    let mut manifest = record.manifest.clone();
    manifest.file.path = path(&format!("template-input/{}", manifest.file.path.as_str()));
    let mut boundary_profiles: Vec<FileReference> = request
        .selected()
        .template()
        .boundary_refs
        .iter()
        .filter(|b| request.selected().profile().boundary_refs.contains(&b.id))
        .map(|b| {
            let mut p = b.profile.clone();
            p.file.path = path(&format!("template-input/{}", p.file.path.as_str()));
            p
        })
        .collect();
    boundary_profiles.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    let source = match record.source {
        BundledSource::Bundled => "bundled",
    };
    let mut installation = format!(
        "Trusted installation catalog\nsupported initialization: structure, dry-run\nunavailable initialization execution: build, run, open\ntemplate={}@{}\narchive={} {}\nmanifest={} {}\nsource={}\n",
        record.id,
        record.version,
        record.archive.sha256.as_str(),
        record.archive.size,
        record.manifest.file.sha256.as_str(),
        record.manifest.file.size,
        source
    );
    for recipe in &record.recipes {
        installation.push_str(&format!(
            "recipe={} version={} image={}\n",
            recipe.id,
            recipe.version,
            recipe.sha256.as_str()
        ));
    }
    let catalog = save_file(store, "installation.txt", installation.as_bytes())?;
    let mut checks = Vec::new();
    if !request.projection().dry_run {
        if committed.is_none() {
            return Err(QueryError::new(
                ErrorKind::CheckFailed,
                "EVD001",
                "structure",
                "structure evidence requires a verified CommitNew receipt",
            ));
        }
        let mut structure = format!(
            "Initial complete source inventory verified after CommitNew.\nrequest={}\nroot={}\n",
            request.digest().as_str(),
            project.root().as_str()
        );
        for file in project.references() {
            structure.push_str(&format!(
                "{} {} {}\n",
                file.path.as_str(),
                file.digest.as_str(),
                file.size
            ));
        }
        let proof = save_file(store, "structure.txt", structure.as_bytes())?;
        for requirement in request
            .selected()
            .required_checks(CheckLevel::Structure)
            .map_err(|e| {
                QueryError::new(ErrorKind::InvalidInput, "PLAN001", e.subject, e.message)
            })?
        {
            if requirement.id.as_str() != "structure" {
                return Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "PLAN001",
                    requirement.id.as_str(),
                    "selected required check has no installed structure implementation",
                ));
            }
            checks.push(CheckObservation {
                id: requirement.id,
                level: CheckLevel::Structure,
                target: requirement.target,
                status: ExecutionStatus::Passed,
                requirement: Applicability::Required,
                environment: Vec::new(),
                command: None,
                evidence_level: Some(EvidenceLevel::Static),
                cleanup: Cleanup::NotRun,
                diagnostics: Vec::new(),
                attachments: vec![proof.clone()],
                reason: None,
            });
        }
    }
    Ok(SuccessObservations {
        tool: tool.clone(),
        template: TemplateObservation {
            id: record.id.clone(),
            version: record.version,
            manifest,
            package_sha256: record.archive.sha256.clone(),
            input_root,
        },
        template_package: archive,
        evidence_root: store.root().clone(),
        boundary_profiles,
        checks,
        open: OpenObservation::skipped(request.projection().open.clone()),
        diagnostics: Vec::new(),
        logs: vec![catalog],
    })
}
fn store_file_identity(
    store: &mut LocalInitEvidence,
    name: &str,
    bytes: &[u8],
    original: &FileIdentity,
) -> Result<FileIdentity> {
    let file = save_file(store, name, bytes)?;
    if file.sha256 != original.sha256 || file.size != original.size {
        return Err(QueryError::new(
            ErrorKind::Conflict,
            "FILE001",
            name,
            "frozen template bytes changed before evidence storage",
        ));
    }
    Ok(file)
}

pub(super) fn empty_report(tool: &ComponentIdentity) -> InitReport {
    InitReport {
        request_digest: None,
        tool: tool.clone(),
        template: None,
        template_package: None,
        evidence_root: None,
        template_profile: None,
        boundary_profiles: Vec::new(),
        request: None,
        output: None,
        plan: None,
        creation: Creation::NotCreated,
        termination: Termination::Failed,
        exit_code: 3,
        checks: Vec::new(),
        ide_entries: Vec::new(),
        open: OpenObservation::skipped(None),
        recovery: Vec::new(),
        diagnostics: Vec::new(),
        logs: Vec::new(),
    }
}
pub(super) fn bounded_error(mut error: QueryError) -> QueryError {
    for finding in &mut error.findings {
        let mut boundary = finding.message.len().min(8192);
        while !finding.message.is_char_boundary(boundary) {
            boundary -= 1;
        }
        finding.message.truncate(boundary);
        if finding.message.is_empty() {
            finding.message = "initialization stage failed".into();
        }
    }
    error
}
pub(super) fn append_failure(report: &mut InitReport, error: QueryError) {
    let error = bounded_error(error);
    for finding in error.findings {
        report.diagnostics.push(Diagnostic {
            code: Id::new(finding.code).unwrap_or_else(|_| Id::new("PLAN001").expect("fixed code")),
            message: finding.message,
            location: finding.locations.into_iter().next(),
        });
    }
}
