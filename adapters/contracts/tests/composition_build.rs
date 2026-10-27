//! Build-only adapter vectors share the frozen composition fixture with package.
mod composition_fixture;
use composition_fixture::*;
use madb_contracts::{
    DocumentKind,
    composition::build::{assemble_build_files, bind_build_graph},
    task::TaskBinding,
};
use madb_model::Digest;
use serde_json::{Value, json};

#[test]
fn bound_profile_two_reuses_build_graph_and_result_assembly() {
    let f = Fixture::new();
    let contract = f.finish().unwrap();
    let tasks = f.tasks(&contract, &["compile", "link", "run"]);
    let graph = f.graph(&contract, &tasks, None).unwrap();
    assert_eq!(graph.tasks().len(), 3);
    let bound = f.bound();
    let artifacts = assemble_build_files(
        &bound,
        &document(DocumentKind::Project, f.project.clone()),
        &contract,
        &f.required(),
        &tasks,
        &results(&f, &tasks),
    )
    .unwrap();
    assert_eq!(artifacts.evidence.len(), 3);
    assert_eq!(artifacts.files.references().count(), 6);
    assert!(madb_contracts::build_completion_policy(bound.profile()).is_err());
}
#[test]
fn mismatched_source_selector_tool_and_unselected_nodes_are_rejected() {
    let f = Fixture::new();
    let contract = f.finish().unwrap();
    let good = f.tasks(&contract, &["compile", "link", "run"]);
    for (field, replacement, expected) in [
        (
            "input_digest",
            json!(Digest::of_bytes(b"wrong").as_str()),
            "TASK001",
        ),
        (
            "task_digest",
            json!(Digest::of_bytes(b"wrong").as_str()),
            "PLAN002",
        ),
    ] {
        let mut tasks = good.clone();
        let mut t: Value = serde_json::from_slice(&tasks[2]).unwrap();
        t[field] = replacement;
        if field != "task_digest" {
            seal(&mut t);
        }
        tasks[2] = bytes(&t);
        code(f.graph(&contract, &tasks, Some(&id("task-0"))), expected);
    }
    let mut task: Value = serde_json::from_slice(&good[0]).unwrap();
    task["selector"]["profile_digest"] = json!(Digest::of_bytes(b"wrong profile").as_str());
    seal(&mut task);
    code(f.graph(&contract, &[bytes(&task)], None), "PLAN002");
    task = serde_json::from_slice(&good[0]).unwrap();
    task["tool"]["version"] = json!("9.9.9");
    seal(&mut task);
    code(f.graph(&contract, &[bytes(&task)], None), "LOCK004");
    let mut installed = f.image();
    installed.digest = Digest::of_bytes(b"wrong installed image");
    code(
        bind_build_graph(
            &f.bound(),
            &document(DocumentKind::Project, f.project.clone()),
            &contract,
            &f.required(),
            vec![TaskBinding {
                task: &good[0],
                installed_tool: installed,
            }],
            None,
            &[],
        ),
        "LOCK004",
    );
}
#[test]
fn wrong_context_and_missing_responsibility_fail_before_graph() {
    let f = Fixture::new();
    let contract = f.finish().unwrap();
    let tasks = f.tasks(&contract, &["compile"]);
    let bound = f.bound();
    let project = document(DocumentKind::Project, f.project.clone());
    code(
        bind_build_graph(
            &bound,
            &project,
            &contract,
            &[],
            vec![TaskBinding {
                task: &tasks[0],
                installed_tool: f.image(),
            }],
            None,
            &[],
        ),
        "InputInvalid",
    );
    let mut changed = contract.value().clone();
    changed["source_digest"] = json!(Digest::of_bytes(b"other source").as_str());
    code(
        f.graph(&document(DocumentKind::ContractIr, changed), &tasks, None),
        "LOCK005",
    );
    let mut project = f.project.clone();
    project["sdk_namespace"] = json!("other");
    code(
        bind_build_graph(
            &bound,
            &document(DocumentKind::Project, project),
            &contract,
            &f.required(),
            vec![TaskBinding {
                task: &tasks[0],
                installed_tool: f.image(),
            }],
            None,
            &[],
        ),
        "LOCK002",
    );
}
#[test]
fn completion_and_assembly_preserve_policy_and_input_binding() {
    let f = Fixture::new();
    let contract = f.finish().unwrap();
    let tasks = f.tasks(&contract, &["compile", "link"]);
    let bound = f.bound();
    let project = document(DocumentKind::Project, f.project.clone());
    code(
        assemble_build_files(
            &bound,
            &project,
            &contract,
            &f.required(),
            &tasks,
            &results(&f, &tasks),
        ),
        "EXECUTION003",
    );
    let mut tasks = f.tasks(&contract, &["compile", "link", "run"]);
    let mut task: Value = serde_json::from_slice(&tasks[0]).unwrap();
    task["input_digest"] = json!(Digest::of_bytes(b"wrong source").as_str());
    seal(&mut task);
    tasks[0] = bytes(&task);
    code(
        assemble_build_files(
            &bound,
            &project,
            &contract,
            &f.required(),
            &tasks,
            &results(&f, &tasks),
        ),
        "TASK001",
    );
}
#[test]
fn external_runtime_build_keeps_compile_link_and_forbids_run() {
    let mut f = Fixture::new();
    let feature = madb_model::build::EXTERNAL_RUNTIME_VERIFICATION;
    for key in ["required_features", "supported_features"] {
        f.profile[key].as_array_mut().unwrap().push(json!(feature));
    }
    f.repin_profile();
    let contract = f.finish().unwrap();
    let tasks = f.tasks(&contract, &["compile", "link"]);
    f.graph(&contract, &tasks, None).unwrap();
    assemble_build_files(
        &f.bound(),
        &document(DocumentKind::Project, f.project.clone()),
        &contract,
        &f.required(),
        &tasks,
        &results(&f, &tasks),
    )
    .unwrap();
    let run = f.tasks(&contract, &["compile", "link", "run"]);
    code(f.graph(&contract, &run, None), "EXECUTION003");
}
