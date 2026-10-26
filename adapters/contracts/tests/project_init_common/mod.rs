#![allow(dead_code)]
//! Explicit in-memory models; no Host filesystem or initialization command runs.
use madb_app::use_cases::initialize::*;
use madb_app::{Digest, Id, LogicalPath, Result};
use madb_contracts::project_init::ProjectInitAdapter;
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{
    CheckLevel, ComponentIdentity, FileIdentity, FileOwnership, FileReference,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/project-init")
}
pub fn read(path: &str) -> Vec<u8> {
    std::fs::read(root().join(path)).unwrap()
}
pub fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
pub fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
pub fn file(name: &str, bytes: &[u8]) -> FileIdentity {
    FileIdentity {
        path: path(name),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
pub fn adapter() -> ProjectInitAdapter {
    ProjectInitAdapter::new().unwrap()
}
pub fn request(dry: bool) -> InitRequest {
    InitRequest {
        template_selector: Some("org.madb.template/rust-c-sdk@0.1.0".into()),
        template_profile_selector: Some(id("macos-arm64-values")),
        parameter_values: BTreeMap::new(),
        targets: None,
        verify: VerifyLevel::Structure,
        open: None,
        dry_run: dry,
        offline: true,
        output: AbsolutePath::new("/model/中文 hello").unwrap(),
    }
}
pub fn prepared(dry: bool) -> (ProjectInitAdapter, FrozenInitRequest, PreparedProject) {
    let a = adapter();
    let t = a
        .decode_template(&read("package-input/meta/template.json"))
        .unwrap();
    let mut r = request(dry);
    r.parameter_values.insert(
        id("name"),
        madb_plan::project::ParameterValue::String("model_demo".into()),
    );
    let (frozen, _) = freeze_request(&t, r, "0.1.0", &a).unwrap();
    let manifest = file(
        "meta/template.json",
        &read("package-input/meta/template.json"),
    );
    let package = file("template-design.tar", &read("template-design.tar"));
    let plan = frozen.selected().plan_with_provenance(&manifest).unwrap();
    let files = plan
        .files()
        .iter()
        .map(|p| {
            if p.ownership == FileOwnership::LocalState {
                return InitialFile::local_state(p.module.clone(), p.path.clone());
            }
            let bytes = if p.path.as_str() == ".madb/template-package.tar" {
                read("template-design.tar")
            } else if let Some(original) = p.path.as_str().strip_prefix(".madb/template-input/") {
                read(&format!("package-input/{original}"))
            } else {
                b"explicitly modelled rendered source; not compiled\n".to_vec()
            };
            InitialFile::rendered(p.module.clone(), p.path.clone(), p.ownership, bytes).unwrap()
        })
        .collect();
    let prepared = PreparedProject::new(&frozen, manifest, package, files).unwrap();
    (a, frozen, prepared)
}
pub fn observations(request: &FrozenInitRequest, project: &PreparedProject) -> SuccessObservations {
    let input_root = path("template-input");
    let mut manifest = project.manifest().clone();
    manifest.path = path(&format!("template-input/{}", manifest.path.as_str()));
    let mut boundary_profiles: Vec<_> = request
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
    SuccessObservations {
        tool: ComponentIdentity {
            id: id("org.madb/model-host"),
            version: "0.1.0".into(),
            sha256: Digest::of_bytes(b"model-host"),
        },
        template: TemplateObservation {
            id: request.selected().template().id.clone(),
            version: request.selected().template().version,
            manifest: FileReference {
                file: manifest,
                schema: id("madb.project-template/1"),
            },
            package_sha256: project.package().sha256.clone(),
            input_root,
        },
        template_package: file("template-package.tar", &read("template-design.tar")),
        evidence_root: AbsolutePath::new("/model/evidence").unwrap(),
        boundary_profiles,
        checks: if request.projection().dry_run {
            Vec::new()
        } else {
            vec![CheckObservation {
                id: id("structure"),
                level: CheckLevel::Structure,
                target: id("mac-arm64"),
                status: ExecutionStatus::Passed,
                requirement: Applicability::Required,
                environment: Vec::new(),
                command: None,
                evidence_level: Some(EvidenceLevel::Static),
                cleanup: Cleanup::NotRun,
                diagnostics: Vec::new(),
                attachments: vec![file("model.log", b"model-only")],
                reason: None,
            }]
        },
        open: OpenObservation::skipped(None),
        diagnostics: Vec::new(),
        logs: Vec::new(),
    }
}
pub struct MockCommit;
impl CommitNewPort for MockCommit {
    fn commit_new(&mut self, p: &PreparedProject) -> Result<HostCommitObservation> {
        Ok(HostCommitObservation {
            root: p.root().clone(),
            files: Ok(p.references()),
        })
    }
}
/// This intentionally echoes a model inventory. Production Host must read actual
/// saved bytes; this replacement establishes only application predicate behavior.
pub struct MockEvidence {
    pub calls: usize,
    pub corrupt: bool,
}
impl InitEvidencePort for MockEvidence {
    fn save_result(
        &mut self,
        root: &AbsolutePath,
        bytes: &[u8],
        refs: &[FileIdentity],
    ) -> Result<SavedEvidenceObservation> {
        self.calls += 1;
        let mut files = refs.to_vec();
        files.push(file(
            "init-result.json",
            if self.corrupt { b"wrong" } else { bytes },
        ));
        Ok(SavedEvidenceObservation {
            root: root.clone(),
            files,
        })
    }
}
pub fn sink() -> MockEvidence {
    MockEvidence {
        calls: 0,
        corrupt: false,
    }
}
