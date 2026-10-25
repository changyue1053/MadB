mod common;
use common::{registry, value};
use madb_app::{
    Digest, LogicalPath, Result,
    inputs::{FrozenInputs, capture_inputs},
    pipeline::{ArtifactKind, ArtifactRecipe},
    ports::{ConfigurationBytes, FileReference, ReadWorkspace},
};
use madb_contracts::{
    DocumentKind,
    execution::{
        BoundArtifactRecipe, ExecutionOperation, ExecutionRecipe, generation_task_digest,
        read_execution, source_manifest, validate_build_generation,
    },
    generation_id, project_digest, source_digest,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn encode(v: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(v).unwrap()
}
fn file(path: &str, schema: &str, bytes: &[u8]) -> Value {
    json!({"path":path,"schema":schema,"sha256":Digest::of_bytes(bytes).as_str(),"size":bytes.len()})
}
#[derive(Default)]
struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, path: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
        Ok(self.0.get(path).cloned())
    }
}
struct Fixture {
    project: Vec<u8>,
    read: Vec<u8>,
    profile: Vec<u8>,
    lock: Vec<u8>,
    snapshot: Vec<u8>,
    generate: Value,
    bundle: Value,
    memory: Memory,
}
impl Fixture {
    fn new() -> Self {
        Self::with_profile(value(DocumentKind::Profile))
    }
    fn with_profile(profile: Value) -> Self {
        let digest = madb_contracts::profile_digest(
            &registry()
                .validate(DocumentKind::Profile, profile.clone())
                .unwrap(),
        )
        .unwrap();
        let profile = encode(&profile);
        let profile_ref = file("profiles/rust-c.json", "madb.profile/1", &profile);
        let source = b"fn fixture() {}\n";
        let source_ref = file("native/fixture.rs", "madb.source-input/1", source);
        let schema = b"{}\n";
        let schema_ref = file("schemas/common.schema.json", "madb.schema/1", schema);
        let mut project = value(DocumentKind::Project);
        project["boundaries"][0]["profile"] = profile_ref.clone();
        let mut lock = value(DocumentKind::Lock);
        lock["profiles"][0]["file"] = profile_ref.clone();
        lock["profiles"][0]["semantic_digest"] = json!(digest);
        lock["project_digest"] = json!(
            project_digest(
                &registry()
                    .validate(DocumentKind::Project, project.clone())
                    .unwrap()
            )
            .unwrap()
        );
        lock["inputs"] = json!([source_ref.clone()]);
        lock["inputs"][0].as_object_mut().unwrap().remove("schema");
        lock["schemas"] = json!([schema_ref.clone()]);
        lock["schemas"][0].as_object_mut().unwrap().remove("schema");
        let lock = encode(&lock);
        let mut read = value(DocumentKind::ReadRequest);
        read["selector"]["profile_digest"] = json!(digest);
        read["lock"] = file("madb.lock", "madb.lock/1", &lock);
        read["profile"] = profile_ref;
        read["inputs"] = json!([source_ref]);
        let input_digest = source_digest(&read["inputs"]).unwrap();
        let mut snapshot = value(DocumentKind::ContractIr);
        snapshot["selector"] = read["selector"].clone();
        snapshot["source_digest"] = json!(input_digest);
        let snapshot = encode(&snapshot);
        let mut generate = value(DocumentKind::GenerateRequest);
        generate["selector"] = read["selector"].clone();
        for field in ["lock", "profile", "inputs"] {
            generate[field] = read[field].clone();
        }
        generate["contract"] = file("contract.json", "madb.contract-ir/1", &snapshot);
        generate["input_digest"] = json!(input_digest);
        let mut bundle = value(DocumentKind::Execution);
        bundle["operation"] = json!("generate");
        bundle["context"]["contract"] = generate["contract"].clone();
        bundle["context"]["project"] = file("madb.json", "madb.project/1", &encode(&project));
        bundle["context"]["read_request"] =
            file("read.json", "madb.read-request/1", &encode(&read));
        bundle["context"]["lock"] = read["lock"].clone();
        bundle["context"]["profile"] = read["profile"].clone();
        bundle["recipe"]["inputs"] = json!([
            read["inputs"][0].clone(),
            schema_ref.clone(),
            generate["contract"].clone()
        ]);
        bundle["recipe"]["response_path"] = json!("outputs.json");
        bundle["recipe"]["installed_tool"]["sha256"] = bundle["recipe"]["tool"]["sha256"].clone();
        let mut memory = Memory::default();
        memory.0.insert(
            LogicalPath::new("native/fixture.rs").unwrap(),
            source.to_vec(),
        );
        memory.0.insert(
            LogicalPath::new("schemas/common.schema.json").unwrap(),
            schema.to_vec(),
        );
        memory
            .0
            .insert(LogicalPath::new("contract.json").unwrap(), snapshot.clone());
        let mut f = Self {
            project: encode(&project),
            read: encode(&read),
            profile,
            lock,
            snapshot,
            generate,
            bundle,
            memory,
        };
        f.seal();
        f
    }
    fn seal(&mut self) {
        self.generate["task_digest"] = json!(
            generation_task_digest(
                &registry()
                    .validate(DocumentKind::GenerateRequest, self.generate.clone())
                    .unwrap(),
                &self.bundle["recipe"]
            )
            .unwrap()
        );
        self.bundle["recipe"]["request"] = file(
            "generate.json",
            "madb.generate-request/1",
            &encode(&self.generate),
        );
    }
    fn configuration(&self) -> ConfigurationBytes<'_> {
        ConfigurationBytes {
            project: &self.project,
            request: &self.read,
            profile: &self.profile,
            lock: &self.lock,
        }
    }
    fn recipe(&self) -> ExecutionRecipe {
        read_execution(&encode(&self.bundle))
            .unwrap()
            .recipe
            .unwrap()
    }
    fn frozen(&self) -> FrozenInputs {
        let files = self
            .memory
            .0
            .iter()
            .map(|(path, bytes)| FileReference {
                path: path.clone(),
                digest: Digest::of_bytes(bytes),
                size: bytes.len() as u64,
            })
            .collect();
        capture_inputs(&self.memory, files).unwrap()
    }
    fn bind(&self) -> Result<BoundArtifactRecipe> {
        BoundArtifactRecipe::new(
            self.configuration(),
            Some(&self.snapshot),
            &self.recipe(),
            &encode(&self.generate),
            &self.frozen(),
        )
    }
    fn outputs(&self) -> Value {
        let mut output = value(DocumentKind::OutputSet);
        output["profile_digest"] = self.generate["selector"]["profile_digest"].clone();
        output["input_digest"] = self.generate["input_digest"].clone();
        output["task_digest"] = self.generate["task_digest"].clone();
        output["files"][0]["sha256"] = json!(Digest::of_bytes(b"generated header\n").as_str());
        output["files"][0]["size"] = json!(17);
        output["generation_id"] = json!(
            generation_id(
                &registry()
                    .validate(DocumentKind::OutputSet, output.clone())
                    .unwrap()
            )
            .unwrap()
        );
        output
    }
}
fn code<T>(result: Result<T>, code: &str) {
    let error = match result {
        Ok(_) => panic!("invalid binding accepted"),
        Err(error) => error,
    };
    assert!(
        error.findings.iter().any(|f| f.code == code),
        "expected {code}: {error}"
    );
}
#[test]
fn execution_decode_exposes_owned_values_and_rejects_stdin_cycle() {
    let f = Fixture::new();
    let spec = read_execution(&encode(&f.bundle)).unwrap();
    assert_eq!(spec.operation, ExecutionOperation::Generate);
    spec.context
        .validate(f.configuration(), Some(&f.snapshot))
        .unwrap();
    let mut bad = f.bundle.clone();
    bad["recipe"]["inputs"][0] = bad["recipe"]["request"].clone();
    code(read_execution(&encode(&bad)), "EXECUTION001");
    let mut bad = f.bundle.clone();
    bad["recipe"]["environment"] = json!([{"name":"A","value":"x"},{"name":"A","value":"y"}]);
    code(read_execution(&encode(&bad)), "EXECUTION001");
}
#[test]
fn generation_projection_excludes_locations_but_binds_semantic_arguments_and_schema() {
    let f = Fixture::new();
    let doc = registry()
        .validate(DocumentKind::GenerateRequest, f.generate.clone())
        .unwrap();
    let expected = generation_task_digest(&doc, &f.bundle["recipe"]).unwrap();
    let mut recipe = f.bundle["recipe"].clone();
    recipe["request"]["path"] = json!("elsewhere.json");
    recipe["installed_tool"]["path"] = json!("bin/elsewhere");
    recipe["inputs"].as_array_mut().unwrap().reverse();
    assert_eq!(generation_task_digest(&doc, &recipe).unwrap(), expected);
    recipe["arguments"] = json!(["changed"]);
    assert_ne!(generation_task_digest(&doc, &recipe).unwrap(), expected);
    recipe = f.bundle["recipe"].clone();
    recipe["inputs"][0]["schema"] = json!("madb.other/1");
    assert_ne!(generation_task_digest(&doc, &recipe).unwrap(), expected);
}
#[test]
fn real_generate_request_binds_and_response_identity_is_checked() {
    let f = Fixture::new();
    let bound = f.bind().unwrap();
    assert_eq!(bound.invocation().process.stdin, encode(&f.generate));
    let output = f.outputs();
    let declaration = bound.validate_result(&encode(&output)).unwrap();
    assert_eq!(declaration.kind, ArtifactKind::Generation);
    assert_eq!(declaration.files[0].path.as_str(), "include/fixture.h");
    let mut bad = output.clone();
    bad["files"][0]["sources"] = json!(["undeclared.symbol"]);
    bad["generation_id"] = json!(
        generation_id(
            &registry()
                .validate(DocumentKind::OutputSet, bad.clone())
                .unwrap()
        )
        .unwrap()
    );
    code(bound.validate_result(&encode(&bad)), "EXECUTION002");
    let mut bad = output;
    bad["manifest_path"] = json!("wrong.json");
    code(bound.validate_result(&encode(&bad)), "EXECUTION002");
}
#[test]
fn binding_rejects_changed_digest_context_image_and_frozen_reference() {
    let mut f = Fixture::new();
    f.bundle["recipe"]["arguments"] = json!(["new"]);
    code(f.bind(), "PLAN002");
    f.seal();
    f.bundle["recipe"]["installed_tool"]["size"] = json!(0);
    code(f.bind(), "LOCK004");
    let mut f = Fixture::new();
    f.generate["contract_digest"] = json!(Digest::of_bytes(b"other").as_str());
    f.seal();
    code(f.bind(), "EXECUTION002");
    let mut f = Fixture::new();
    f.memory.0.insert(
        LogicalPath::new("contract.json").unwrap(),
        b"changed".to_vec(),
    );
    code(f.bind(), "FILE004");
}
#[test]
fn metadata_executes_read_request_and_checks_real_ir_without_previous_snapshot() {
    let f = Fixture::new();
    let mut bundle = f.bundle.clone();
    bundle["operation"] = json!("metadata");
    bundle["context"]["contract"] = Value::Null;
    bundle["recipe"]["request"] = file("read.json", "madb.read-request/1", &f.read);
    bundle["recipe"]["response_path"] = json!("contract.json");
    let recipe = read_execution(&encode(&bundle)).unwrap().recipe.unwrap();
    let bound =
        BoundArtifactRecipe::new(f.configuration(), None, &recipe, &f.read, &f.frozen()).unwrap();
    let declaration = bound.validate_result(&f.snapshot).unwrap();
    assert_eq!(declaration.kind, ArtifactKind::Metadata);
    assert!(declaration.files.is_empty());
    let mut bad: Value = serde_json::from_slice(&f.snapshot).unwrap();
    bad["source_digest"] = json!(Digest::of_bytes(b"other").as_str());
    code(bound.validate_result(&encode(&bad)), "PORT002");
}
#[test]
fn build_generation_and_source_manifest_check_actual_committed_inventory() {
    let f = Fixture::new();
    let outputs = f.outputs();
    let bytes = encode(&outputs);
    let mut memory = Memory::default();
    memory
        .0
        .insert(LogicalPath::new("outputs.json").unwrap(), bytes.clone());
    memory.0.insert(
        LogicalPath::new("include/fixture.h").unwrap(),
        b"generated header\n".to_vec(),
    );
    let refs = memory
        .0
        .iter()
        .map(|(path, bytes)| FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        })
        .collect();
    let frozen = capture_inputs(&memory, refs).unwrap();
    assert_eq!(source_manifest(&frozen).unwrap(), bytes);
    let mut task = value(DocumentKind::BuildTask);
    task["generation_id"] = outputs["generation_id"].clone();
    task["input_digest"] = outputs["input_digest"].clone();
    validate_build_generation(&[encode(&task)], &bytes, &frozen).unwrap();
    task["generation_id"] = json!(Digest::of_bytes(b"old").as_str());
    code(
        validate_build_generation(&[encode(&task)], &bytes, &frozen),
        "EXECUTION002",
    );
}
#[test]
fn package_checks_actual_output_set_evidence_dependency_bytes_and_lineage() {
    let mut f = Fixture::new();
    let output = f.outputs();
    let output_bytes = encode(&output);
    f.memory.0.insert(
        LogicalPath::new("outputs.json").unwrap(),
        output_bytes.clone(),
    );
    f.memory.0.insert(
        LogicalPath::new("include/fixture.h").unwrap(),
        b"generated header\n".to_vec(),
    );
    let mut evidence = value(DocumentKind::BuildEvidence);
    evidence["generation_id"] = output["generation_id"].clone();
    evidence["input_digest"] = output["input_digest"].clone();
    evidence["tool"] = f.bundle["recipe"]["tool"].clone();
    evidence["outputs"] =
        json!([{"path":"build/library.a","sha256":Digest::of_bytes(b"library").as_str(),"size":7}]);
    let mut evidence_refs = Vec::new();
    for phase in ["compile", "link", "run"] {
        evidence["phase"] = json!(phase);
        evidence["task_id"] = json!(phase);
        let bytes = encode(&evidence);
        let path = format!("evidence/{phase}.json");
        evidence_refs.push(file(&path, "madb.build-evidence/1", &bytes));
        f.memory.0.insert(LogicalPath::new(path).unwrap(), bytes);
    }
    f.memory.0.insert(
        LogicalPath::new("build/library.a").unwrap(),
        b"library".to_vec(),
    );
    let mut request = value(DocumentKind::PackageRequest);
    request["generation_id"] = output["generation_id"].clone();
    request["lock"] = f.generate["lock"].clone();
    request["output_set"] = file("outputs.json", "madb.output-set/1", &output_bytes);
    request["build_evidence"] = json!(evidence_refs);
    let mut bundle = f.bundle.clone();
    bundle["operation"] = json!("package");
    bundle["recipe"]["inputs"] = json!(
        f.memory
            .0
            .iter()
            .map(|(path, bytes)| file(path.as_str(), "madb.artifact/1", bytes))
            .collect::<Vec<_>>()
    );
    bundle["recipe"]["response_path"] = json!("candidate.json");
    bundle["recipe"]["request"] = file("package.json", "madb.package-request/1", &encode(&request));
    let recipe = read_execution(&encode(&bundle)).unwrap().recipe.unwrap();
    BoundArtifactRecipe::new(
        f.configuration(),
        Some(&f.snapshot),
        &recipe,
        &encode(&request),
        &f.frozen(),
    )
    .unwrap();
    f.memory.0.insert(
        LogicalPath::new("build/library.a").unwrap(),
        b"wrong".to_vec(),
    );
    code(
        BoundArtifactRecipe::new(
            f.configuration(),
            Some(&f.snapshot),
            &recipe,
            &encode(&request),
            &f.frozen(),
        ),
        "FILE004",
    );
    f.memory.0.insert(
        LogicalPath::new("build/library.a").unwrap(),
        b"library".to_vec(),
    );
    request["dependencies"] = json!([file("missing.a", "madb.dependency/1", b"missing")]);
    bundle["recipe"]["request"] = file("package.json", "madb.package-request/1", &encode(&request));
    let recipe = read_execution(&encode(&bundle)).unwrap().recipe.unwrap();
    code(
        BoundArtifactRecipe::new(
            f.configuration(),
            Some(&f.snapshot),
            &recipe,
            &encode(&request),
            &f.frozen(),
        ),
        "FILE003",
    );
}

fn build_results(
    f: &Fixture,
    phases: &[&str],
) -> (
    Vec<Vec<u8>>,
    BTreeMap<madb_app::Id, madb_app::task::ValidatedTaskResult>,
) {
    use madb_app::{Id, task::capture_task_result};
    use madb_contracts::{task::BoundBuildTask, task_digest};
    let mut tasks = Vec::new();
    let mut results = BTreeMap::new();
    for &phase in phases {
        let mut task = value(DocumentKind::BuildTask);
        task["selector"] = f.generate["selector"].clone();
        task["id"] = json!(phase);
        task["phase"] = json!(phase);
        task["working_directory"] = json!("work");
        task["tool"] = f.bundle["recipe"]["tool"].clone();
        task["input_digest"] = f.generate["input_digest"].clone();
        let output_path = format!("build/{phase}.bin");
        task["outputs"] = json!([output_path]);
        task["task_digest"] = json!(
            task_digest(
                &registry()
                    .validate(DocumentKind::BuildTask, task.clone())
                    .unwrap()
            )
            .unwrap()
        );
        let bytes = encode(&task);
        let bound = BoundBuildTask::new(
            f.configuration(),
            &f.snapshot,
            &bytes,
            f.recipe().installed_tool,
        )
        .unwrap();
        let mut evidence = value(DocumentKind::BuildEvidence);
        evidence["task_id"] = task["id"].clone();
        evidence["profile_digest"] = task["selector"]["profile_digest"].clone();
        for key in [
            "task_digest",
            "input_digest",
            "generation_id",
            "tool",
            "phase",
        ] {
            evidence[key] = task[key].clone();
        }
        let mut output_ref = file(&output_path, "madb.artifact/1", b"output");
        output_ref.as_object_mut().unwrap().remove("schema");
        let mut log_ref = file("logs/result.txt", "madb.log/1", b"log");
        log_ref.as_object_mut().unwrap().remove("schema");
        evidence["outputs"] = json!([output_ref]);
        evidence["logs"] = json!([log_ref]);
        let mut memory = Memory::default();
        memory
            .0
            .insert(LogicalPath::new(output_path).unwrap(), b"output".to_vec());
        memory.0.insert(
            LogicalPath::new("logs/result.txt").unwrap(),
            b"log".to_vec(),
        );
        let result = capture_task_result(&memory, &bound, &encode(&evidence)).unwrap();
        results.insert(Id::new(phase).unwrap(), result);
        tasks.push(bytes);
    }
    (tasks, results)
}

#[test]
fn build_assembly_preserves_exact_evidence_and_rejects_incomplete_phase_set() {
    use madb_app::Id;
    use madb_contracts::execution::assemble_build_files;
    let f = Fixture::new();
    let (tasks, mut results) = build_results(&f, &["compile", "link", "run"]);
    let batch = assemble_build_files(&tasks, &results, &f.profile).unwrap();
    assert_eq!(batch.evidence.len(), 3);
    assert_eq!(batch.files.references().count(), 6);
    for (index, (_, result)) in results.iter().enumerate() {
        let path = format!("evidence/task-{index:04}.json");
        let (_, bytes) = batch
            .files
            .files()
            .find(|(file, _)| file.path.as_str() == path)
            .unwrap();
        assert_eq!(bytes, result.payload());
    }
    let compile = Id::new("compile").unwrap();
    let result = results.remove(&compile).unwrap();
    let only = BTreeMap::from([(compile, result)]);
    code(
        assemble_build_files(&tasks[..1], &only, &f.profile),
        "EXECUTION003",
    );
}

fn freeze(memory: &Memory) -> FrozenInputs {
    let references = memory
        .0
        .iter()
        .map(|(path, bytes)| FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        })
        .collect();
    capture_inputs(memory, references).unwrap()
}
#[test]
fn package_parent_gate_rejects_external_evidence_and_omitted_pinned_evidence() {
    use madb_contracts::execution::validate_package_parents;
    let f = Fixture::new();
    let output = f.outputs();
    let bytes = encode(&output);
    let mut source = Memory::default();
    source
        .0
        .insert(LogicalPath::new("outputs.json").unwrap(), bytes.clone());
    source.0.insert(
        LogicalPath::new("include/fixture.h").unwrap(),
        b"generated header\n".to_vec(),
    );
    let source = freeze(&source);
    let mut build = Memory::default();
    let mut refs = Vec::new();
    for (index, phase) in ["compile", "link", "run"].iter().enumerate() {
        let mut evidence = value(DocumentKind::BuildEvidence);
        evidence["task_id"] = json!(phase);
        evidence["phase"] = json!(phase);
        let bytes = encode(&evidence);
        let path = format!("evidence/task-{index:04}.json");
        refs.push(file(&path, "madb.build-evidence/1", &bytes));
        build.0.insert(LogicalPath::new(path).unwrap(), bytes);
    }
    let build = freeze(&build);
    let mut request = value(DocumentKind::PackageRequest);
    request["output_set"] = file("outputs.json", "madb.output-set/1", &bytes);
    request["build_evidence"] = json!(refs);
    validate_package_parents(&encode(&request), &source, &build).unwrap();
    let mut omitted = request.clone();
    omitted["build_evidence"].as_array_mut().unwrap().pop();
    code(
        validate_package_parents(&encode(&omitted), &source, &build),
        "EXECUTION002",
    );
    request["build_evidence"][0]["path"] = json!("external/forged-evidence.json");
    code(
        validate_package_parents(&encode(&request), &source, &build),
        "FILE003",
    );
}
#[test]
fn frozen_superset_cannot_hide_missing_materialized_inputs() {
    let mut f = Fixture::new();
    f.bundle["recipe"]["inputs"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    f.seal();
    code(f.bind(), "FILE003");
}

#[test]
fn dependency_preparation_requires_an_exact_selected_and_locked_image() {
    use madb_contracts::execution::validate_dependency_tool;
    let f = Fixture::new();
    let mut compiler = f.recipe().installed_tool;
    validate_dependency_tool(f.configuration(), &compiler).unwrap();
    compiler.digest = Digest::of_bytes(b"unlocked compiler");
    code(
        validate_dependency_tool(f.configuration(), &compiler),
        "LOCK004",
    );
    compiler = f.recipe().installed_tool;
    compiler.size = 0;
    code(
        validate_dependency_tool(f.configuration(), &compiler),
        "LOCK004",
    );
    let mut lock: Value = serde_json::from_slice(&f.lock).unwrap();
    lock["tools"][0]["arguments"] = json!(["changed"]);
    let lock = encode(&lock);
    let mut request: Value = serde_json::from_slice(&f.read).unwrap();
    request["lock"] = file("madb.lock", "madb.lock/1", &lock);
    let request = encode(&request);
    code(
        validate_dependency_tool(
            ConfigurationBytes {
                project: &f.project,
                request: &request,
                profile: &f.profile,
                lock: &lock,
            },
            &f.recipe().installed_tool,
        ),
        "LOCK004",
    );
}

fn external_runtime_fixture() -> Fixture {
    let mut profile = value(DocumentKind::Profile);
    for key in ["required_features", "supported_features"] {
        profile[key]
            .as_array_mut()
            .unwrap()
            .push(json!(madb_model::build::EXTERNAL_RUNTIME_VERIFICATION));
    }
    Fixture::with_profile(profile)
}

#[test]
fn external_runtime_build_assembly_accepts_two_phases_without_relaxing_identity() {
    use madb_contracts::execution::assemble_build_files;
    let f = external_runtime_fixture();
    let (tasks, mut results) = build_results(&f, &["compile", "link"]);
    let batch = assemble_build_files(&tasks, &results, &f.profile).unwrap();
    assert_eq!(batch.evidence.len(), 2);
    code(
        assemble_build_files(&tasks, &results, &Fixture::new().profile),
        "EXECUTION002",
    );
    let compile = madb_app::Id::new("compile").unwrap();
    let only = BTreeMap::from([(compile.clone(), results.remove(&compile).unwrap())]);
    code(
        assemble_build_files(&tasks[..1], &only, &f.profile),
        "EXECUTION003",
    );
    code(
        assemble_build_files(&tasks, &only, &f.profile),
        "EXECUTION002",
    );
}

fn package_recipe(f: &Fixture, request: &Value) -> ExecutionRecipe {
    let mut bundle = f.bundle.clone();
    bundle["operation"] = json!("package");
    bundle["recipe"]["inputs"] = json!(
        f.memory
            .0
            .iter()
            .map(|(path, bytes)| file(path.as_str(), "madb.artifact/1", bytes))
            .collect::<Vec<_>>()
    );
    bundle["recipe"]["response_path"] = json!("candidate.json");
    bundle["recipe"]["request"] = file("package.json", "madb.package-request/1", &encode(request));
    read_execution(&encode(&bundle)).unwrap().recipe.unwrap()
}

#[test]
fn external_runtime_package_accepts_two_phases_and_keeps_negative_evidence_checks() {
    let mut f = external_runtime_fixture();
    let output = f.outputs();
    let output_bytes = encode(&output);
    f.memory.0.insert(
        LogicalPath::new("outputs.json").unwrap(),
        output_bytes.clone(),
    );
    f.memory.0.insert(
        LogicalPath::new("include/fixture.h").unwrap(),
        b"generated header\n".to_vec(),
    );
    f.memory.0.insert(
        LogicalPath::new("build/library.a").unwrap(),
        b"library".to_vec(),
    );
    let mut request = value(DocumentKind::PackageRequest);
    request["selector"] = f.generate["selector"].clone();
    request["generation_id"] = output["generation_id"].clone();
    request["lock"] = f.generate["lock"].clone();
    request["output_set"] = file("outputs.json", "madb.output-set/1", &output_bytes);
    for mode in [
        "valid",
        "missing-link",
        "duplicate-id",
        "failed",
        "wrong-profile",
        "wrong-generation",
        "wrong-tool",
        "forbidden-run",
    ] {
        let mut refs = Vec::new();
        for phase in ["compile", "link"] {
            if mode == "missing-link" && phase == "link" {
                continue;
            }
            let mut evidence = value(DocumentKind::BuildEvidence);
            evidence["phase"] = json!(phase);
            evidence["task_id"] = json!(phase);
            evidence["generation_id"] = output["generation_id"].clone();
            evidence["input_digest"] = output["input_digest"].clone();
            evidence["profile_digest"] = output["profile_digest"].clone();
            evidence["tool"] = f.bundle["recipe"]["tool"].clone();
            evidence["outputs"] = json!([{"path":"build/library.a", "sha256":Digest::of_bytes(b"library").as_str(), "size":7}]);
            if phase == "link" {
                match mode {
                    "duplicate-id" => evidence["task_id"] = json!("compile"),
                    "failed" => {
                        evidence["status"] = json!("failed");
                        evidence["exit_code"] = json!(1);
                    }
                    "wrong-profile" => {
                        evidence["profile_digest"] =
                            json!(Digest::of_bytes(b"other-profile").as_str())
                    }
                    "wrong-generation" => {
                        evidence["generation_id"] =
                            json!(Digest::of_bytes(b"other-generation").as_str())
                    }
                    "wrong-tool" => {
                        evidence["tool"]["sha256"] = json!(Digest::of_bytes(b"other-tool").as_str())
                    }
                    "forbidden-run" => evidence["phase"] = json!("run"),
                    _ => {}
                }
            }
            let bytes = encode(&evidence);
            let path = format!("evidence/{phase}.json");
            refs.push(file(&path, "madb.build-evidence/1", &bytes));
            f.memory.0.insert(LogicalPath::new(path).unwrap(), bytes);
        }
        request["build_evidence"] = json!(refs);
        let recipe = package_recipe(&f, &request);
        let result = BoundArtifactRecipe::new(
            f.configuration(),
            Some(&f.snapshot),
            &recipe,
            &encode(&request),
            &f.frozen(),
        );
        match mode {
            "valid" => {
                result.unwrap();
            }
            "missing-link" | "forbidden-run" => code(result, "EXECUTION003"),
            "wrong-tool" => code(result, "LOCK004"),
            _ => code(result, "EXECUTION002"),
        }
    }
}

#[test]
fn two_phase_package_parent_gate_rejects_omission_external_and_duplicate_refs() {
    use madb_contracts::execution::validate_package_parents;
    let f = external_runtime_fixture();
    let output = f.outputs();
    let bytes = encode(&output);
    let mut source = Memory::default();
    source
        .0
        .insert(LogicalPath::new("outputs.json").unwrap(), bytes.clone());
    source.0.insert(
        LogicalPath::new("include/fixture.h").unwrap(),
        b"generated header\n".to_vec(),
    );
    let source = freeze(&source);
    let mut build = Memory::default();
    let mut refs = Vec::new();
    for (index, phase) in ["compile", "link"].iter().enumerate() {
        let mut evidence = value(DocumentKind::BuildEvidence);
        evidence["task_id"] = json!(phase);
        evidence["phase"] = json!(phase);
        let path = format!("evidence/task-{index:04}.json");
        let bytes = encode(&evidence);
        refs.push(file(&path, "madb.build-evidence/1", &bytes));
        build.0.insert(LogicalPath::new(path).unwrap(), bytes);
    }
    let build = freeze(&build);
    let mut request = value(DocumentKind::PackageRequest);
    request["output_set"] = file("outputs.json", "madb.output-set/1", &bytes);
    request["build_evidence"] = json!(refs);
    validate_package_parents(&encode(&request), &source, &build).unwrap();
    let mut omitted = request.clone();
    omitted["build_evidence"].as_array_mut().unwrap().pop();
    code(
        validate_package_parents(&encode(&omitted), &source, &build),
        "EXECUTION002",
    );
    let mut duplicate = request.clone();
    duplicate["build_evidence"][1] = duplicate["build_evidence"][0].clone();
    code(
        validate_package_parents(&encode(&duplicate), &source, &build),
        "FILE001",
    );
    request["build_evidence"][0]["path"] = json!("external/forged-evidence.json");
    code(
        validate_package_parents(&encode(&request), &source, &build),
        "FILE003",
    );
}
