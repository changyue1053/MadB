#![cfg(unix)]
use madb_app::{
    Digest, Id, LogicalPath, Result,
    dag::{GraphControl, GraphStatus},
    pipeline::*,
    ports::FileReference,
    process::{ProcessPolicy, ProcessRequest},
};
use madb_host::{LocalWorkspace, artifact::LocalArtifactExecutor};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "madb-artifact-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(fs::canonicalize(path).unwrap()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
}
fn remove(path: &Path) {
    if fs::symlink_metadata(path).unwrap().is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        for item in fs::read_dir(path).unwrap() {
            remove(&item.unwrap().path());
        }
        fs::remove_dir(path).unwrap();
    } else {
        fs::remove_file(path).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        remove(&self.0);
    }
}
fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn reference(p: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: path(p),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
struct Recipe {
    invocation: ArtifactInvocation,
    tool_root: PathBuf,
}
impl ArtifactRecipe for Recipe {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<ArtifactDeclaration> {
        assert_eq!(stdout, b"manifest");
        let identity = Digest::of_bytes(stdout);
        Ok(ArtifactDeclaration {
            kind: ArtifactKind::Generation,
            identity: identity.clone(),
            input_digest: Digest::of_bytes(b"source"),
            task_digest: Digest::of_bytes(b"task"),
            profile_digest: Digest::of_bytes(b"profile"),
            generation_id: Some(identity),
            files: vec![reference("api.h", b"header")],
        })
    }
}
fn setup(dir: &Directory, tail: &str) -> Recipe {
    // Resolve the fixture's installed executable once: /bin/sh is a symlink
    // on some hosts. The production reader still refuses symbolic-link images.
    let shell_path = fs::canonicalize("/bin/sh").unwrap();
    let shell = fs::read(&shell_path).unwrap();
    let shell_name = shell_path.file_name().unwrap().to_str().unwrap();
    let script = format!(
        "printf header > api.h\nprintf manifest > manifest.json\n{tail}\nprintf manifest\n"
    );
    fs::write(dir.0.join("recipe.sh"), &script).unwrap();
    Recipe {
        tool_root: shell_path.parent().unwrap().to_owned(),
        invocation: ArtifactInvocation {
            id: Id::new("artifact").unwrap(),
            process: ProcessRequest {
                tool: reference(shell_name, &shell),
                working_directory: path("work"),
                arguments: vec!["../inputs/recipe.sh".to_owned()],
                environment: BTreeMap::new(),
                stdin: b"real request bytes".to_vec(),
            },
            inputs: vec![reference("recipe.sh", script.as_bytes())],
            response_path: path("manifest.json"),
        },
    }
}
#[test]
fn real_artifact_execution_captures_manifest_and_retains_drained_owner() {
    let dir = Directory::new();
    let recipe = setup(&dir, "");
    let source = LocalWorkspace::open(&dir.0).unwrap();
    let mut executor = LocalArtifactExecutor::new(
        &dir.0,
        &path("run"),
        &recipe.tool_root,
        ProcessPolicy::default(),
    )
    .unwrap();
    let mut run = run_artifact(&recipe, &source, &mut executor, &GraphControl::default()).unwrap();
    assert_eq!(
        run.status,
        GraphStatus::Validated,
        "{:?}",
        run.node()
            .unwrap()
            .execution()
            .unwrap()
            .as_ref()
            .unwrap()
            .process
            .as_ref()
            .map(|p| &p.report)
    );
    assert_eq!(run.result().unwrap().manifest().1, b"manifest");
    let retained = run.node().unwrap().path().unwrap().to_owned();
    assert!(retained.join("work/api.h").is_file());
    assert!(run.node_mut().unwrap().take_value().is_none());
    assert!(run.node_mut().unwrap().poll_cleanup().unwrap());
    fs::write(retained.join("work/api.h"), b"changed").unwrap();
    assert_eq!(
        run.result()
            .unwrap()
            .files()
            .files()
            .find(|(f, _)| f.path == path("api.h"))
            .unwrap()
            .1,
        b"header"
    );
}
#[test]
fn manifest_mismatch_and_undeclared_files_reject_but_keep_actual_run() {
    for tail in ["printf changed > manifest.json", "printf extra > ignored"] {
        let dir = Directory::new();
        let recipe = setup(&dir, tail);
        let source = LocalWorkspace::open(&dir.0).unwrap();
        let mut executor = LocalArtifactExecutor::new(
            &dir.0,
            &path("run"),
            &recipe.tool_root,
            ProcessPolicy::default(),
        )
        .unwrap();
        let mut run =
            run_artifact(&recipe, &source, &mut executor, &GraphControl::default()).unwrap();
        assert_eq!(run.status, GraphStatus::Failed);
        assert!(run.result().is_none());
        let node = run.node_mut().unwrap();
        assert!(node.path().unwrap().exists());
        assert!(
            node.execution()
                .unwrap()
                .as_ref()
                .unwrap()
                .process
                .as_ref()
                .unwrap()
                .report
                .diagnostic
                .is_some()
        );
        assert!(node.take_value().is_none());
        assert!(node.poll_cleanup().unwrap());
    }
}
#[test]
fn conflicting_run_directory_is_not_claimed_or_removed() {
    let dir = Directory::new();
    let recipe = setup(&dir, "");
    fs::create_dir(dir.0.join("run")).unwrap();
    fs::write(dir.0.join("run/sentinel"), b"existing").unwrap();
    let source = LocalWorkspace::open(&dir.0).unwrap();
    let mut executor = LocalArtifactExecutor::new(
        &dir.0,
        &path("run"),
        &recipe.tool_root,
        ProcessPolicy::default(),
    )
    .unwrap();
    let run = run_artifact(&recipe, &source, &mut executor, &GraphControl::default()).unwrap();
    assert_eq!(run.status, GraphStatus::Failed);
    assert!(
        run.node()
            .unwrap()
            .preparation_error()
            .unwrap()
            .retained_path
            .is_none()
    );
    assert_eq!(fs::read(dir.0.join("run/sentinel")).unwrap(), b"existing");
}
