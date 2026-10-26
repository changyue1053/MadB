#![cfg(unix)]
use madb_app::{
    Digest, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
};
use madb_host::{LocalWorkspace, extension_files::LocalExtensionWorkspace};
use std::{
    fs, io,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "madb-extension-files-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(fs::canonicalize(path).unwrap()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("{e}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fn remove(p: &Path) {
            if fs::symlink_metadata(p).unwrap().is_dir() {
                fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
                for e in fs::read_dir(p).unwrap() {
                    remove(&e.unwrap().path());
                }
                fs::remove_dir(p).unwrap();
            } else {
                fs::remove_file(p).unwrap();
            }
        }
        remove(&self.0);
    }
}
fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn reference(p: &str, b: &[u8]) -> FileReference {
    FileReference {
        path: path(p),
        digest: Digest::of_bytes(b),
        size: b.len() as u64,
    }
}
fn prepared(scratch: &Scratch) -> LocalExtensionWorkspace {
    let source = scratch.0.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("request.json"), b"input").unwrap();
    let frozen = capture_inputs(
        &LocalWorkspace::open(&source).unwrap(),
        vec![reference("request.json", b"input")],
    )
    .unwrap();
    LocalExtensionWorkspace::create(&scratch.0, &path("run"), frozen, &path("outputs/task"))
        .unwrap()
}
fn capture(workspace: &LocalExtensionWorkspace) -> madb_app::Result<FrozenInputs> {
    capture_inputs(
        workspace.output_workspace(),
        vec![reference("result.json", b"output")],
    )
}
#[test]
fn private_domains_preserve_input_bytes_and_return_only_frozen_output_bytes() {
    let scratch = Scratch::new();
    let work = prepared(&scratch);
    assert_eq!(
        fs::metadata(work.work_path().join("inputs/request.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o400
    );
    assert_eq!(
        fs::metadata(work.work_path().join("inputs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o500
    );
    fs::write(work.output_path().join("result.json"), b"output").unwrap();
    let frozen = capture(&work).unwrap();
    work.verify_outputs(&frozen).unwrap();
    fs::write(work.output_path().join("result.json"), b"changed").unwrap();
    assert!(work.verify_outputs(&frozen).is_err());
    assert_eq!(frozen.files().next().unwrap().1, b"output");
}
#[test]
fn symlink_hardlink_directory_and_fifo_results_are_rejected_without_blocking() {
    for mode in ["symlink", "hardlink", "directory", "fifo"] {
        let scratch = Scratch::new();
        let work = prepared(&scratch);
        let target = work.output_path().join("result.json");
        let outside = scratch.0.join("outside");
        fs::write(&outside, b"output").unwrap();
        match mode {
            "symlink" => symlink(&outside, &target).unwrap(),
            "hardlink" => fs::hard_link(&outside, &target).unwrap(),
            "directory" => fs::create_dir(&target).unwrap(),
            "fifo" => {
                assert!(
                    std::process::Command::new("mkfifo")
                        .arg(&target)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            _ => unreachable!(),
        }
        assert!(capture(&work).is_err(), "accepted {mode}");
    }
}
#[test]
fn aliases_extra_files_and_intermediate_links_fail_exact_inventory() {
    for mode in ["case", "extra", "parent-link"] {
        let scratch = Scratch::new();
        let work = prepared(&scratch);
        match mode {
            "case" => fs::write(work.output_path().join("RESULT.json"), b"output").unwrap(),
            "extra" => {
                fs::write(work.output_path().join("result.json"), b"output").unwrap();
                fs::write(work.output_path().join("undeclared"), b"x").unwrap();
            }
            "parent-link" => {
                let outside = scratch.0.join("outside");
                fs::create_dir(&outside).unwrap();
                fs::write(outside.join("result.json"), b"output").unwrap();
                symlink(&outside, work.output_path().join("alias")).unwrap();
                assert!(
                    work.output_workspace()
                        .read(&path("alias/result.json"), 100)
                        .is_err()
                );
                continue;
            }
            _ => unreachable!(),
        }
        if let Ok(frozen) = capture(&work) {
            assert!(work.verify_outputs(&frozen).is_err(), "accepted {mode}");
        }
    }
}
#[test]
fn changed_inputs_replaced_domains_and_unmanaged_work_entries_are_rejected() {
    for mode in ["input", "domain", "work"] {
        let scratch = Scratch::new();
        let work = prepared(&scratch);
        match mode {
            "input" => {
                let input = work.work_path().join("inputs/request.json");
                fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(input, b"other").unwrap();
            }
            "domain" => {
                fs::rename(work.output_path(), work.work_path().join("outputs/saved")).unwrap();
                fs::create_dir(work.output_path()).unwrap();
            }
            "work" => {
                fs::write(work.work_path().join("unmanaged"), b"x").unwrap();
            }
            _ => unreachable!(),
        }
        assert!(work.verify_inputs().is_err(), "accepted {mode}");
    }
}
#[test]
fn allocation_is_exclusive_and_invalid_domains_never_allocate() {
    let scratch = Scratch::new();
    let work = prepared(&scratch);
    let source = LocalWorkspace::open(&scratch.0.join("source")).unwrap();
    let freeze = || capture_inputs(&source, vec![reference("request.json", b"input")]).unwrap();
    assert!(
        LocalExtensionWorkspace::create(&scratch.0, &path("run"), freeze(), &path("outputs/task"))
            .is_err()
    );
    assert!(
        LocalExtensionWorkspace::create(&scratch.0, &path("bad"), freeze(), &path("inputs/task"))
            .is_err()
    );
    assert!(!scratch.0.join("bad").exists());
    work.verify_inputs().unwrap();
}
