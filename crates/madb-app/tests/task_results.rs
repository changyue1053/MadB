use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    ports::{FileReference, ReadWorkspace},
    process::ProcessRequest,
    task::{BuildRecipe, TaskFileReferences, TaskInvocation, capture_task_result},
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn reference(name: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: path(name),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn invocation(outputs: &[&str]) -> TaskInvocation {
    TaskInvocation {
        id: Id::new("compile").unwrap(),
        process: ProcessRequest {
            tool: reference("tool", b"tool"),
            working_directory: path("work"),
            arguments: vec![],
            environment: BTreeMap::new(),
            stdin: vec![],
        },
        inputs: vec![reference("input", b"input")],
        outputs: outputs.iter().map(|s| path(s)).collect(),
    }
}
struct Recipe {
    invocation: TaskInvocation,
    result: TaskFileReferences,
    fail: bool,
}
impl BuildRecipe for Recipe {
    fn invocation(&self) -> &TaskInvocation {
        &self.invocation
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<TaskFileReferences> {
        assert_eq!(stdout, b"independent payload");
        if self.fail {
            Err(QueryError::new(
                ErrorKind::InvalidInput,
                "TEST001",
                "payload",
                "bad result",
            ))
        } else {
            Ok(self.result.clone())
        }
    }
}
fn recipe() -> Recipe {
    Recipe {
        invocation: invocation(&["out/api.h"]),
        result: TaskFileReferences {
            outputs: vec![reference("out/api.h", b"declaration")],
            logs: vec![reference("logs/build.log", b"log")],
        },
        fail: false,
    }
}
struct Workspace {
    files: RefCell<BTreeMap<LogicalPath, Vec<u8>>>,
    reads: Cell<usize>,
    change_at: Option<usize>,
}
impl Workspace {
    fn normal() -> Self {
        Self {
            files: RefCell::new(BTreeMap::from([
                (path("out/api.h"), b"declaration".to_vec()),
                (path("logs/build.log"), b"log".to_vec()),
            ])),
            reads: Cell::new(0),
            change_at: None,
        }
    }
}
impl ReadWorkspace for Workspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.reads.set(self.reads.get() + 1);
        if self.change_at == Some(self.reads.get()) {
            return Ok(Some(b"changed".to_vec()));
        }
        let value = self.files.borrow().get(path).cloned();
        if let Some(value) = &value {
            assert!(value.len() <= limit);
        }
        Ok(value)
    }
}
fn code(error: QueryError) -> &'static str {
    error.findings[0].code
}

#[test]
fn captures_exact_outputs_and_logs_and_is_independent_of_later_work_changes() {
    let workspace = Workspace::normal();
    let captured = capture_task_result(&workspace, &recipe(), b"independent payload").unwrap();
    assert_eq!(workspace.reads.get(), 4);
    assert_eq!(captured.files().total_bytes(), 14);
    assert_eq!(
        captured
            .outputs()
            .map(|(f, b)| (f.path.as_str(), b))
            .collect::<Vec<_>>(),
        [("out/api.h", b"declaration".as_slice())]
    );
    assert_eq!(
        captured
            .logs()
            .map(|(f, b)| (f.path.as_str(), b))
            .collect::<Vec<_>>(),
        [("logs/build.log", b"log".as_slice())]
    );
    workspace
        .files
        .borrow_mut()
        .insert(path("out/api.h"), b"new content".to_vec());
    assert_eq!(captured.outputs().next().unwrap().1, b"declaration");
    assert_eq!(
        code(captured.files().verify(&workspace).unwrap_err()),
        "INPUT004"
    );
}

#[test]
fn invocation_rejects_cwd_and_output_aliases_ancestors_and_empty_inventory() {
    let mut wrong = invocation(&["out"]);
    wrong.process.working_directory = path("inputs");
    assert_eq!(code(wrong.validate().unwrap_err()), "TASK001");
    for outputs in [
        vec![],
        vec!["out", "out"],
        vec!["A/x", "a/y"],
        vec!["out", "out/api"],
        vec!["输出"],
    ] {
        assert_eq!(
            code(invocation(&outputs).validate().unwrap_err()),
            "TASK001"
        );
    }
    let mut wrong = invocation(&["out"]);
    wrong.inputs.push(reference("INPUT", b"input"));
    assert_eq!(code(wrong.validate().unwrap_err()), "INPUT001");
}

#[test]
fn output_path_budgets_apply_before_file_io() {
    let too_many: Vec<_> = (0..4097).map(|i| format!("file{i}")).collect();
    assert_eq!(
        code(
            invocation(&too_many.iter().map(String::as_str).collect::<Vec<_>>())
                .validate()
                .unwrap_err()
        ),
        "TASK001"
    );
    let deep = std::iter::repeat_n("dir", 33).collect::<Vec<_>>().join("/");
    assert_eq!(
        code(invocation(&[&deep]).validate().unwrap_err()),
        "TASK001"
    );
}

#[test]
fn payload_failure_never_reads_output_paths() {
    let workspace = Workspace::normal();
    let mut recipe = recipe();
    recipe.fail = true;
    assert_eq!(
        code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
        "TEST001"
    );
    assert_eq!(workspace.reads.get(), 0);
}

#[test]
fn mismatched_duplicate_or_missing_outputs_and_logs_are_rejected_before_io() {
    for case in 0..6 {
        let workspace = Workspace::normal();
        let mut recipe = recipe();
        match case {
            0 => recipe.result.outputs.clear(),
            1 => recipe
                .result
                .outputs
                .push(reference("other", b"declaration")),
            2 => recipe.result.outputs.push(recipe.result.outputs[0].clone()),
            3 => recipe.result.logs.clear(),
            4 => recipe.result.logs.push(recipe.result.logs[0].clone()),
            _ => recipe.result.logs = recipe.result.outputs.clone(),
        }
        assert_eq!(
            code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
            "TASK002"
        );
        assert_eq!(workspace.reads.get(), 0);
    }
}

#[test]
fn combined_output_log_case_aliases_and_ancestor_conflicts_are_rejected_before_io() {
    for log in ["OUT/trace.log", "out", "out/api.h/log"] {
        let workspace = Workspace::normal();
        let mut recipe = recipe();
        recipe.result.logs = vec![reference(log, b"log")];
        assert_eq!(
            code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
            "INPUT001"
        );
        assert_eq!(workspace.reads.get(), 0);
    }
}

#[test]
fn result_budget_is_combined_across_outputs_and_logs_before_io() {
    let workspace = Workspace::normal();
    let mut recipe = recipe();
    recipe.result.outputs[0].size = 32 * 1024 * 1024;
    recipe.result.logs = (0..4)
        .map(|i| {
            let mut f = reference(&format!("logs/log{i}"), b"");
            f.size = 32 * 1024 * 1024;
            f
        })
        .collect();
    assert_eq!(
        code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
        "INPUT002"
    );
    assert_eq!(workspace.reads.get(), 0);
    recipe.result.logs = (0..4096)
        .map(|i| reference(&format!("logs/log{i}"), b""))
        .collect();
    assert_eq!(
        code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
        "INPUT002"
    );
    assert_eq!(workspace.reads.get(), 0);
}

#[test]
fn mismatch_missing_and_capture_race_fail_without_a_partial_result() {
    let workspace = Workspace::normal();
    let mut recipe = recipe();
    recipe.result.outputs[0].digest = Digest::of_bytes(b"different");
    assert_eq!(
        code(capture_task_result(&workspace, &recipe, b"independent payload").unwrap_err()),
        "INPUT003"
    );
    let workspace = Workspace::normal();
    workspace.files.borrow_mut().remove(&path("out/api.h"));
    assert_eq!(
        code(
            capture_task_result(&workspace, &crate::recipe(), b"independent payload").unwrap_err()
        ),
        "INPUT003"
    );
    let mut workspace = Workspace::normal();
    workspace.change_at = Some(3);
    assert_eq!(
        code(
            capture_task_result(&workspace, &crate::recipe(), b"independent payload").unwrap_err()
        ),
        "INPUT004"
    );
}
