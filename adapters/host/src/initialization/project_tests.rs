use super::*;
use madb_app::use_cases::initialize::FileOwnership;
use madb_app::{Id, LogicalPath, process::CancelDecision};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let root = PathBuf::from(format!(
            "/private/tmp/madb-init-host-{}",
            crate::initialization::staging::nonce()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn output(&self) -> AbsolutePath {
        AbsolutePath::new(self.0.join("中文 空格工程").to_str().unwrap()).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn files(bytes: &[u8]) -> Vec<InitialFile> {
    vec![
        InitialFile::rendered(
            Id::new("root").unwrap(),
            LogicalPath::new("src/main.rs").unwrap(),
            FileOwnership::User,
            bytes.to_vec(),
        )
        .unwrap(),
        InitialFile::local_state(
            Id::new("root").unwrap(),
            LogicalPath::new("local/config.json").unwrap(),
        ),
    ]
}
fn directories() -> BTreeSet<String> {
    ["src", "local", "generated"].map(str::to_owned).into()
}
fn injected() -> madb_app::QueryError {
    failure(ErrorKind::Host, "injection", "expected test failure")
}

#[test]
fn creates_exact_bytes_and_empty_modules_without_local_state_placeholder() {
    let sandbox = Sandbox::new();
    let output = sandbox.output();
    let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
    let initial = files(b"frozen\0bytes");
    let observation = creator
        .commit_files(&initial, directories(), |_| Ok(()))
        .unwrap();
    assert_eq!(observation.root, output);
    assert_eq!(
        observation.files.unwrap(),
        initial
            .iter()
            .filter_map(InitialFile::reference)
            .collect::<Vec<_>>()
    );
    let root = PathBuf::from(output.as_str());
    assert_eq!(
        fs::read(root.join("src/main.rs")).unwrap(),
        b"frozen\0bytes"
    );
    assert!(root.join("generated").is_dir());
    assert!(!root.join("local/config.json").exists());
    assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 1);
}

#[test]
fn two_real_publishers_have_one_winner_and_no_overwrite() {
    let sandbox = Sandbox::new();
    let output = sandbox.output();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [b"writer-a".as_slice(), b"writer-b".as_slice()]
        .into_iter()
        .map(|bytes| {
            let output = output.clone();
            let barrier = barrier.clone();
            let bytes = bytes.to_vec();
            thread::spawn(move || {
                let mut creator =
                    LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
                let result = creator.commit_files(&files(&bytes), directories(), |event| {
                    if event.point == InitializationPoint::BeforePublish {
                        barrier.wait();
                    }
                    Ok(())
                });
                (bytes, result)
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    let winner = results.iter().find(|(_, result)| result.is_ok()).unwrap();
    assert!(winner.1.as_ref().unwrap().files.is_ok());
    let loser = results.iter().find(|(_, result)| result.is_err()).unwrap();
    assert_eq!(loser.1.as_ref().unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(
        fs::read(PathBuf::from(output.as_str()).join("src/main.rs")).unwrap(),
        winner.0
    );
    assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 1);
}

#[test]
fn competing_file_directory_and_symlink_are_never_replaced() {
    for kind in 0..3 {
        let sandbox = Sandbox::new();
        let output = sandbox.output();
        let target = PathBuf::from(output.as_str());
        let outside = sandbox.0.join("outside");
        fs::write(&outside, b"untouched").unwrap();
        let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
        let error = creator
            .commit_files(&files(b"ours"), directories(), |event| {
                if event.point == InitializationPoint::BeforePublish {
                    match kind {
                        0 => fs::write(&target, b"competitor").unwrap(),
                        1 => {
                            fs::create_dir(&target).unwrap();
                            fs::write(target.join("competitor"), b"kept").unwrap();
                        }
                        _ => std::os::unix::fs::symlink(&outside, &target).unwrap(),
                    }
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Conflict);
        assert_eq!(fs::read(&outside).unwrap(), b"untouched");
        match kind {
            0 => assert_eq!(fs::read(&target).unwrap(), b"competitor"),
            1 => assert_eq!(fs::read(target.join("competitor")).unwrap(), b"kept"),
            _ => assert!(
                fs::symlink_metadata(&target)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            ),
        }
    }
}

#[test]
fn parent_symlink_is_rejected_without_following_it() {
    let sandbox = Sandbox::new();
    let actual = sandbox.0.join("actual");
    fs::create_dir(&actual).unwrap();
    let link = sandbox.0.join("link");
    std::os::unix::fs::symlink(&actual, &link).unwrap();
    let output = AbsolutePath::new(link.join("new").to_str().unwrap()).unwrap();
    assert!(LocalProjectCreator::open(&output, CommitControl::default()).is_err());
    assert_eq!(fs::read_dir(actual).unwrap().count(), 0);
}

#[test]
fn parent_replacement_before_commit_is_detected_and_never_redirects() {
    let sandbox = Sandbox::new();
    let parent = sandbox.0.join("parent");
    fs::create_dir(&parent).unwrap();
    let moved = sandbox.0.join("moved");
    let output = AbsolutePath::new(parent.join("project").to_str().unwrap()).unwrap();
    let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
    let error = creator
        .commit_files(&files(b"ours"), directories(), |event| {
            if event.point == InitializationPoint::BeforePublish {
                fs::rename(&parent, &moved).unwrap();
                fs::create_dir(&parent).unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert!(!parent.join("project").exists());
    assert!(!moved.join("project").exists());
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 0);
}

#[test]
fn parent_replacement_after_commit_keeps_created_fact_and_files() {
    let sandbox = Sandbox::new();
    let parent = sandbox.0.join("parent");
    fs::create_dir(&parent).unwrap();
    let moved = sandbox.0.join("moved");
    let output = AbsolutePath::new(parent.join("project").to_str().unwrap()).unwrap();
    let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
    let observed = creator
        .commit_files(&files(b"ours"), directories(), |event| {
            if event.point == InitializationPoint::AfterPublish {
                fs::rename(&parent, &moved).unwrap();
                fs::create_dir(&parent).unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(observed.files.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(
        fs::read(moved.join("project/src/main.rs")).unwrap(),
        b"ours"
    );
    assert!(!parent.join("project").exists());
}

#[test]
fn frozen_byte_mutation_and_undeclared_file_are_rejected_before_publish() {
    for extra in [false, true] {
        let sandbox = Sandbox::new();
        let output = sandbox.output();
        let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
        let error = creator
            .commit_files(&files(b"frozen"), directories(), |event| {
                if event.point == InitializationPoint::BeforePublish {
                    fs::write(
                        event
                            .staging
                            .join(if extra { "undeclared" } else { "src/main.rs" }),
                        b"altered",
                    )
                    .unwrap();
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Conflict);
        assert!(!PathBuf::from(output.as_str()).exists());
    }
}

#[test]
fn staging_entry_replacement_is_retained_instead_of_deleting_other_content() {
    let sandbox = Sandbox::new();
    let output = sandbox.output();
    let moved = sandbox.0.join("old-stage");
    let mut replacement = None;
    let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
    let error = creator
        .commit_files(&files(b"ours"), directories(), |event| {
            if event.point == InitializationPoint::BeforePublish {
                fs::rename(event.staging, &moved).unwrap();
                fs::create_dir(event.staging).unwrap();
                fs::write(event.staging.join("foreign"), b"preserve").unwrap();
                replacement = Some(event.staging.to_path_buf());
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert_eq!(
        fs::read(replacement.unwrap().join("foreign")).unwrap(),
        b"preserve"
    );
    assert!(!PathBuf::from(output.as_str()).exists());
}

#[test]
fn faults_before_creation_clean_staging_and_after_creation_preserve_project() {
    for point in [
        InitializationPoint::StageCreated,
        InitializationPoint::FileWritten,
        InitializationPoint::BeforePublish,
        InitializationPoint::AfterPublish,
    ] {
        let sandbox = Sandbox::new();
        let output = sandbox.output();
        let mut creator = LocalProjectCreator::open(&output, CommitControl::default()).unwrap();
        let result = creator.commit_files(&files(b"ours"), directories(), |event| {
            if event.point == point {
                Err(injected())
            } else {
                Ok(())
            }
        });
        if point == InitializationPoint::AfterPublish {
            assert_eq!(result.unwrap().files.unwrap_err().kind, ErrorKind::Host);
            assert_eq!(
                fs::read(PathBuf::from(output.as_str()).join("src/main.rs")).unwrap(),
                b"ours"
            );
        } else {
            assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
            assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 0);
        }
    }
}

#[test]
fn cancellation_before_publication_is_failure_and_after_is_already_finished() {
    for point in [
        InitializationPoint::StageCreated,
        InitializationPoint::BeforePublish,
        InitializationPoint::AfterPublish,
    ] {
        let sandbox = Sandbox::new();
        let output = sandbox.output();
        let control = CommitControl::default();
        let mut creator = LocalProjectCreator::open(&output, control.clone()).unwrap();
        let result = creator.commit_files(&files(b"ours"), directories(), |event| {
            if event.point == point {
                let expected = if point == InitializationPoint::AfterPublish {
                    CancelDecision::AlreadyFinished
                } else {
                    CancelDecision::Requested
                };
                assert_eq!(control.cancel(), expected);
            }
            Ok(())
        });
        if point == InitializationPoint::AfterPublish {
            assert!(result.unwrap().files.is_ok());
            assert!(!creator.cancellation_observed());
        } else {
            assert_eq!(result.unwrap_err().kind, ErrorKind::CheckFailed);
            assert!(creator.cancellation_observed());
            assert!(!PathBuf::from(output.as_str()).exists());
        }
    }
}

#[test]
fn already_cancelled_attempt_does_not_create_staging() {
    let sandbox = Sandbox::new();
    let output = sandbox.output();
    let control = CommitControl::default();
    control.cancel();
    let mut creator = LocalProjectCreator::open(&output, control).unwrap();
    assert_eq!(
        creator
            .commit_files(&files(b"ours"), directories(), |_| panic!(
                "no side effects"
            ))
            .unwrap_err()
            .kind,
        ErrorKind::CheckFailed
    );
    assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 0);
    assert!(creator.cancellation_observed());
}

#[test]
fn unrelated_failure_is_not_reclassified_by_a_concurrent_cancel_flag() {
    let sandbox = Sandbox::new();
    let output = sandbox.output();
    let control = CommitControl::default();
    let mut creator = LocalProjectCreator::open(&output, control.clone()).unwrap();
    let result = creator.commit_files(&files(b"ours"), directories(), |event| {
        if event.point == InitializationPoint::StageCreated {
            control.cancel();
            return Err(injected());
        }
        Ok(())
    });
    assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
    assert!(!creator.cancellation_observed());
    assert!(!PathBuf::from(output.as_str()).exists());
}
