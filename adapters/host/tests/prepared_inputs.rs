#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use madb_app::inputs::{FrozenInputs, capture_inputs};
use madb_app::ports::{FileReference, ReadWorkspace};
use madb_app::process::{
    CancelDecision, ExecutionControl, ProcessPolicy, ProcessRequest, ResultAdmission,
};
use madb_app::{Digest, ErrorKind, LogicalPath, Result};
use madb_host::inputs::{PreparationError, PreparedRun};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// These tests never execute processes, so they retain exclusive ownership of
/// every temporary tree and can restore directory permissions before removal.
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        loop {
            let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "madb-prepared-inputs-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(fs::canonicalize(path).unwrap()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create isolated test directory: {error}"),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = remove_owned_tree(&self.0) {
            if std::thread::panicking() {
                eprintln!(
                    "cannot remove owned test directory {}: {error}",
                    self.0.display()
                );
            } else {
                panic!(
                    "cannot remove owned test directory {}: {error}",
                    self.0.display()
                );
            }
        }
    }
}

fn remove_owned_tree(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            remove_owned_tree(&entry?.path())?;
        }
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

/// A logical inventory may be valid while a filesystem rejects a component.
/// This source deliberately does not impose an OS filename-length limit.
struct MemorySource {
    reference: FileReference,
    bytes: Vec<u8>,
}

impl ReadWorkspace for MemorySource {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        if path == &self.reference.path {
            assert!(self.bytes.len() <= limit);
            Ok(Some(self.bytes.clone()))
        } else {
            Ok(None)
        }
    }
}

fn snapshot(path: &str) -> FrozenInputs {
    let bytes = b"captured input".to_vec();
    let source = MemorySource {
        reference: FileReference {
            path: LogicalPath::new(path).unwrap(),
            digest: Digest::of_bytes(&bytes),
            size: bytes.len() as u64,
        },
        bytes,
    };
    capture_inputs(&source, vec![source.reference.clone()]).unwrap()
}

fn logical(path: &str) -> LogicalPath {
    LogicalPath::new(path).unwrap()
}

fn failed_preparation(
    result: std::result::Result<PreparedRun, PreparationError>,
) -> PreparationError {
    match result {
        Ok(_) => panic!("preparation unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn assert_preparation_error(error: &PreparationError) {
    assert_eq!(error.error.kind, ErrorKind::Host);
    assert_eq!(error.error.findings.len(), 1);
    assert_eq!(error.error.findings[0].code, "INPUT005");
}

#[test]
fn materialization_failure_retains_only_the_newly_owned_tree() {
    let parent = TestDirectory::new();
    // Within LogicalPath's 1024-byte limit, but beyond NAME_MAX on supported
    // Linux/macOS filesystems. Capture succeeds independently of filesystem IO.
    let filename = "x".repeat(1000);
    let frozen = snapshot(&filename);
    let expected = parent.path().join("failed-run");
    let error = failed_preparation(PreparedRun::create(
        parent.path(),
        &logical("failed-run"),
        frozen,
    ));

    assert_preparation_error(&error);
    assert_eq!(error.error.findings[0].subject, filename);
    assert_eq!(
        error.error.findings[0].message,
        rustix::io::Errno::NAMETOOLONG.to_string()
    );
    assert_eq!(error.retained_path.as_deref(), Some(expected.as_path()));
    assert_eq!(expected.parent(), Some(parent.path()));
    assert!(expected.is_dir());
    assert!(expected.join("inputs").is_dir());
    assert!(expected.join("work").is_dir());
    assert_eq!(fs::read_dir(expected.join("inputs")).unwrap().count(), 0);
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    drop(error);
    assert!(
        expected.is_dir(),
        "dropping the error must retain its partial tree"
    );
}

#[test]
fn dropping_an_unexecuted_preparation_keeps_inputs_and_work() {
    let parent = TestDirectory::new();
    let prepared = PreparedRun::create(
        parent.path(),
        &logical("prepared-run"),
        snapshot("src/input.txt"),
    )
    .unwrap();
    let path = prepared.path().to_owned();
    assert_eq!(path, parent.path().join("prepared-run"));
    assert_eq!(
        fs::metadata(path.join("inputs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o200,
        0
    );

    drop(prepared);

    assert!(path.is_dir());
    assert!(path.join("inputs/src").is_dir());
    assert!(path.join("work").is_dir());
    assert_eq!(
        fs::read(path.join("inputs/src/input.txt")).unwrap(),
        b"captured input"
    );
    assert_eq!(fs::read_dir(path.join("work")).unwrap().count(), 0);
}

#[test]
fn an_existing_directory_is_never_overwritten_or_claimed() {
    let parent = TestDirectory::new();
    let existing = parent.path().join("existing");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("keep.txt"), b"original directory").unwrap();
    let before = fs::metadata(&existing).unwrap();
    let error = failed_preparation(PreparedRun::create(
        parent.path(),
        &logical("existing"),
        snapshot("input.txt"),
    ));

    assert_preparation_error(&error);
    assert!(error.retained_path.is_none());
    let after = fs::metadata(&existing).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert_eq!(
        fs::read(existing.join("keep.txt")).unwrap(),
        b"original directory"
    );
    assert_eq!(fs::read_dir(&existing).unwrap().count(), 1);
    assert!(!existing.join("inputs").exists());
    assert!(!existing.join("work").exists());
}

#[test]
fn an_existing_file_is_never_overwritten_or_claimed() {
    let parent = TestDirectory::new();
    let existing = parent.path().join("existing");
    fs::write(&existing, b"original file").unwrap();
    let before = fs::metadata(&existing).unwrap();
    let error = failed_preparation(PreparedRun::create(
        parent.path(),
        &logical("existing"),
        snapshot("input.txt"),
    ));

    assert_preparation_error(&error);
    assert!(error.retained_path.is_none());
    let after = fs::metadata(&existing).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert!(after.is_file());
    assert_eq!(fs::read(&existing).unwrap(), b"original file");
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
}

#[test]
fn nested_and_non_ascii_run_names_are_rejected_before_allocation() {
    let parent = TestDirectory::new();
    for name in ["nested/run", "运行"] {
        let error = failed_preparation(PreparedRun::create(
            parent.path(),
            &logical(name),
            snapshot("input.txt"),
        ));
        assert_preparation_error(&error);
        assert!(error.retained_path.is_none());
        assert_eq!(error.error.findings[0].subject, "name");
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
}

fn request() -> ProcessRequest {
    ProcessRequest {
        tool: FileReference {
            path: logical("unavailable-tool"),
            digest: Digest::of_bytes(b"tool"),
            size: 4,
        },
        working_directory: logical("work"),
        arguments: vec![],
        environment: BTreeMap::new(),
        stdin: vec![],
    }
}

#[test]
fn runner_initialization_failure_consumes_the_execution_control() {
    let parent = TestDirectory::new();
    let tool_root = parent.path().join("missing-tools");
    let control = ExecutionControl::default();
    let first = PreparedRun::create(parent.path(), &logical("first-run"), snapshot("input.txt"))
        .unwrap()
        .execute(&tool_root, ProcessPolicy::default(), &request(), &control);

    let error = first
        .execution()
        .as_ref()
        .err()
        .expect("missing tool root must fail initialization");
    assert_eq!(error.code, "EXEC010");
    assert_eq!(error.stage, "configuration");
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert!(first.path().join("inputs/input.txt").is_file());
    assert!(first.path().join("work").is_dir());
    assert!(!tool_root.exists());

    let second = PreparedRun::create(parent.path(), &logical("second-run"), snapshot("input.txt"))
        .unwrap()
        .execute(&tool_root, ProcessPolicy::default(), &request(), &control);

    let repeated = second
        .execution()
        .as_ref()
        .err()
        .expect("a used control must reject a second run");
    assert_eq!(repeated.code, "EXEC001");
    assert_eq!(repeated.stage, "admission");
    assert!(second.path().join("inputs/input.txt").is_file());
    assert!(second.path().join("work").is_dir());
}

#[test]
fn cancellation_before_execution_precedes_runner_initialization() {
    let parent = TestDirectory::new();
    let tool_root = parent.path().join("missing-tools");
    let prepared = PreparedRun::create(
        parent.path(),
        &logical("cancelled-run"),
        snapshot("input.txt"),
    )
    .unwrap();
    let control = ExecutionControl::default();
    assert_eq!(control.cancel(), CancelDecision::Requested);

    let retained = prepared.execute(&tool_root, ProcessPolicy::default(), &request(), &control);
    let execution = retained
        .execution()
        .as_ref()
        .expect("cancellation must prevent initialization");
    assert_eq!(execution.admission, ResultAdmission::RejectedByCancellation);
    assert!(execution.process.is_none());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert!(retained.path().join("inputs/input.txt").is_file());
    assert!(retained.path().join("work").is_dir());
    assert!(!tool_root.exists());
}

#[test]
fn graph_directory_is_exclusive_and_drop_retains_it() {
    use madb_host::dag::LocalGraphExecutor;
    let parent = TestDirectory::new();
    let graph = LocalGraphExecutor::new(
        parent.path(),
        &logical("graph"),
        parent.path(),
        ProcessPolicy::default(),
    )
    .unwrap();
    let expected = parent.path().join("graph");
    assert_eq!(graph.path(), expected);
    assert_eq!(
        fs::metadata(&expected).unwrap().permissions().mode() & 0o777,
        0o700
    );
    fs::write(expected.join("keep"), b"retained").unwrap();
    let error = match LocalGraphExecutor::new(
        parent.path(),
        &logical("graph"),
        parent.path(),
        ProcessPolicy::default(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("existing graph directory was claimed"),
    };
    assert!(error.retained_path.is_none());
    assert_eq!(fs::read(expected.join("keep")).unwrap(), b"retained");
    drop(graph);
    assert!(expected.exists());
}

#[test]
fn graph_directory_rejects_nested_names_before_allocation() {
    use madb_host::dag::LocalGraphExecutor;
    let parent = TestDirectory::new();
    let error = match LocalGraphExecutor::new(
        parent.path(),
        &logical("nested/graph"),
        parent.path(),
        ProcessPolicy::default(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("nested graph name accepted"),
    };
    assert_preparation_error(&error);
    assert!(error.retained_path.is_none());
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn standalone_task_rejects_dependencies_before_source_reads_or_allocation() {
    use madb_app::{
        Id,
        task::{BuildRecipe, TaskFileReferences, TaskInvocation},
    };
    use madb_host::task::PreparedTask;
    struct Recipe {
        invocation: TaskInvocation,
        dependencies: Vec<Id>,
    }
    impl BuildRecipe for Recipe {
        fn invocation(&self) -> &TaskInvocation {
            &self.invocation
        }
        fn dependencies(&self) -> &[Id] {
            &self.dependencies
        }
        fn validate_result(&self, _: &[u8]) -> Result<TaskFileReferences> {
            panic!("must not execute")
        }
    }
    struct NoRead;
    impl ReadWorkspace for NoRead {
        fn read(&self, _: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
            panic!("must reject dependencies before any source reads")
        }
    }
    let parent = TestDirectory::new();
    let input = snapshot("input").references().next().unwrap().clone();
    let recipe = Recipe {
        invocation: TaskInvocation {
            id: Id::new("consumer").unwrap(),
            process: ProcessRequest {
                tool: input.clone(),
                working_directory: logical("work"),
                arguments: vec![],
                environment: BTreeMap::new(),
                stdin: vec![],
            },
            inputs: vec![input],
            outputs: vec![logical("output")],
        },
        dependencies: vec![Id::new("producer").unwrap()],
    };
    let error = match PreparedTask::create(parent.path(), &logical("task"), &NoRead, &recipe) {
        Err(error) => error,
        Ok(_) => panic!("standalone task admitted dependency-bearing recipe"),
    };
    assert_eq!(error.error.findings[0].code, "TASK001");
    assert!(error.retained_path.is_none());
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}
