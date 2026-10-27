//! Pure format and use-case counterexamples; these do not certify a native SDK.
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result, transaction::CommitControl,
    use_cases::project_build::*,
};
use madb_contracts::project_build::ProjectBuildAdapter;
use serde_json::Value;
use std::collections::BTreeMap;
fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn absolute(s: &str) -> AbsolutePath {
    AbsolutePath::new(s).unwrap()
}
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../tests/fixtures/project-build/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn file(v: &Value) -> FileIdentity {
    FileIdentity {
        path: path(v["path"].as_str().unwrap()),
        sha256: Digest::new(v["sha256"].as_str().unwrap()).unwrap(),
        size: v["size"].as_u64().unwrap(),
    }
}
fn dir(v: &Value) -> InputDirectory {
    InputDirectory {
        path: path(v["path"].as_str().unwrap()),
        present: v["present"].as_bool().unwrap(),
        members: v["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| DirectoryMember {
                path: path(m["path"].as_str().unwrap()),
                kind: if m["kind"] == "file" {
                    MemberKind::File
                } else {
                    MemberKind::Directory
                },
            })
            .collect(),
    }
}
#[test]
fn typed_request_and_input_projections_match_frozen_known_hashes() {
    let format = ProjectBuildAdapter::new().unwrap();
    let vectors: Value = serde_json::from_slice(&fixture("digest-vectors.json")).unwrap();
    for v in vectors["vectors"].as_array().unwrap() {
        let p = &v["projection"];
        let digest = match v["domain"].as_str().unwrap() {
            "madb.project-build-request/1" => format
                .request_digest(&RequestIdentity {
                    operation: if p["operation"] == "build" {
                        Operation::Build
                    } else {
                        Operation::InstallTool
                    },
                    root: absolute(p["root"].as_str().unwrap()),
                    descriptor: file(&p["descriptor"]),
                })
                .unwrap(),
            "madb.project-build-input/1" => {
                let inputs = InputInventory {
                    files: p["files"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| InputFile {
                            source: path(f["source"].as_str().unwrap()),
                            file: FileIdentity {
                                path: path(&format!("inputs/{}", f["source"].as_str().unwrap())),
                                sha256: Digest::new(f["sha256"].as_str().unwrap()).unwrap(),
                                size: f["size"].as_u64().unwrap(),
                            },
                        })
                        .collect(),
                    directories: p["directories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(dir)
                        .collect(),
                };
                format
                    .input_digest(&file(&p["descriptor"]), &inputs)
                    .unwrap()
            }
            "madb.project-build-output/1" => {
                let tool = &p["tool"];
                format
                    .output_digest(
                        &ToolIdentity {
                            id: Id::new(tool["id"].as_str().unwrap()).unwrap(),
                            version: tool["version"].as_str().unwrap().into(),
                            sha256: Digest::new(tool["sha256"].as_str().unwrap()).unwrap(),
                            size: tool["size"].as_u64().unwrap(),
                            source_revision: tool["source_revision"].as_str().unwrap().into(),
                        },
                        &OutputProjection {
                            snapshot_digest: Digest::new(p["snapshot_digest"].as_str().unwrap())
                                .unwrap(),
                            candidate: CandidateOutput {
                                root: path(p["candidate"]["root"].as_str().unwrap()),
                                manifest: file(&p["candidate"]["manifest"]),
                            },
                            consumer: file(&p["consumer"]),
                            outputs: p["outputs"].as_array().unwrap().iter().map(file).collect(),
                        },
                    )
                    .unwrap()
            }
            _ => panic!("unknown vector domain"),
        };
        assert_eq!(
            digest.as_str(),
            v["digest"].as_str().unwrap(),
            "{}",
            v["id"]
        );
    }
}
#[test]
fn descriptor_uses_the_same_reserved_identifiers_as_templates() {
    let format = ProjectBuildAdapter::new().unwrap();
    let descriptor = format
        .decode_descriptor(&fixture("descriptor-positive.json"))
        .unwrap();
    for name in ["int", "gen", "true", "Self", "while"] {
        let mut bad = descriptor.clone();
        bad.namespace = name.into();
        assert!(format.encode_descriptor(&bad).is_err(), "{name}");
    }
}
struct Workspace {
    root: AbsolutePath,
    bytes: Vec<u8>,
    captured: CapturedInputs,
    begun: usize,
    published: usize,
    installed: usize,
    tail: bool,
    unknown: bool,
}
impl Workspace {
    fn new(headers: usize) -> Self {
        let bytes = fixture("descriptor-positive.json");
        let mut files = BTreeMap::new();
        for name in [
            DESCRIPTOR,
            ORIGIN,
            ".madb/template-package.tar",
            ".madb/template-input/profile.json",
            ".madb/template-input/toolchain.json",
            "rust/src/lib.rs",
            "consumer/main.c",
        ] {
            files.insert(
                path(name),
                if name == DESCRIPTOR {
                    bytes.clone()
                } else {
                    b"sample".to_vec()
                },
            );
        }
        let mut members = Vec::new();
        for i in 0..headers {
            let name = format!("h{i:04}.h");
            files.insert(path(&format!("consumer/include/{name}")), vec![]);
            members.push(DirectoryMember {
                path: path(&name),
                kind: MemberKind::File,
            });
        }
        let captured = CapturedInputs::new(
            files,
            vec![
                InputDirectory {
                    path: path("rust/src"),
                    present: true,
                    members: vec![DirectoryMember {
                        path: path("lib.rs"),
                        kind: MemberKind::File,
                    }],
                },
                InputDirectory {
                    path: path("consumer/include"),
                    present: headers > 0,
                    members,
                },
            ],
        )
        .unwrap();
        Self {
            root: absolute("/example/project"),
            bytes,
            captured,
            begun: 0,
            published: 0,
            installed: 0,
            tail: false,
            unknown: false,
        }
    }
}
impl ProjectWorkspace for Workspace {
    fn root(&self) -> &AbsolutePath {
        &self.root
    }
    fn descriptor_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.bytes.clone())
    }
    fn check_origin(&mut self, _: &Descriptor, _: &[u8]) -> Result<()> {
        Ok(())
    }
    fn capture(&mut self, _: &Descriptor, _: &[u8]) -> Result<CapturedInputs> {
        Ok(self.captured.clone())
    }
    fn begin_attempt(&mut self, c: &CapturedInputs) -> Result<BuildAttempt> {
        self.begun += 1;
        Ok(BuildAttempt {
            root: absolute("/example/attempt"),
            inputs: c.inventory()?,
            previous: None,
        })
    }
    fn created_attempt_root(&self) -> Option<&AbsolutePath> {
        None
    }
    fn revalidate(&self) -> Result<()> {
        Ok(())
    }
    fn verify_outputs(&self, _: &BuildAttempt, _: &ProjectOutput) -> Result<()> {
        Ok(())
    }
    fn publish(
        &mut self,
        _: &BuildAttempt,
        _: &ProjectOutput,
        control: &CommitControl,
    ) -> PublicationObservation {
        self.published += 1;
        control.begin().unwrap().publish(|| Ok(())).unwrap();
        assert_eq!(
            control.cancel(),
            madb_app::process::CancelDecision::AlreadyFinished
        );
        PublicationObservation {
            published: if self.unknown { None } else { Some(true) },
            durability: if self.tail || self.unknown {
                Durability::Uncertain
            } else {
                Durability::Confirmed
            },
            error: (self.tail || self.unknown).then(|| {
                QueryError::new(
                    ErrorKind::Conflict,
                    "STORE003",
                    "tail",
                    "effect occurred before this failure",
                )
            }),
        }
    }
    fn install_tool(&mut self, d: &Descriptor, control: &CommitControl) -> InstallationObservation {
        self.installed += 1;
        control.begin().unwrap().publish(|| Ok(())).unwrap();
        InstallationObservation {
            installed: Some(InstalledTool {
                path: absolute("/example/project/.madb/wrapper/madb"),
                sha256: d.tool.sha256.clone(),
                size: d.tool.size,
            }),
            error: self.tail.then(|| {
                QueryError::new(ErrorKind::Conflict, "STORE003", "tail", "\0".repeat(8192))
            }),
        }
    }
}
#[derive(Default)]
struct Control(CommitControl);
impl ProjectBuildControl for Control {
    fn checkpoint(&self) -> BuildResult<()> {
        Ok(())
    }
    fn commit(&self) -> &CommitControl {
        &self.0
    }
}
struct Execute;
impl ProjectBuildExecution for Execute {
    fn preflight(&mut self, _: Operation, _: &Descriptor) -> Result<()> {
        Ok(())
    }
    fn execute(
        &mut self,
        d: &Descriptor,
        _: &BuildAttempt,
        snapshot: &Digest,
        report: &mut ExecutionProgress<'_>,
    ) -> BuildResult<ProjectOutput> {
        for stage in [
            StageId::Prepare,
            StageId::Metadata,
            StageId::Generate,
            StageId::Build,
            StageId::Package,
            StageId::ConsumerLink,
        ] {
            report.stage_passed(stage);
        }
        let member = |name: &str| FileIdentity {
            path: path(name),
            sha256: Digest::of_bytes(name.as_bytes()),
            size: name.len() as u64,
        };
        let manifest = member("native/candidate/candidate.json");
        let consumer = member("native/consumer-build/madb_demo");
        let projection = OutputProjection {
            snapshot_digest: snapshot.clone(),
            candidate: CandidateOutput {
                root: path("native/candidate"),
                manifest: manifest.clone(),
            },
            consumer: consumer.clone(),
            outputs: vec![
                manifest,
                consumer,
                member("native/candidate/include/values.h"),
            ],
        };
        let identity = ProjectBuildAdapter::new()?.output_digest(&d.tool, &projection)?;
        Ok(ProjectOutput {
            identity,
            projection,
        })
    }
}
fn run(w: &mut Workspace, operation: Operation) -> ProjectReport {
    run_project(
        w,
        &ProjectBuildAdapter::new().unwrap(),
        &mut Execute,
        &Control::default(),
        operation,
        absolute("/example/evidence"),
    )
}
#[test]
fn metadata_budget_rejects_during_capture_and_encodes_its_empty_inventory_failure() {
    let mut w = Workspace::new(1000);
    let report = run(&mut w, Operation::Build);
    assert_eq!(w.begun, 0);
    assert!(report.inputs.files.is_empty());
    assert_eq!(report.exit_code, 2);
    assert_eq!(report.stages[0].status, StageStatus::Failed);
    ProjectBuildAdapter::new()
        .unwrap()
        .encode_report(&report)
        .unwrap();
}
#[test]
fn publication_and_installation_tail_facts_remain_encodable_after_later_evidence_failure() {
    let format = ProjectBuildAdapter::new().unwrap();
    for operation in [Operation::Build, Operation::InstallTool] {
        let mut w = Workspace::new(0);
        w.tail = true;
        let mut report = run(&mut w, operation);
        assert_eq!(report.exit_code, 3);
        assert_eq!(report.termination, Termination::Failed);
        if operation == Operation::Build {
            assert_eq!(report.published, Some(true));
            assert!(report.result.is_some());
        } else {
            assert!(report.installed_tool.is_some());
        }
        format.encode_report(&report).unwrap();
        for _ in 0..3 {
            report.evidence_failed(QueryError::new(
                ErrorKind::Host,
                "HOST003",
                "tail",
                "\0".repeat(8192),
            ));
        }
        format.encode_report(&report).unwrap();
        assert_eq!(report.exit_code, 3);
    }
}
#[test]
fn uncertain_publication_never_invents_a_result_or_cancellation() {
    let mut w = Workspace::new(0);
    w.unknown = true;
    let report = run(&mut w, Operation::Build);
    assert_eq!(report.published, None);
    assert!(report.result.is_none());
    assert_eq!(report.exit_code, 3);
    ProjectBuildAdapter::new()
        .unwrap()
        .encode_report(&report)
        .unwrap();
}
#[test]
fn successful_publication_followed_by_evidence_error_retains_result_and_marks_publish_error() {
    let mut w = Workspace::new(0);
    let mut report = run(&mut w, Operation::Build);
    assert_eq!(report.exit_code, 0);
    report.evidence_failed(QueryError::new(
        ErrorKind::Host,
        "HOST003",
        "evidence",
        "failure after CAS",
    ));
    assert!(report.result.is_some());
    assert_eq!(report.stages.last().unwrap().status, StageStatus::Error);
    ProjectBuildAdapter::new()
        .unwrap()
        .encode_report(&report)
        .unwrap();
}
