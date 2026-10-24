mod common;

use common::{registry, value};
use madb_app::{
    Digest, Id, LogicalPath, Phase, QueryError,
    dag::ExecutionGraph,
    ports::{ConfigurationBytes, FileReference},
    process::{MAX_STDIN_BYTES, MAX_TOOL_BYTES},
    task::BuildRecipe,
};
use madb_contracts::{
    DocumentKind, project_digest,
    task::{BoundBuildTask, TaskBinding, bind_graph},
    task_digest,
};
use serde_json::{Value, json};

struct Fixture {
    project: Value,
    request: Value,
    profile: Value,
    lock: Value,
    snapshot: Value,
    task: Value,
}

struct Encoded {
    project: Vec<u8>,
    request: Vec<u8>,
    profile: Vec<u8>,
    lock: Vec<u8>,
    snapshot: Vec<u8>,
    task: Vec<u8>,
    installed: FileReference,
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(value).unwrap()
}

fn byte_reference(mut reference: Value, bytes: &[u8]) -> Value {
    reference["sha256"] = json!(Digest::of_bytes(bytes).as_str());
    reference["size"] = json!(bytes.len());
    reference
}

impl Fixture {
    fn new() -> Self {
        let profile = value(DocumentKind::Profile);
        let mut task = value(DocumentKind::BuildTask);
        task["working_directory"] = json!("work");
        task["tool"] = json!({
            "id": profile["tools"][0]["id"],
            "version": profile["tools"][0]["version"],
            "sha256": profile["tools"][0]["sha256"],
        });
        task["arguments"] = json!([
            "../inputs/fixture.c",
            "a b",
            "$(literal)",
            "-o",
            "fixture.o"
        ]);
        task["environment"] =
            json!([{"name":"LANG","value":"C"}, {"name":"MADB_TEST","value":"a b"}]);
        let mut fixture = Self {
            project: value(DocumentKind::Project),
            request: value(DocumentKind::ReadRequest),
            profile,
            lock: value(DocumentKind::Lock),
            snapshot: value(DocumentKind::ContractIr),
            task,
        };
        fixture.seal_task();
        fixture
    }

    fn seal_task(&mut self) {
        let document = registry()
            .validate(DocumentKind::BuildTask, self.task.clone())
            .unwrap();
        self.task["task_digest"] = json!(task_digest(&document).unwrap());
    }

    // Golden fixtures describe format semantics, not real configuration bytes.
    // This test creates actual byte references without editing those fixtures.
    fn encoded(&self) -> Encoded {
        let profile = encode(&self.profile);
        let profile_ref = byte_reference(self.request["profile"].clone(), &profile);
        let mut project = self.project.clone();
        project["boundaries"][0]["profile"] = profile_ref.clone();
        let mut lock = self.lock.clone();
        lock["profiles"][0]["file"] = profile_ref.clone();
        lock["project_digest"] = json!(
            project_digest(
                &registry()
                    .validate(DocumentKind::Project, project.clone())
                    .unwrap()
            )
            .unwrap()
        );
        let lock = encode(&lock);
        let mut request = self.request.clone();
        request["profile"] = profile_ref;
        request["lock"] = byte_reference(request["lock"].clone(), &lock);
        Encoded {
            project: encode(&project),
            request: encode(&request),
            profile,
            lock,
            snapshot: encode(&self.snapshot),
            task: encode(&self.task),
            installed: FileReference {
                path: LogicalPath::new("bin/recipe-tool").unwrap(),
                digest: Digest::new(self.profile["tools"][0]["sha256"].as_str().unwrap()).unwrap(),
                size: 42,
            },
        }
    }
}

impl Encoded {
    fn bind(&self) -> madb_app::Result<BoundBuildTask> {
        BoundBuildTask::new(
            ConfigurationBytes {
                project: &self.project,
                request: &self.request,
                profile: &self.profile,
                lock: &self.lock,
            },
            &self.snapshot,
            &self.task,
            self.installed.clone(),
        )
    }

    fn graph(
        &self,
        tasks: &[Vec<u8>],
        target: Option<&Id>,
        active: &[Id],
    ) -> madb_app::Result<ExecutionGraph<BoundBuildTask>> {
        bind_graph(
            ConfigurationBytes {
                project: &self.project,
                request: &self.request,
                profile: &self.profile,
                lock: &self.lock,
            },
            &self.snapshot,
            tasks
                .iter()
                .map(|task| TaskBinding {
                    task,
                    installed_tool: self.installed.clone(),
                })
                .collect(),
            target,
            active,
        )
    }

    fn evidence(&self) -> Value {
        let task: Value = serde_json::from_slice(&self.task).unwrap();
        let mut result = value(DocumentKind::BuildEvidence);
        result["task_id"] = task["id"].clone();
        for field in [
            "task_digest",
            "input_digest",
            "generation_id",
            "tool",
            "phase",
        ] {
            result[field] = task[field].clone();
        }
        result["profile_digest"] = task["selector"]["profile_digest"].clone();
        result["target"] = task["selector"]["target"].clone();
        result
    }
}

fn rejected<T>(result: madb_app::Result<T>) -> QueryError {
    match result {
        Ok(_) => panic!("invalid task binding succeeded"),
        Err(error) => error,
    }
}

fn has_code(error: QueryError, code: &str) {
    assert!(
        error.findings.iter().any(|finding| finding.code == code),
        "expected {code}: {error}"
    );
}

#[test]
fn checked_binding_preserves_exact_recipe_and_host_image_location() {
    let encoded = Fixture::new().encoded();
    let bound = encoded.bind().unwrap();
    let invocation = bound.invocation();
    assert_eq!(invocation.id.as_str(), "compile-fixture");
    assert_eq!(invocation.process.tool, encoded.installed);
    assert_eq!(invocation.process.working_directory.as_str(), "work");
    assert_eq!(invocation.process.stdin, encoded.task);
    assert_eq!(
        invocation.process.arguments,
        [
            "../inputs/fixture.c",
            "a b",
            "$(literal)",
            "-o",
            "fixture.o"
        ]
    );
    assert_eq!(invocation.process.environment["MADB_TEST"], "a b");
    assert_eq!(invocation.inputs[0].path.as_str(), "fixture.c");
    assert_eq!(invocation.outputs[0].as_str(), "fixture.o");
}

#[test]
fn source_lineage_is_distinct_from_current_task_file_inventory() {
    let mut fixture = Fixture::new();
    let task_input_digest = madb_contracts::source_digest(&fixture.task["inputs"]).unwrap();
    assert_ne!(
        task_input_digest,
        fixture.task["input_digest"].as_str().unwrap()
    );
    fixture.encoded().bind().unwrap();
    fixture.task["input_digest"] = json!(task_input_digest);
    fixture.seal_task();
    has_code(rejected(fixture.encoded().bind()), "TASK001");
}

#[test]
fn configuration_references_cover_actual_lock_and_profile_bytes() {
    for file in ["lock", "profile"] {
        let mut encoded = Fixture::new().encoded();
        if file == "lock" {
            encoded.lock.push(b'\n');
        } else {
            encoded.profile.push(b'\n');
        }
        has_code(rejected(encoded.bind()), "FILE004");
    }
}

#[test]
fn checked_context_is_required_not_merely_task_self_consistency() {
    let mut fixture = Fixture::new();
    fixture.snapshot["source_digest"] = json!(Digest::of_bytes(b"different lineage").as_str());
    has_code(rejected(fixture.encoded().bind()), "PORT002");
    let mut fixture = Fixture::new();
    fixture.project["sdk_namespace"] = json!("different");
    has_code(rejected(fixture.encoded().bind()), "LOCK001");
}

#[test]
fn changed_task_bytes_require_a_matching_content_identity() {
    let mut fixture = Fixture::new();
    fixture.task["arguments"][0] = json!("different");
    has_code(rejected(fixture.encoded().bind()), "PLAN002");
}

#[test]
fn different_selector_cannot_reuse_the_checked_configuration() {
    for field in ["target", "route", "profile_id"] {
        let mut fixture = Fixture::new();
        fixture.task["selector"][field] = json!("other");
        fixture.seal_task();
        has_code(rejected(fixture.encoded().bind()), "PLAN002");
    }
}

#[test]
fn dependencies_and_other_working_directories_are_not_silently_rewritten() {
    for (field, changed) in [
        ("dependencies", json!(["prior-task"])),
        ("working_directory", json!("elsewhere")),
    ] {
        let mut fixture = Fixture::new();
        fixture.task[field] = changed;
        fixture.seal_task();
        has_code(rejected(fixture.encoded().bind()), "TASK001");
    }
}

#[test]
fn a_locked_component_is_not_an_installed_tool_image() {
    let mut fixture = Fixture::new();
    fixture.task["tool"] = fixture.lock["components"][0].clone();
    fixture.seal_task();
    let mut encoded = fixture.encoded();
    encoded.installed.digest =
        Digest::new(fixture.task["tool"]["sha256"].as_str().unwrap()).unwrap();
    has_code(rejected(encoded.bind()), "LOCK004");
}

#[test]
fn image_must_match_selected_profile_and_complete_locked_tool() {
    let mut fixture = Fixture::new();
    fixture.lock["tools"][0]["arguments"] = json!(["changed-tool-configuration"]);
    has_code(rejected(fixture.encoded().bind()), "LOCK004");
    let mut fixture = Fixture::new();
    fixture.task["tool"]["version"] = json!("different");
    fixture.seal_task();
    has_code(rejected(fixture.encoded().bind()), "LOCK004");
}

#[test]
fn installed_image_requires_matching_digest_and_bounded_positive_size() {
    for size in [0, MAX_TOOL_BYTES + 1] {
        let mut encoded = Fixture::new().encoded();
        encoded.installed.size = size;
        has_code(rejected(encoded.bind()), "LOCK004");
    }
    let mut encoded = Fixture::new().encoded();
    encoded.installed.digest = Digest::of_bytes(b"other image");
    has_code(rejected(encoded.bind()), "LOCK004");
}

#[test]
fn duplicate_environment_and_oversized_stdin_are_rejected() {
    let mut fixture = Fixture::new();
    fixture.task["environment"] =
        json!([{"name":"LANG","value":"C"}, {"name":"LANG","value":"other"}]);
    // The existing task projection rejects duplicate set identities before an
    // environment map could silently overwrite one value.
    has_code(rejected(fixture.encoded().bind()), "HASH004");
    let mut encoded = Fixture::new().encoded();
    encoded.task.resize(MAX_STDIN_BYTES + 1, b' ');
    has_code(rejected(encoded.bind()), "TASK001");
}

#[test]
fn matching_result_provides_references_without_claiming_byte_verification() {
    let encoded = Fixture::new().encoded();
    let bound = encoded.bind().unwrap();
    let files = bound.validate_result(&encode(&encoded.evidence())).unwrap();
    assert_eq!(files.outputs.len(), 1);
    assert_eq!(files.outputs[0].path.as_str(), "fixture.o");
    assert_eq!(files.logs.len(), 1);
    assert_eq!(files.logs[0].path.as_str(), "logs/build.txt");
    assert_eq!(files.outputs[0].size, 7);
}

#[test]
fn result_cannot_change_task_batch_tool_target_or_phase() {
    let encoded = Fixture::new().encoded();
    let bound = encoded.bind().unwrap();
    for (field, changed) in [
        ("task_id", json!("other")),
        ("task_digest", json!(Digest::of_bytes(b"task").as_str())),
        ("input_digest", json!(Digest::of_bytes(b"input").as_str())),
        (
            "generation_id",
            json!(Digest::of_bytes(b"generation").as_str()),
        ),
        (
            "profile_digest",
            json!(Digest::of_bytes(b"profile").as_str()),
        ),
        ("target", json!("other")),
        ("phase", json!("run")),
    ] {
        let mut evidence = encoded.evidence();
        evidence[field] = changed;
        has_code(
            bound.validate_result(&encode(&evidence)).err().unwrap(),
            "PORT002",
        );
    }
    let mut evidence = encoded.evidence();
    evidence["tool"]["version"] = json!("other");
    has_code(
        bound.validate_result(&encode(&evidence)).err().unwrap(),
        "PORT002",
    );
}

#[test]
fn failed_status_nonzero_exit_or_incomplete_outputs_cannot_pass() {
    let encoded = Fixture::new().encoded();
    let bound = encoded.bind().unwrap();
    for (field, changed) in [
        ("status", json!("failed")),
        ("status", json!("error")),
        ("exit_code", json!(1)),
        ("outputs", json!([])),
    ] {
        let mut evidence = encoded.evidence();
        evidence[field] = changed;
        has_code(
            bound.validate_result(&encode(&evidence)).err().unwrap(),
            "PORT004",
        );
    }
    let mut evidence = encoded.evidence();
    let duplicate = evidence["outputs"][0].clone();
    evidence["outputs"].as_array_mut().unwrap().push(duplicate);
    has_code(
        bound.validate_result(&encode(&evidence)).err().unwrap(),
        "PORT004",
    );
}

#[test]
fn malformed_or_wrong_schema_result_is_rejected() {
    let encoded = Fixture::new().encoded();
    let bound = encoded.bind().unwrap();
    has_code(bound.validate_result(b"{").err().unwrap(), "FMT001");
    has_code(
        bound.validate_result(&encoded.task).err().unwrap(),
        "FMT003",
    );
}

fn sealed_task(mut task: Value) -> Vec<u8> {
    let document = registry()
        .validate(DocumentKind::BuildTask, task.clone())
        .unwrap();
    task["task_digest"] = json!(task_digest(&document).unwrap());
    encode(&task)
}

fn graph_tasks(encoded: &Encoded) -> Vec<Vec<u8>> {
    let producer: Value = serde_json::from_slice(&encoded.task).unwrap();
    let mut consumer = producer.clone();
    consumer["id"] = json!("link-fixture");
    consumer["phase"] = json!("link");
    consumer["dependencies"] = json!([producer["id"]]);
    consumer["inputs"] = value(DocumentKind::BuildEvidence)["outputs"].clone();
    consumer["outputs"] = json!(["linked.o"]);
    vec![sealed_task(consumer), encoded.task.clone()]
}

#[test]
fn graph_binding_preserves_dependencies_phase_and_deterministic_order() {
    let encoded = Fixture::new().encoded();
    let tasks = graph_tasks(&encoded);
    let graph = encoded.graph(&tasks, None, &[]).unwrap();
    let metadata = graph.tasks();
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata[0].id.as_str(), "compile-fixture");
    assert_eq!(metadata[0].phase, Phase::Compile);
    assert!(metadata[0].dependencies.is_empty());
    assert_eq!(metadata[1].id.as_str(), "link-fixture");
    assert_eq!(metadata[1].phase, Phase::Link);
    assert_eq!(metadata[1].dependencies, [metadata[0].id.clone()]);
    assert_eq!(metadata[1].outputs[0].as_str(), "linked.o");

    let target = Id::new("compile-fixture").unwrap();
    let selected = encoded.graph(&tasks, Some(&target), &[]).unwrap();
    assert_eq!(selected.tasks().len(), 1);
    assert_eq!(selected.tasks()[0].id, target);
    assert!(encoded.graph(&tasks, None, &[target]).is_err());

    // A graph-capable document cannot bypass scheduling via the old entry.
    let mut single = Fixture::new().encoded();
    single.task = tasks[0].clone();
    has_code(rejected(single.bind()), "TASK001");
}

#[test]
fn every_graph_node_uses_the_same_checked_context_even_when_unselected() {
    let encoded = Fixture::new().encoded();
    let target = Id::new("compile-fixture").unwrap();
    for (field, changed, code) in [
        ("selector.target", json!("different"), "PLAN002"),
        (
            "input_digest",
            json!(Digest::of_bytes(b"other lineage").as_str()),
            "TASK001",
        ),
        ("working_directory", json!("elsewhere"), "TASK001"),
    ] {
        let mut tasks = graph_tasks(&encoded);
        let mut task: Value = serde_json::from_slice(&tasks[0]).unwrap();
        if field == "selector.target" {
            task["selector"]["target"] = changed;
        } else {
            task[field] = changed;
        }
        tasks[0] = sealed_task(task);
        has_code(rejected(encoded.graph(&tasks, Some(&target), &[])), code);
    }
}

#[test]
fn graph_binding_checks_each_task_digest_and_executable_image() {
    let encoded = Fixture::new().encoded();
    let mut tasks = graph_tasks(&encoded);
    let mut changed: Value = serde_json::from_slice(&tasks[0]).unwrap();
    changed["arguments"][0] = json!("different");
    tasks[0] = encode(&changed);
    has_code(rejected(encoded.graph(&tasks, None, &[])), "PLAN002");

    let mut tasks = graph_tasks(&encoded);
    let mut changed: Value = serde_json::from_slice(&tasks[0]).unwrap();
    changed["tool"]["sha256"] = json!(Digest::of_bytes(b"other tool").as_str());
    tasks[0] = sealed_task(changed);
    has_code(rejected(encoded.graph(&tasks, None, &[])), "LOCK004");
}

#[test]
fn graph_binding_cannot_skip_invalid_configuration() {
    let mut encoded = Fixture::new().encoded();
    let tasks = graph_tasks(&encoded);
    encoded.lock.push(b'\n');
    has_code(rejected(encoded.graph(&tasks, None, &[])), "FILE004");
}

#[test]
fn graph_structural_validation_precedes_named_target_selection() {
    let encoded = Fixture::new().encoded();
    let target = Id::new("compile-fixture").unwrap();
    for dependency in ["missing", "link-fixture"] {
        let mut tasks = graph_tasks(&encoded);
        let mut consumer: Value = serde_json::from_slice(&tasks[0]).unwrap();
        consumer["dependencies"] = json!([dependency]);
        tasks[0] = sealed_task(consumer);
        assert!(encoded.graph(&tasks, Some(&target), &[]).is_err());
    }
    // A same-named file must not become external source when its producer was
    // omitted from the direct dependencies or a target selection.
    let mut tasks = graph_tasks(&encoded);
    let mut consumer: Value = serde_json::from_slice(&tasks[0]).unwrap();
    consumer["dependencies"] = json!([]);
    tasks[0] = sealed_task(consumer);
    assert!(encoded.graph(&tasks, Some(&target), &[]).is_err());
}

#[test]
fn graph_document_limits_are_checked_before_parsing_the_context() {
    let mut encoded = Fixture::new().encoded();
    encoded.request = b"invalid context".to_vec();
    for tasks in [
        vec![],
        vec![vec![]; 4097],
        vec![vec![b' '; MAX_STDIN_BYTES + 1]],
        vec![vec![b' '; MAX_STDIN_BYTES]; 17],
    ] {
        has_code(rejected(encoded.graph(&tasks, None, &[])), "TASK001");
    }
}
