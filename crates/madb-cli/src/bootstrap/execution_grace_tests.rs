//! A real process owner drains a separately owned process group before releasing
//! its lock. Reusing this test executable keeps the fixture independent of Python.
use super::*;
use madb_app::process::{
    ExecutionControl, ProcessRequest, StopReason, Termination, execute_process,
};
use madb_host::{cancellation::CancellationSignals, process::LocalProcessRunner};
use std::{
    fs::{self, OpenOptions},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Instant,
};

const ROLE: &str = "MADB_TEST_TERMINATION_GRACE_ROLE";
const DIRECTORY: &str = "MADB_TEST_TERMINATION_GRACE_DIRECTORY";
const HELPER: &str = "bootstrap::execution::grace_tests::termination_grace_fixture";

struct FixtureDirectory(PathBuf);
impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct OwnedFixtureChild(Child);
impl Drop for OwnedFixtureChild {
    fn drop(&mut self) {
        // The helper has no descendants. This fallback runs on failed assertions.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait_until(condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "fixture barrier timed out"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn termination_grace_fixture() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os(DIRECTORY).unwrap());
    if role == "retry" {
        let lock = OpenOptions::new()
            .write(true)
            .open(directory.join("compile.lock"))
            .unwrap();
        lock.try_lock()
            .expect("prior owner must have released its lock");
        assert!(directory.join("child-cleaned").exists());
        assert!(directory.join("owner-cleaned").exists());
        return;
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let observed = cancelled.clone();
    let _signals = CancellationSignals::install_action(move || {
        observed.store(true, Ordering::Release);
    })
    .unwrap();
    if role == "child" {
        fs::write(directory.join("child-ready"), b"ready").unwrap();
        wait_until(|| cancelled.load(Ordering::Acquire));
        thread::sleep(Duration::from_millis(700));
        fs::write(directory.join("child-cleaned"), b"cleaned").unwrap();
        return;
    }
    assert_eq!(role, "owner");
    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("compile.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let mut child = OwnedFixtureChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", HELPER, "--nocapture"])
            .env(ROLE, "child")
            .env(DIRECTORY, &directory)
            .process_group(0)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_until(|| directory.join("child-ready").exists());
    fs::write(directory.join("owner-ready"), b"ready").unwrap();
    wait_until(|| cancelled.load(Ordering::Acquire));
    // A negative PGID is an operand, not another kill option.
    assert!(
        Command::new("/bin/kill")
            .args(["-TERM", "--", &format!("-{}", child.0.id())])
            .status()
            .unwrap()
            .success()
    );
    assert!(child.0.wait().unwrap().success());
    drop(lock);
    fs::write(directory.join("owner-cleaned"), b"cleaned").unwrap();
}

#[test]
fn explicit_grace_drains_a_child_group_and_allows_lock_retry() {
    let directory =
        FixtureDirectory(std::env::temp_dir().join(locations::unique_run_name().as_str()));
    fs::create_dir_all(directory.0.join("work")).unwrap();
    let tool = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let image = fs::read(&tool).unwrap();
    let options = ExecutionOptions {
        command: "build".into(),
        root: directory.0.clone(),
        execution: "unused.json".into(),
        tool_root: None,
        process_timeout: Duration::from_secs(10),
        process_termination_grace: Duration::from_millis(2000),
        check: false,
        task: Some("metadata".into()),
        active_tasks: Vec::new(),
    };
    let runner = LocalProcessRunner::new(
        tool.parent().unwrap(),
        &directory.0,
        policy(
            &options,
            [ROLE.to_owned(), DIRECTORY.to_owned()].into_iter(),
        ),
    )
    .unwrap();
    let mut request = ProcessRequest {
        tool: FileReference {
            path: logical(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
            digest: Digest::of_bytes(&image),
            size: image.len() as u64,
        },
        working_directory: logical("work").unwrap(),
        arguments: vec!["--exact".into(), HELPER.into(), "--nocapture".into()],
        environment: BTreeMap::from([
            (ROLE.into(), "owner".into()),
            (DIRECTORY.into(), directory.0.to_str().unwrap().into()),
        ]),
        stdin: Vec::new(),
    };
    let control = ExecutionControl::default();
    let cancellation = control.clone();
    let barrier = directory.0.join("owner-ready");
    let cancel = thread::spawn(move || {
        let start = Instant::now();
        while !barrier.exists() && start.elapsed() < Duration::from_secs(8) {
            thread::sleep(Duration::from_millis(5));
        }
        cancellation.cancel();
        barrier.exists()
    });
    let execution = execute_process(&runner, &request, &control).unwrap();
    assert!(
        cancel.join().unwrap(),
        "owner and child were ready before cancellation"
    );
    let run = execution.process.expect("owner process was started");
    assert!(
        matches!(run.report.termination,
        Termination::Stopped { reason: StopReason::Cancellation, forced: false, exit: Some(exit) }
            if exit.code == Some(0)),
        "{:?}",
        run.report
    );
    assert!(run.report.cleanup.locally_complete());
    assert!(run.pending_cleanup.is_none());
    assert!(directory.0.join("child-cleaned").exists());
    assert!(directory.0.join("owner-cleaned").exists());
    request.environment.insert(ROLE.into(), "retry".into());
    let retry = execute_process(&runner, &request, &ExecutionControl::default()).unwrap();
    let retry = retry.process.expect("retry process was started");
    assert!(
        matches!(retry.report.termination, Termination::Exited(exit) if exit.code == Some(0)),
        "{:?}",
        retry.report
    );
    assert!(retry.report.cleanup.locally_complete());
    assert!(retry.pending_cleanup.is_none());
}
