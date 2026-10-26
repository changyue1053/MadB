mod common;
use common::*;
use madb_app::{
    Digest, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{extension_files::*, *};
use madb_protocol::{Value, byte_digest};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Default, Clone)]
struct Memory(BTreeMap<String, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, path: &LogicalPath, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        if let Some(bytes) = self.0.get(path.as_str()) {
            if bytes.len() > limit {
                return Err(madb_app::QueryError::new(
                    madb_app::ErrorKind::InvalidInput,
                    "INPUT002",
                    path.as_str(),
                    "byte limit exceeded",
                ));
            }
            return Ok(Some(bytes.clone()));
        }
        Ok(None)
    }
}
impl Memory {
    fn put(&mut self, path: &str, bytes: &[u8]) -> Value {
        self.0.insert(path.into(), bytes.into());
        json!({"path":path,"sha256":byte_digest(bytes),"size":bytes.len()})
    }
    fn document(&mut self, path: &str, document: &Value) -> Value {
        let mut reference = self.put(path, &serde_json::to_vec(document).unwrap());
        reference["schema"] = document["schema"].clone();
        reference
    }
    fn freeze(&self) -> FrozenInputs {
        capture_inputs(
            self,
            self.0
                .iter()
                .map(|(p, b)| FileReference {
                    path: LogicalPath::new(p).unwrap(),
                    digest: Digest::of_bytes(b),
                    size: b.len() as u64,
                })
                .collect(),
        )
        .unwrap()
    }
}
fn build() -> (BoundExtensionFiles, Memory, Value) {
    let mut inputs = Memory::default();
    let mut q = value(DocumentKind::BuildTask);
    q["inputs"] = json!([inputs.put("fixture.c", b"source")]);
    q["working_directory"] = json!("outputs/task");
    q["task_digest"] = json!(
        task_digest(
            &registry()
                .validate(DocumentKind::BuildTask, q.clone())
                .unwrap()
        )
        .unwrap()
    );
    let mut reference = inputs.document("request.json", &q);
    reference["path"] = json!("inputs/request.json");
    let bound =
        BoundExtensionFiles::new("build", &reference, "outputs/task", inputs.freeze()).unwrap();
    let mut outputs = Memory::default();
    let mut result = value(DocumentKind::BuildEvidence);
    result["task_digest"] = q["task_digest"].clone();
    result["outputs"] = json!([outputs.put("fixture.o", b"object")]);
    result["logs"] = json!([outputs.put("logs/build.txt", b"compiler completed")]);
    (bound, outputs, result)
}
fn result_ref(outputs: &mut Memory, result: &Value) -> Value {
    let mut reference = outputs.document("result.json", result);
    reference["path"] = json!("outputs/task/result.json");
    reference
}
#[test]
fn successful_build_is_frozen_with_explicit_identity_and_domain_local_paths() {
    let (bound, mut outputs, result) = build();
    let reference = result_ref(&mut outputs, &result);
    let captured = bound.capture_result(&outputs, &reference).unwrap();
    assert_eq!(
        captured.document().value()["task_digest"],
        bound.request().value()["task_digest"]
    );
    assert_eq!(captured.reference(), &reference);
    outputs.put("fixture.o", b"mutated after capture");
    let admitted = captured.into_inputs().unwrap();
    assert_eq!(
        admitted
            .files()
            .find(|(r, _)| r.path.as_str() == "fixture.o")
            .unwrap()
            .1,
        b"object"
    );
    assert!(admitted.verify(&outputs).is_err());
}
#[test]
fn control_references_cannot_escape_or_alias_fixed_domains() {
    let (bound, mut outputs, result) = build();
    let reference = result_ref(&mut outputs, &result);
    for path in [
        "../result.json",
        "/result.json",
        "outputs/task/../result.json",
        "outputs/other/result.json",
        "outputs/Task/result.json",
        "outputs/task//result.json",
        "outputs/task/result.json/child",
    ] {
        let mut bad = reference.clone();
        bad["path"] = json!(path);
        assert!(
            bound.capture_result(&outputs, &bad).is_err(),
            "accepted {path}"
        );
    }
    let mut wrong = reference;
    wrong["schema"] = json!("madb.contract-ir/1");
    assert!(bound.capture_result(&outputs, &wrong).is_err());
}
#[test]
fn raw_bytes_and_port_semantics_both_have_to_match() {
    let (bound, mut outputs, result) = build();
    let reference = result_ref(&mut outputs, &result);
    outputs.0.get_mut("result.json").unwrap().push(b' ');
    assert!(bound.capture_result(&outputs, &reference).is_err());
    let reference = result_ref(&mut outputs, &result);
    outputs.put("fixture.o", b"changed");
    assert!(bound.capture_result(&outputs, &reference).is_err());
    let (bound, mut outputs, mut result) = build();
    result["input_digest"] = json!(byte_digest(b"another input"));
    let reference = result_ref(&mut outputs, &result);
    assert_eq!(
        bound
            .capture_result(&outputs, &reference)
            .err()
            .unwrap()
            .code,
        "OutputInvalid"
    );
}
#[test]
fn payload_paths_reject_traversal_aliases_and_file_directory_collisions() {
    for path in [
        "../outside",
        "/outside",
        "Fixture.o",
        "fixture.o/child",
        "fixture.o",
    ] {
        let (bound, mut outputs, mut result) = build();
        result["logs"][0] = outputs.put(path, b"log");
        let reference = result_ref(&mut outputs, &result);
        assert!(
            bound.capture_result(&outputs, &reference).is_err(),
            "accepted {path}"
        );
    }
}
#[test]
fn failed_business_results_remain_diagnostic_and_cannot_feed_downstream() {
    for status in ["failed", "error"] {
        let (bound, mut outputs, mut result) = build();
        result["status"] = json!(status);
        result["exit_code"] = json!(1);
        let reference = result_ref(&mut outputs, &result);
        let captured = bound.capture_result(&outputs, &reference).unwrap();
        assert_eq!(captured.document().value()["status"], status);
        assert_eq!(captured.into_inputs().unwrap_err().code, "ToolFailed");

        let mut inputs = Memory::default();
        let mut package = value(DocumentKind::PackageRequest);
        package["build_evidence"] = json!([inputs.document("build.json", &result)]);
        package["output_root"] = json!("outputs/package");
        let mut request = inputs.document("request.json", &package);
        request["path"] = json!("inputs/request.json");
        assert_eq!(
            BoundExtensionFiles::new("package", &request, "outputs/package", inputs.freeze())
                .err()
                .unwrap()
                .code,
            "InputInvalid"
        );
    }
}

fn verification() -> (BoundExtensionFiles, Memory, Value) {
    let mut inputs = Memory::default();
    let mut q = value(DocumentKind::VerifyRequest);
    let mut candidate = value(DocumentKind::Candidate);
    let mut evidence = value(DocumentKind::BuildEvidence);
    evidence["outputs"] = json!([inputs.put("fixture.o", b"fixture")]);
    evidence["logs"] = json!([inputs.put("logs/build.txt", b"fixture")]);
    candidate["build_evidence"] = json!([inputs.document("evidence/build.json", &evidence)]);
    candidate["candidate_digest"] = json!(
        candidate_digest(
            &registry()
                .validate(DocumentKind::Candidate, candidate.clone())
                .unwrap()
        )
        .unwrap()
    );
    q["identity"]["candidate_digest"] = candidate["candidate_digest"].clone();
    q["candidate"] = inputs.document("candidate.json", &candidate);
    inputs.put("include/fixture.h", b"/* fixture header */\n");
    inputs.put("fixtures/value.json", b"fixture");
    q["cases"] = json!([inputs.document("cases/value.json", &value(DocumentKind::Case))]);
    q["coverage"] = inputs.document("coverage.json", &value(DocumentKind::Coverage));
    q["output_root"] = json!("outputs/task");
    let mut reference = inputs.document("request.json", &q);
    reference["path"] = json!("inputs/request.json");
    let bound =
        BoundExtensionFiles::new("verify", &reference, "outputs/task", inputs.freeze()).unwrap();
    let mut outputs = Memory::default();
    let mut report = value(DocumentKind::Report);
    report["identity"] = q["identity"].clone();
    report["results"][0]["attachments"] =
        json!([outputs.put("evidence.txt", b"real fixture evidence")]);
    (bound, outputs, report)
}

#[test]
fn reports_need_independent_cases_and_evidence_level_before_admission() {
    use madb_model::evidence::EvidenceLevel;
    let selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    let required = required_case(
        &document(DocumentKind::Case),
        &selected,
        EvidenceLevel::HostRun,
    )
    .unwrap();
    let (bound, mut outputs, report) = verification();
    let reference = result_ref(&mut outputs, &report);
    assert!(
        bound
            .capture_result(&outputs, &reference)
            .unwrap()
            .into_inputs()
            .is_err()
    );
    bound
        .capture_result(&outputs, &reference)
        .unwrap()
        .accept_report(std::slice::from_ref(&required))
        .unwrap();
    for status in ["failed", "error", "skipped", "unsupported", "blocked"] {
        let mut report = report.clone();
        report["results"][0]["status"] = json!(status);
        let reference = result_ref(&mut outputs, &report);
        assert!(
            bound
                .capture_result(&outputs, &reference)
                .unwrap()
                .accept_report(std::slice::from_ref(&required))
                .is_err()
        );
    }
    let reference = result_ref(&mut outputs, &report);
    assert!(
        bound
            .capture_result(&outputs, &reference)
            .unwrap()
            .accept_report(&[])
            .is_err()
    );
    let device_required = required_case(
        &document(DocumentKind::Case),
        &selected,
        EvidenceLevel::DeviceRun,
    )
    .unwrap();
    assert!(
        bound
            .capture_result(&outputs, &reference)
            .unwrap()
            .accept_report(&[device_required])
            .is_err()
    );
}

struct ArtifactFixture {
    operation: &'static str,
    project: Vec<u8>,
    read: Vec<u8>,
    profile: Vec<u8>,
    lock: Vec<u8>,
    contract: Vec<u8>,
    request: Value,
    memory: Memory,
    recipe: Value,
}
impl ArtifactFixture {
    fn new(operation: &'static str) -> Self {
        let mut memory = Memory::default();
        memory.put("sdk/fixture.json", b"fixture");
        memory.put("schemas/common.schema.json", b"fixture");
        let mut source = memory.put("native/fixture.rs", b"pub fn fixture() {}\n");
        source["schema"] = json!("madb.source-input/1");
        let profile_value = value(DocumentKind::Profile);
        let profile = serde_json::to_vec(&profile_value).unwrap();
        let profile_ref = memory.document("profiles/rust-c.json", &profile_value);
        let mut project = value(DocumentKind::Project);
        project["boundaries"][0]["profile"] = profile_ref.clone();
        let mut lock = value(DocumentKind::Lock);
        lock["profiles"][0]["file"] = profile_ref.clone();
        lock["project_digest"] = json!(
            project_digest(
                &registry()
                    .validate(DocumentKind::Project, project.clone())
                    .unwrap()
            )
            .unwrap()
        );
        let lock_ref = memory.document("madb.lock", &lock);
        let lock = serde_json::to_vec(&lock).unwrap();
        let mut read = value(DocumentKind::ReadRequest);
        read["profile"] = profile_ref;
        read["lock"] = lock_ref;
        read["inputs"] = json!([source]);
        read["output_root"] = json!("outputs/task");
        let contract = serde_json::to_vec(&value(DocumentKind::ContractIr)).unwrap();
        let mut recipe = value(DocumentKind::Execution)["recipe"].clone();
        recipe["installed_tool"]["sha256"] = recipe["tool"]["sha256"].clone();
        recipe["response_path"] = json!("result.json");
        let request = match operation {
            "read" => read.clone(),
            "generate" => {
                let mut q = value(DocumentKind::GenerateRequest);
                for field in ["inputs", "lock", "profile", "selector", "output_root"] {
                    q[field] = read[field].clone();
                }
                q["contract"] = memory.document("contract.json", &value(DocumentKind::ContractIr));
                q
            }
            "package" => {
                let output = value(DocumentKind::OutputSet);
                memory.put("include/fixture.h", b"/* fixture header */\n");
                let mut q = value(DocumentKind::PackageRequest);
                q["lock"] = read["lock"].clone();
                q["output_root"] = read["output_root"].clone();
                q["output_set"] = memory.document("outputs.json", &output);
                let mut evidence = value(DocumentKind::BuildEvidence);
                evidence["tool"] = recipe["tool"].clone();
                evidence["outputs"] = json!([memory.put("build/library.a", b"library")]);
                evidence["logs"] = json!([memory.put("logs/build.txt", b"fixture")]);
                let mut refs = Vec::new();
                for phase in ["compile", "link", "run"] {
                    evidence["task_id"] = json!(phase);
                    evidence["phase"] = json!(phase);
                    refs.push(memory.document(&format!("evidence/{phase}.json"), &evidence));
                }
                q["build_evidence"] = json!(refs);
                let mut dependency = memory.put("deps/runtime.a", b"runtime");
                dependency["schema"] = json!("madb.dependency/1");
                q["dependencies"] = json!([dependency]);
                q
            }
            _ => unreachable!(),
        };
        Self {
            operation,
            project: serde_json::to_vec(&project).unwrap(),
            read: serde_json::to_vec(&read).unwrap(),
            profile,
            lock,
            contract,
            request,
            memory,
            recipe,
        }
    }
    fn bind(&mut self) -> Result<BoundExtensionFiles, FormatError> {
        use madb_contracts::execution::{
            BoundArtifactRecipe, generation_task_digest, read_execution,
        };
        self.recipe["inputs"] = json!(self.memory.0.iter().map(|(path,bytes)| json!({"path":path,"size":bytes.len(),"sha256":byte_digest(bytes),"schema":"madb.artifact/1"})).collect::<Vec<_>>());
        if self.operation == "generate" {
            self.request["task_digest"] = json!(
                generation_task_digest(
                    &registry()
                        .validate(DocumentKind::GenerateRequest, self.request.clone())
                        .unwrap(),
                    &self.recipe
                )
                .unwrap()
            );
        }
        let mut memory = self.memory.clone();
        let mut reference = memory.document("request.json", &self.request);
        self.recipe["request"] = reference.clone();
        reference["path"] = json!("inputs/request.json");
        let mut bundle = value(DocumentKind::Execution);
        bundle["operation"] = json!(if self.operation == "read" {
            "metadata"
        } else {
            self.operation
        });
        if self.operation == "read" {
            bundle["context"]["contract"] = Value::Null;
        } else {
            bundle["context"]["contract"] = json!({"path":"contract.json","schema":"madb.contract-ir/1","size":self.contract.len(),"sha256":byte_digest(&self.contract)});
        }
        bundle["recipe"] = self.recipe.clone();
        let recipe = read_execution(&serde_json::to_vec(&bundle).unwrap())
            .unwrap()
            .recipe
            .unwrap();
        let checked = BoundArtifactRecipe::new(
            madb_app::ports::ConfigurationBytes {
                project: &self.project,
                request: &self.read,
                profile: &self.profile,
                lock: &self.lock,
            },
            if self.operation == "read" {
                None
            } else {
                Some(&self.contract)
            },
            &recipe,
            &serde_json::to_vec(&self.request).unwrap(),
            &memory.freeze(),
        )
        .map_err(|e| FormatError::new("InputInvalid", "/context", e.to_string()))?;
        BoundExtensionFiles::from_artifact_recipe(
            &reference,
            "outputs/task",
            memory.freeze(),
            checked,
        )
    }
    fn response(&self) -> (Memory, Value) {
        let mut outputs = Memory::default();
        let mut response = match self.operation {
            "read" => value(DocumentKind::ContractIr),
            "generate" => {
                let mut v = value(DocumentKind::OutputSet);
                v["manifest_path"] = json!("result.json");
                v["task_digest"] = self.request["task_digest"].clone();
                v["generation_id"] = json!(
                    generation_id(
                        &registry()
                            .validate(DocumentKind::OutputSet, v.clone())
                            .unwrap()
                    )
                    .unwrap()
                );
                outputs.put("include/fixture.h", b"/* fixture header */\n");
                v
            }
            "package" => {
                let mut v = value(DocumentKind::Candidate);
                v["manifest_path"] = json!("result.json");
                for field in ["generation_id", "dependencies", "build_evidence"] {
                    v[field] = self.request[field].clone();
                }
                v["lock_digest"] = self.request["lock"]["sha256"].clone();
                v["candidate_digest"] = json!(
                    candidate_digest(
                        &registry()
                            .validate(DocumentKind::Candidate, v.clone())
                            .unwrap()
                    )
                    .unwrap()
                );
                for (path, bytes) in &self.memory.0 {
                    if path.starts_with("evidence/")
                        || path.starts_with("logs/")
                        || path.starts_with("build/")
                        || path.starts_with("deps/")
                        || path.starts_with("include/")
                    {
                        outputs.put(path, bytes);
                    }
                }
                v
            }
            _ => unreachable!(),
        };
        if self.operation == "read" {
            response["source_digest"] = json!(
                source_digest(&serde_json::from_slice::<Value>(&self.read).unwrap()["inputs"])
                    .unwrap()
            );
        }
        (outputs, response)
    }
}
fn generation() -> (BoundExtensionFiles, Memory, Value) {
    let mut f = ArtifactFixture::new("generate");
    let bound = f.bind().unwrap();
    let (outputs, result) = f.response();
    (bound, outputs, result)
}
#[test]
fn file_manifest_identity_is_recomputed_separately_from_its_byte_reference() {
    let (bound, mut outputs, result) = generation();
    let reference = result_ref(&mut outputs, &result);
    bound
        .capture_result(&outputs, &reference)
        .unwrap()
        .into_inputs()
        .unwrap();
    let mut changed = result.clone();
    changed["generation_id"] = json!(byte_digest(b"other semantic identity"));
    let reference = result_ref(&mut outputs, &changed);
    assert!(bound.capture_result(&outputs, &reference).is_err());
    let mut changed = result;
    changed["manifest_path"] = json!("other.json");
    changed["generation_id"] = json!(
        generation_id(
            &registry()
                .validate(DocumentKind::OutputSet, changed.clone())
                .unwrap()
        )
        .unwrap()
    );
    let reference = result_ref(&mut outputs, &changed);
    assert!(bound.capture_result(&outputs, &reference).is_err());
}
#[test]
fn artifacts_require_checked_context_and_reader_reuses_c04_result_validation() {
    for operation in ["read", "generate", "package"] {
        let mut f = ArtifactFixture::new(operation);
        let bound = f.bind().unwrap();
        let (mut outputs, result) = f.response();
        let reference = result_ref(&mut outputs, &result);
        bound
            .capture_result(&outputs, &reference)
            .unwrap()
            .into_inputs()
            .unwrap();
        let mut memory = f.memory.clone();
        let mut request = memory.document("request.json", &f.request);
        request["path"] = json!("inputs/request.json");
        assert!(
            BoundExtensionFiles::new(operation, &request, "outputs/task", memory.freeze()).is_err()
        );
    }
    let mut f = ArtifactFixture::new("read");
    let bound = f.bind().unwrap();
    let (mut outputs, mut ir) = f.response();
    ir["structure_digest"] = json!(byte_digest(b"wrong semantics"));
    let reference = result_ref(&mut outputs, &ir);
    assert!(bound.capture_result(&outputs, &reference).is_err());
}
#[test]
fn malformed_input_wrong_format_and_task_semantics_are_rejected_before_invocation() {
    let mut inputs = Memory::default();
    let mut q = value(DocumentKind::BuildTask);
    q["inputs"] = json!([inputs.put("fixture.c", b"source")]);
    let mut reference = inputs.document("request.json", &q);
    reference["path"] = json!("inputs/request.json");
    assert!(
        BoundExtensionFiles::new("build", &reference, "outputs/task", inputs.freeze()).is_err()
    );
    q["task_digest"] = json!(
        task_digest(
            &registry()
                .validate(DocumentKind::BuildTask, q.clone())
                .unwrap()
        )
        .unwrap()
    );
    let mut reference = inputs.document("request.json", &q);
    reference["path"] = json!("inputs/request.json");
    assert!(BoundExtensionFiles::new("read", &reference, "outputs/task", inputs.freeze()).is_err());
    assert!(BoundExtensionFiles::new("build", &reference, "inputs/task", inputs.freeze()).is_err());
    reference["path"] = json!("inputs/../request.json");
    assert!(
        BoundExtensionFiles::new("build", &reference, "outputs/task", inputs.freeze()).is_err()
    );
}

#[test]
fn legacy_package_admission_rejects_new_composition_documents_in_dependencies() {
    for kind in [DocumentKind::Composition, DocumentKind::ProfileV2] {
        let mut fixture = ArtifactFixture::new("package");
        let dependency = fixture
            .memory
            .document("deps/new-format.json", &value(kind));
        fixture.request["dependencies"] = json!([dependency]);
        let error = fixture.bind().err().expect("legacy admission must reject");
        assert_eq!(error.code, "InputInvalid", "{kind:?}: {error}");
        assert!(
            error.message.starts_with("CapabilityUnsupported:"),
            "{kind:?}: {error}"
        );
        assert_eq!(error.path, "/schema");
    }
}

#[test]
fn candidate_closure_captures_dependencies_evidence_outputs_and_logs() {
    let mut f = ArtifactFixture::new("package");
    let bound = f.bind().unwrap();
    let (mut outputs, result) = f.response();
    let reference = result_ref(&mut outputs, &result);
    let frozen = bound
        .capture_result(&outputs, &reference)
        .unwrap()
        .into_inputs()
        .unwrap();
    for path in [
        "deps/runtime.a",
        "evidence/compile.json",
        "evidence/link.json",
        "evidence/run.json",
        "build/library.a",
        "logs/build.txt",
    ] {
        assert!(
            frozen.references().any(|r| r.path.as_str() == path),
            "missing {path}"
        );
    }
    for missing in [
        "deps/runtime.a",
        "evidence/compile.json",
        "build/library.a",
        "logs/build.txt",
    ] {
        let mut bad = outputs.clone();
        bad.0.remove(missing);
        assert!(
            bound.capture_result(&bad, &reference).is_err(),
            "accepted missing {missing}"
        );
        let mut bad = outputs.clone();
        bad.put(missing, b"wrong bytes");
        assert!(
            bound.capture_result(&bad, &reference).is_err(),
            "accepted corrupt {missing}"
        );
    }
}
#[test]
fn checked_package_rejects_transitive_input_gaps_and_wrong_payload_format() {
    for missing in [
        "deps/runtime.a",
        "build/library.a",
        "logs/build.txt",
        "sdk/fixture.json",
        "schemas/common.schema.json",
    ] {
        let mut f = ArtifactFixture::new("package");
        f.memory.0.remove(missing);
        assert!(f.bind().is_err(), "accepted missing {missing}");
    }
    let mut f = ArtifactFixture::new("package");
    f.request["build_evidence"][0]["schema"] = json!("madb.profile/1");
    assert!(f.bind().is_err());
    for path in ["../outside.a", "/outside.a", "deps/../outside.a"] {
        let mut f = ArtifactFixture::new("package");
        f.request["dependencies"][0]["path"] = json!(path);
        // Schema rejection is also a pre-invocation rejection; no invalid request is decoded as another port.
        assert!(
            registry()
                .validate(DocumentKind::PackageRequest, f.request.clone())
                .is_err()
        );
    }
    let mut f = ArtifactFixture::new("package");
    f.request["dependencies"][0]["sha256"] = json!(byte_digest(b"other"));
    assert!(f.bind().is_err());
}
#[test]
fn checked_package_rejects_mixed_generation_profile_input_target_tool_and_phase() {
    for field in [
        "generation_id",
        "profile_digest",
        "input_digest",
        "target",
        "tool",
        "phase",
    ] {
        let mut f = ArtifactFixture::new("package");
        let mut evidence: Value =
            serde_json::from_slice(&f.memory.0["evidence/compile.json"]).unwrap();
        match field {
            "target" => evidence[field] = json!("x86_64-unknown-linux-gnu"),
            "phase" => evidence[field] = json!("metadata"),
            "tool" => evidence["tool"]["sha256"] = json!(byte_digest(b"other tool")),
            _ => evidence[field] = json!(byte_digest(b"old generation or another input")),
        }
        f.request["build_evidence"][0] = f.memory.document("evidence/compile.json", &evidence);
        let error = f.bind().err().unwrap_or_else(|| panic!("accepted {field}"));
        assert!(
            error.message.contains(if field == "tool" {
                "LOCK004"
            } else if field == "phase" {
                "EXECUTION003"
            } else {
                "EXECUTION002"
            }),
            "{field}: {error}"
        );
    }
    let mut f = ArtifactFixture::new("package");
    f.request["generation_id"] = json!(byte_digest(b"new candidate over old source"));
    assert!(f.bind().is_err());
    let mut f = ArtifactFixture::new("generate");
    f.request["input_digest"] = json!(byte_digest(b"different contract source"));
    assert!(f.bind().is_err());
}
#[test]
fn undeclared_frozen_files_are_not_materialized_as_implicit_inputs() {
    let (bound, _, _) = build();
    let mut memory = Memory(
        bound
            .inputs()
            .files()
            .map(|(r, b)| (r.path.as_str().to_owned(), b.to_vec()))
            .collect(),
    );
    memory.put("untracked-secret.txt", b"not declared by task");
    assert!(
        BoundExtensionFiles::new(
            "build",
            bound.input_ref(),
            bound.output_root(),
            memory.freeze()
        )
        .is_err()
    );
    let mut f = ArtifactFixture::new("read");
    f.memory.put("untracked.txt", b"not in request closure");
    assert!(f.bind().is_err());
}
#[test]
fn frozen_coverage_and_candidate_requirements_cannot_be_silently_omitted() {
    for change in ["profile", "coverage", "candidate", "build_lineage"] {
        let (bound, _, _) = verification();
        let mut memory = Memory(
            bound
                .inputs()
                .files()
                .map(|(r, b)| (r.path.as_str().to_owned(), b.to_vec()))
                .collect(),
        );
        let mut q = bound.request().value().clone();
        match change {
            "profile" => q["identity"]["profile_digest"] = json!(byte_digest(b"different profile")),
            "coverage" => {
                let mut coverage: Value =
                    serde_json::from_slice(&memory.0["coverage.json"]).unwrap();
                coverage["requirements"][0]["case_ids"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("DATA-002/required"));
                q["coverage"] = memory.document("coverage.json", &coverage);
            }
            "candidate" => {
                let mut candidate: Value =
                    serde_json::from_slice(&memory.0["candidate.json"]).unwrap();
                candidate["required_cases"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("DATA-002/required"));
                candidate["candidate_digest"] = json!(
                    candidate_digest(
                        &registry()
                            .validate(DocumentKind::Candidate, candidate.clone())
                            .unwrap()
                    )
                    .unwrap()
                );
                q["identity"]["candidate_digest"] = candidate["candidate_digest"].clone();
                q["candidate"] = memory.document("candidate.json", &candidate);
            }
            "build_lineage" => {
                let mut evidence: Value =
                    serde_json::from_slice(&memory.0["evidence/build.json"]).unwrap();
                evidence["generation_id"] = json!(byte_digest(b"another generation"));
                let mut candidate: Value =
                    serde_json::from_slice(&memory.0["candidate.json"]).unwrap();
                candidate["build_evidence"][0] = memory.document("evidence/build.json", &evidence);
                candidate["candidate_digest"] = json!(
                    candidate_digest(
                        &registry()
                            .validate(DocumentKind::Candidate, candidate.clone())
                            .unwrap()
                    )
                    .unwrap()
                );
                q["identity"]["candidate_digest"] = candidate["candidate_digest"].clone();
                q["candidate"] = memory.document("candidate.json", &candidate);
            }
            _ => unreachable!(),
        }
        let mut reference = memory.document("request.json", &q);
        reference["path"] = json!("inputs/request.json");
        assert!(
            BoundExtensionFiles::new("verify", &reference, "outputs/task", memory.freeze())
                .is_err(),
            "accepted {change}"
        );
    }
}

fn session_manifest(operation: &str, process: &madb_app::process::ProcessRequest) -> Value {
    let port = match operation {
        "read" => "contract-reader",
        "generate" => "generator",
        "package" => "artifact-packager",
        _ => unreachable!(),
    };
    json!({"schema":"madb.extension/1","id":"org.madb.fixture","version":"0.1.0",
        "compatibility":{"protocol":["1.0"],"ir":["1.0"]},"languages":[],
        "components":[{"id":"reader","port":port,"supported_features":["core.values@1"],
            "entrypoint":{"kind":"process","path":process.tool.path.as_str(),"args":process.arguments}}],
        "dependencies":[],"routes":[],"assets":[],
        "execution":{"transport":"stdio-jsonl","network":"none","env":process.environment.keys().collect::<Vec<_>>()}})
}
fn session_options(component_id: &str) -> madb_contracts::extension_session::SessionOptions<'_> {
    madb_contracts::extension_session::SessionOptions {
        component_id,
        session_id: "session",
        request_id: "request",
        protocol: "1.0",
        ir: "1.0",
        required_features: &["core.values@1"],
        handshake_timeout: std::time::Duration::from_secs(1),
        task_timeout: std::time::Duration::from_secs(1),
    }
}
fn recipe_process(f: &ArtifactFixture) -> madb_app::process::ProcessRequest {
    let r = &f.recipe;
    madb_app::process::ProcessRequest {
        tool: FileReference {
            path: LogicalPath::new(r["installed_tool"]["path"].as_str().unwrap()).unwrap(),
            digest: Digest::new(r["installed_tool"]["sha256"].as_str().unwrap()).unwrap(),
            size: r["installed_tool"]["size"].as_u64().unwrap(),
        },
        working_directory: LogicalPath::new("work").unwrap(),
        arguments: r["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect(),
        environment: r["environment"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["name"].as_str().unwrap().to_owned(),
                    v["value"].as_str().unwrap().to_owned(),
                )
            })
            .collect(),
        // E02 uses its own dialogue on stdin; payload bytes are in inputs/request.json.
        stdin: Vec::new(),
    }
}
#[test]
fn checked_artifact_recipe_binds_the_process_actually_selected_for_the_session() {
    use madb_app::extension::SessionDriver;
    use madb_contracts::{extension::ExtensionManifest, extension_session::HostSession};
    for operation in ["read", "generate", "package"] {
        let mut fixture = ArtifactFixture::new(operation);
        let bound = fixture.bind().unwrap();
        for change in [
            "baseline",
            "path",
            "digest",
            "size",
            "arguments",
            "environment",
        ] {
            let mut process = recipe_process(&fixture);
            match change {
                "baseline" => {}
                "path" => process.tool.path = LogicalPath::new("bin/other-same-port").unwrap(),
                "digest" => process.tool.digest = Digest::of_bytes(b"other executable bytes"),
                "size" => process.tool.size += 1,
                "arguments" => process.arguments.push("--other-mode".into()),
                "environment" => {
                    process
                        .environment
                        .insert("E02_FIXTURE".into(), "other".into());
                }
                _ => unreachable!(),
            }
            // Manifest B deliberately agrees with the substituted process. Only
            // the retained checked recipe can detect this same-port substitution.
            let manifest = ExtensionManifest::decode(
                &serde_json::to_vec(&session_manifest(operation, &process)).unwrap(),
            )
            .unwrap();
            let session =
                HostSession::new(&manifest, &bound, session_options("reader"), |_| Ok(())).unwrap();
            let result = session.validate_request(&process);
            if change == "baseline" {
                result.unwrap();
            } else {
                assert_eq!(
                    result.unwrap_err().code,
                    "ExtensionIdentityMismatch",
                    "{operation}/{change}"
                );
            }
        }
    }
}
#[test]
fn same_executable_cannot_relabel_the_read_or_generate_component() {
    use madb_contracts::{extension::ExtensionManifest, extension_session::HostSession};
    for operation in ["read", "generate"] {
        let mut fixture = ArtifactFixture::new(operation);
        let bound = fixture.bind().unwrap();
        let process = recipe_process(&fixture);
        for change in ["extension", "component", "version"] {
            let mut value = session_manifest(operation, &process);
            let component = if change == "component" {
                value["components"][0]["id"] = json!("another-reader");
                "another-reader"
            } else {
                "reader"
            };
            if change == "extension" {
                value["id"] = json!("org.madb.another");
            }
            if change == "version" {
                value["version"] = json!("0.2.0");
            }
            let manifest = ExtensionManifest::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
            let error = HostSession::new(&manifest, &bound, session_options(component), |_| Ok(()))
                .err()
                .unwrap_or_else(|| panic!("accepted {operation}/{change}"));
            assert_eq!(error.code, "ExtensionIdentityMismatch");
        }
    }
}
