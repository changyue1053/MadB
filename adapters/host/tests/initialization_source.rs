#![cfg(target_os = "macos")]

use madb_app::{Digest, ErrorKind};
use madb_host::initialization_source::FrozenInitializationTool;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const CHILD_ROOT: &str = "MADB_TEST_INITIALIZATION_IMAGE_ROOT";
const CHILD_EXPECTATION: &str = "MADB_TEST_INITIALIZATION_IMAGE_EXPECTATION";
const DEADLINE: Duration = Duration::from_secs(15);

// This is a test-only subprocess entry, selected with --exact. It invokes only
// the public capture API. No production hook, alternate image path or identity
// injection is needed to reproduce replacement before its first file open.
#[test]
fn initialization_image_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    fs::write(root.join("ready"), b"running before capture").unwrap();
    let deadline = Instant::now() + DEADLINE;
    while !root.join("capture-now").is_file() {
        assert!(Instant::now() < deadline, "parent never released capture");
        thread::sleep(Duration::from_millis(10));
    }
    let expected = std::env::var(CHILD_EXPECTATION).unwrap();
    match FrozenInitializationTool::capture(env!("CARGO_PKG_VERSION")) {
        Ok(tool) => {
            assert_eq!(expected, "same-running-image", "replacement was accepted");
            let expected_digest = fs::read_to_string(root.join("expected-digest")).unwrap();
            assert_eq!(tool.identity().sha256.as_str(), expected_digest);
            tool.revalidate().unwrap();
            std::process::exit(0);
        }
        Err(error) => {
            assert_eq!(expected, "replaced-image", "unchanged image was rejected");
            assert_eq!(error.kind, ErrorKind::Conflict);
            std::process::exit(4);
        }
    }
}

#[test]
fn running_image_identity_is_checked_before_initial_capture() {
    for replacement in [None, Some(false), Some(true)] {
        check_before_capture_replacement(replacement);
    }
}

// None is the successful control. false replaces the pathname with non-image
// bytes; true uses an identical executable copy with a distinct inode. A hash
// equality check by itself must not pass the latter case.
fn check_before_capture_replacement(replacement: Option<bool>) {
    let sandbox = Sandbox::new();
    let source = std::env::current_exe().unwrap();
    let executable = sandbox.0.join("running image");
    fs::copy(&source, &executable).unwrap();
    let expected_bytes = fs::read(&executable).unwrap();
    let expected_digest = Digest::of_bytes(&expected_bytes);
    fs::write(sandbox.0.join("expected-digest"), expected_digest.as_str()).unwrap();
    let mut child = ChildGuard(Some(
        Command::new(&executable)
            .args([
                "--exact",
                "initialization_image_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ROOT, &sandbox.0)
            .env(
                CHILD_EXPECTATION,
                if replacement.is_some() {
                    "replaced-image"
                } else {
                    "same-running-image"
                },
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    child.await_ready(&sandbox.0.join("ready"));
    if let Some(identical) = replacement {
        let original_inode = fs::metadata(&executable).unwrap().ino();
        fs::rename(&executable, sandbox.0.join("loaded-image-retained")).unwrap();
        if identical {
            fs::copy(&source, &executable).unwrap();
            assert_eq!(fs::read(&executable).unwrap(), expected_bytes);
        } else {
            fs::write(
                &executable,
                b"replacement is not the running Mach-O image\n",
            )
            .unwrap();
        }
        assert_ne!(fs::metadata(&executable).unwrap().ino(), original_inode);
    }
    fs::write(sandbox.0.join("capture-now"), b"continue").unwrap();
    let output = child.finish();
    assert_eq!(
        output.status.code(),
        Some(if replacement.is_some() { 4 } else { 0 }),
        "case {replacement:?}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

struct ChildGuard(Option<Child>);
impl ChildGuard {
    fn await_ready(&mut self, path: &Path) {
        let deadline = Instant::now() + DEADLINE;
        while !path.is_file() {
            assert!(
                self.0.as_mut().unwrap().try_wait().unwrap().is_none(),
                "child exited before its capture barrier"
            );
            assert!(
                Instant::now() < deadline,
                "child did not reach capture barrier"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn finish(mut self) -> Output {
        let deadline = Instant::now() + DEADLINE;
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "child did not finish capture");
            thread::sleep(Duration::from_millis(10));
        }
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let path = madb_host::locations::temporary_parent()
            .unwrap()
            .join(madb_host::locations::unique_run_name().as_str());
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
