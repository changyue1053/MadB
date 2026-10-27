use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = crate::locations::temporary_parent().unwrap().join(format!(
            "madb-init-execution-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> LockedInitializationFile {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        LockedInitializationFile {
            path,
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        }
    }
    fn final_project(&self) -> FinalProjectExecution {
        let file = self.file("author.rs", b"original author");
        FinalProjectExecution {
            root: self.0.clone(),
            directory: open_absolute_directory(&self.0).unwrap(),
            initial: vec![FileReference {
                path: LogicalPath::new("author.rs").unwrap(),
                digest: file.digest,
                size: file.size,
            }],
            runtime: std::cell::RefCell::new(Vec::new()),
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn missing_and_wrong_initial_bytes_have_distinct_categories() {
    let t = Tree::new();
    let file = t.file("tool", b"fixed");
    file.verify().unwrap();
    fs::write(&file.path, b"larger wrong bytes").unwrap();
    assert_eq!(file.verify().unwrap_err().kind, ErrorKind::InvalidInput);
    fs::remove_file(&file.path).unwrap();
    assert_eq!(file.verify().unwrap_err().kind, ErrorKind::Host);
}
#[test]
fn tool_observation_cannot_substitute_a_different_image() {
    let t = Tree::new();
    let file = t.file("tool", b"fixed");
    let mut tool = LockedInitializationTool {
        observation: ToolObservation {
            id: madb_app::Id::new("fixed").unwrap(),
            version: "1".into(),
            sha256: file.digest.clone(),
            host: madb_app::Id::new("host").unwrap(),
            target: madb_app::Id::new("target").unwrap(),
            source: "installed://fixed".into(),
            arguments: vec![],
        },
        file,
    };
    tool.verify().unwrap();
    fs::write(&tool.file.path, b"changed").unwrap();
    assert_eq!(tool.revalidate().unwrap_err().kind, ErrorKind::Conflict);
    tool.observation.sha256 = Digest::of_bytes(b"other");
    assert_eq!(tool.verify().unwrap_err().kind, ErrorKind::InvalidInput);
}
#[test]
fn final_root_replacement_cannot_redirect_subsequent_runtime_writes() {
    let t = Tree::new();
    let project = t.final_project();
    let held = t.0.with_extension("held");
    fs::rename(&t.0, &held).unwrap();
    fs::create_dir(&t.0).unwrap();
    fs::write(t.0.join("author.rs"), b"original author").unwrap();
    assert_eq!(
        project.verify_initial().unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert!(project.create_native_root().is_err());
    assert!(!t.0.join(".madb").exists());
    fs::remove_dir_all(held).unwrap();
}
#[test]
fn materialized_runtime_is_no_overwrite_and_is_rechecked_as_input() {
    let t = Tree::new();
    let project = t.final_project();
    project.create_native_root().unwrap();
    let path = LogicalPath::new("scripts/helper.py").unwrap();
    project.write_native_new(&path, b"trusted").unwrap();
    assert_eq!(
        project.write_native_new(&path, b"other").unwrap_err().kind,
        ErrorKind::Conflict
    );
    project.verify_initial().unwrap();
    fs::write(project.native_root().join("scripts/helper.py"), b"changed").unwrap();
    assert_eq!(
        project.verify_initial().unwrap_err().kind,
        ErrorKind::Conflict
    );
}
#[test]
fn initial_author_drift_is_not_accepted_as_a_new_source() {
    let t = Tree::new();
    let project = t.final_project();
    fs::write(t.0.join("author.rs"), b"modified author").unwrap();
    assert_eq!(
        project.verify_initial().unwrap_err().kind,
        ErrorKind::Conflict
    );
}
#[test]
fn generated_executable_capture_rejects_file_and_ancestor_aliases() {
    let t = Tree::new();
    let file = t.file("consumer", b"image");
    capture_generated_executable(&file.path).unwrap();
    std::os::unix::fs::symlink(&file.path, t.0.join("alias")).unwrap();
    assert!(capture_generated_executable(&t.0.join("alias")).is_err());
    fs::create_dir(t.0.join("real")).unwrap();
    fs::write(t.0.join("real/image"), b"image").unwrap();
    std::os::unix::fs::symlink("real", t.0.join("redirect")).unwrap();
    assert!(capture_generated_executable(&t.0.join("redirect/image")).is_err());
}

fn true_tool() -> LockedInitializationTool {
    let path = fs::canonicalize("/usr/bin/true").unwrap();
    let file = capture_generated_executable(&path).unwrap();
    LockedInitializationTool {
        observation: ToolObservation {
            id: madb_app::Id::new("test-true").unwrap(),
            version: "observed-test-image".into(),
            sha256: file.digest.clone(),
            host: madb_app::Id::new("host").unwrap(),
            target: madb_app::Id::new("target").unwrap(),
            source: "test://true".into(),
            arguments: vec![],
        },
        file,
    }
}
#[test]
fn cancellation_remains_live_through_real_process_validation() {
    let tree = Tree::new();
    let project = tree.final_project();
    let tool = true_tool();
    let control = ExecutionControl::default();
    let mut reached = false;
    let output = project
        .run(
            InitializationProcess {
                tool: &tool,
                arguments: vec![],
                environment: BTreeMap::new(),
                policy: ProcessPolicy::default(),
            },
            &control,
            |_| {
                reached = true;
                control.cancel();
                Ok(())
            },
        )
        .unwrap();
    assert!(reached);
    assert_eq!(
        output.execution.admission,
        madb_app::process::ResultAdmission::RejectedByCancellation
    );
    assert!(output.validation_error.is_none());
    assert!(
        output
            .execution
            .process
            .as_ref()
            .unwrap()
            .report
            .cleanup
            .locally_complete()
    );
}
#[test]
fn typed_validation_failure_is_retained_even_if_cancellation_is_concurrent() {
    let tree = Tree::new();
    let project = tree.final_project();
    let tool = true_tool();
    let control = ExecutionControl::default();
    let mut reached = false;
    let output = project
        .run(
            InitializationProcess {
                tool: &tool,
                arguments: vec![],
                environment: BTreeMap::new(),
                policy: ProcessPolicy::default(),
            },
            &control,
            |_| {
                reached = true;
                control.cancel();
                Err(conflict("test", "frozen input drift"))
            },
        )
        .unwrap();
    assert!(reached);
    assert_eq!(output.validation_error.unwrap().kind, ErrorKind::Conflict);
    assert!(output.execution.process.is_some());
}
#[test]
fn cancellation_before_process_keeps_a_precise_no_spawn_observation() {
    let tree = Tree::new();
    let project = tree.final_project();
    let tool = true_tool();
    let control = ExecutionControl::default();
    control.cancel();
    let output = project
        .run(
            InitializationProcess {
                tool: &tool,
                arguments: vec![],
                environment: BTreeMap::new(),
                policy: ProcessPolicy::default(),
            },
            &control,
            |_| panic!("no validation after pre-start cancellation"),
        )
        .unwrap();
    assert_eq!(
        output.execution.admission,
        madb_app::process::ResultAdmission::RejectedByCancellation
    );
    assert!(output.execution.process.is_none());
}

#[test]
fn frozen_tool_drift_at_run_entry_is_conflict_before_spawn() {
    let tree = Tree::new();
    let project = tree.final_project();
    let mut tool = true_tool();
    tool.file.digest = Digest::of_bytes(b"wrong frozen identity");
    tool.observation.sha256 = tool.file.digest.clone();
    let result = project.run(
        InitializationProcess {
            tool: &tool,
            arguments: vec![],
            environment: BTreeMap::new(),
            policy: ProcessPolicy::default(),
        },
        &ExecutionControl::default(),
        |_| panic!("must not run"),
    );
    assert_eq!(result.err().unwrap().kind, ErrorKind::Conflict);
}
