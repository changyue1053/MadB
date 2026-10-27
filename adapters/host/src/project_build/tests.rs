use super::*;
use madb_app::{Id, process::CancelDecision, transaction::CommitControl};
use std::{
    os::unix::fs::{FileExt, PermissionsExt, symlink},
    path::PathBuf,
};

struct Fixture {
    path: PathBuf,
    descriptor: Descriptor,
    original: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let path = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("madb-project-host-{}", fs::nonce()));
        std::fs::create_dir(&path).unwrap();
        let original = b"descriptor bytes decoded by the separate format port\n".to_vec();
        let descriptor = Descriptor {
            template: TemplateIdentity {
                id: Id::new("org.madb.template/rust-c-sdk").unwrap(),
                version: "0.2.0".into(),
                archive: identity(".madb/template-package.tar", b"original archive"),
            },
            namespace: "demo".into(),
            profile: FileReference {
                file: identity(".madb/template-input/profile.json", b"profile"),
                schema: Id::new("madb.profile/1").unwrap(),
            },
            sdk: FileReference {
                file: identity(".madb/template-input/toolchain.json", b"sdk"),
                schema: Id::new("madb.toolchain/1").unwrap(),
            },
            tool: ToolIdentity {
                id: Id::new("org.madb/cli").unwrap(),
                version: "0.1.0".into(),
                sha256: Digest::of_bytes(b"tool bytes"),
                size: 10,
                source_revision: "a".repeat(40),
            },
        };
        let result = Self {
            path,
            descriptor,
            original,
        };
        for (p, b) in [
            (DESCRIPTOR, result.original.as_slice()),
            (".madb/template-package.tar", b"original archive"),
            (".madb/template-input/profile.json", b"profile"),
            (".madb/template-input/toolchain.json", b"sdk"),
            ("rust/src/lib.rs", b"pub fn value() -> u32 { 7 }"),
            ("consumer/main.c", b"int main(void) { return 0; }"),
        ] {
            result.write(p, b);
        }
        result.write(
            ORIGIN,
            &origin_bytes(&result.descriptor, &result.original).unwrap(),
        );
        result
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let full = self.path.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }
    fn open(&self) -> LocalProjectBuild {
        LocalProjectBuild::open(&AbsolutePath::new(self.path.to_str().unwrap()).unwrap()).unwrap()
    }
    fn prepared(&self) -> (LocalProjectBuild, BuildAttempt) {
        let mut ws = self.open();
        let captured = ws.capture(&self.descriptor, &self.original).unwrap();
        let attempt = ws.begin_attempt(&captured).unwrap();
        (ws, attempt)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn identity(path: &str, bytes: &[u8]) -> FileIdentity {
    FileIdentity {
        path: logical(path).unwrap(),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn outputs(attempt: &BuildAttempt, tag: &[u8]) -> ProjectOutput {
    let root = PathBuf::from(attempt.root.as_str());
    let candidate = "native/candidate";
    std::fs::create_dir_all(root.join(candidate)).unwrap();
    let mut files = Vec::new();
    for (p, b) in [
        ("native/candidate/candidate.json", b"manifest".as_slice()),
        ("native/candidate/values.h", b"header"),
        ("native/candidate/libvalues.a", tag),
        ("native/consumer", b"consumer"),
    ] {
        std::fs::write(root.join(p), b).unwrap();
        files.push(identity(p, b));
    }
    std::fs::set_permissions(
        root.join("native/consumer"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    ProjectOutput {
        identity: Digest::of_bytes(tag),
        projection: OutputProjection {
            snapshot_digest: Digest::of_bytes(b"snapshot"),
            candidate: CandidateOutput {
                root: logical(candidate).unwrap(),
                manifest: identity("native/candidate/candidate.json", b"manifest"),
            },
            consumer: identity("native/consumer", b"consumer"),
            outputs: files,
        },
    }
}

#[test]
fn capture_copies_actual_bytes_and_distinguishes_missing_and_empty_headers() {
    let f = Fixture::new();
    let (ws, a) = f.prepared();
    assert!(
        !a.inputs
            .directories
            .iter()
            .find(|d| d.path.as_str() == "consumer/include")
            .unwrap()
            .present
    );
    assert_eq!(
        std::fs::read(PathBuf::from(a.root.as_str()).join("inputs/rust/src/lib.rs")).unwrap(),
        b"pub fn value() -> u32 { 7 }"
    );
    assert!(PathBuf::from(a.root.as_str()).join("native").is_dir());
    drop(ws);
    std::fs::create_dir(f.path.join("consumer/include")).unwrap();
    let (_, b) = f.prepared();
    assert!(
        b.inputs
            .directories
            .iter()
            .find(|d| d.path.as_str() == "consumer/include")
            .unwrap()
            .present
    );
}
#[test]
fn source_links_replacements_and_added_members_fail_closed() {
    let f = Fixture::new();
    f.write("consumer/include/a.h", b"header");
    let (ws, _) = f.prepared();
    f.write("consumer/include/b.h", b"added");
    assert!(ws.revalidate().is_err());
    let g = Fixture::new();
    std::fs::remove_file(g.path.join("rust/src/lib.rs")).unwrap();
    symlink("../../consumer/main.c", g.path.join("rust/src/lib.rs")).unwrap();
    assert!(g.open().capture(&g.descriptor, &g.original).is_err());
    let h = Fixture::new();
    let (ws, _) = h.prepared();
    let source = h.path.join("rust/src/lib.rs");
    let original = std::fs::read(&source).unwrap();
    std::fs::remove_file(&source).unwrap();
    std::fs::write(&source, original).unwrap();
    assert!(ws.revalidate().is_err());
}
#[test]
fn captured_source_growth_is_conflict_and_preserves_current() {
    let f = Fixture::new();
    let (mut first, a) = f.prepared();
    let original = outputs(&a, b"original");
    assert_eq!(
        first
            .publish(&a, &original, &CommitControl::default())
            .published,
        Some(true)
    );
    let current_path = f.path.join(".madb/project-state/current.json");
    let current = std::fs::read(&current_path).unwrap();
    let (mut next, b) = f.prepared();
    let candidate = outputs(&b, b"replacement");
    f.write(
        "rust/src/lib.rs",
        b"pub fn value() -> u32 { 7 } // appended after capture",
    );
    let result = next.publish(&b, &candidate, &CommitControl::default());
    assert_eq!(result.published, Some(false));
    assert_eq!(result.error.unwrap().kind, ErrorKind::Conflict);
    assert_eq!(std::fs::read(&current_path).unwrap(), current);
    assert!(PathBuf::from(a.root.as_str()).is_dir());
    assert!(PathBuf::from(b.root.as_str()).is_dir());
}
#[test]
fn captured_source_size_changes_are_conflicts_but_initial_budget_is_invalid() {
    for replacement in [
        b"pub fn value() -> u32 { 7 } // larger".as_slice(),
        b"shorter",
        b"pub fn value() -> u32 { 8 }",
    ] {
        let f = Fixture::new();
        let (ws, _) = f.prepared();
        f.write("rust/src/lib.rs", replacement);
        assert_eq!(ws.revalidate().unwrap_err().kind, ErrorKind::Conflict);
    }
    let f = Fixture::new();
    let ws = f.open();
    assert_eq!(
        fs::read(&ws.directory, "rust/src/lib.rs", 1)
            .unwrap_err()
            .kind,
        ErrorKind::InvalidInput
    );
    let mut ws = ws;
    ws.capture(&f.descriptor, &f.original).unwrap();
    ws.revalidate().unwrap();
}
#[test]
fn captured_source_missing_replaced_and_changed_types_are_conflicts() {
    for change in 0..7 {
        let f = Fixture::new();
        let (ws, _) = f.prepared();
        let source = f.path.join("rust/src/lib.rs");
        let original = std::fs::read(&source).unwrap();
        match change {
            0 => std::fs::remove_file(&source).unwrap(),
            1 => {
                let replacement = source.with_extension("replacement");
                std::fs::write(&replacement, original).unwrap();
                std::fs::rename(&replacement, &source).unwrap();
            }
            2 => {
                std::fs::remove_file(&source).unwrap();
                std::fs::create_dir(&source).unwrap();
            }
            3 => {
                std::fs::remove_file(&source).unwrap();
                symlink("../../consumer/main.c", &source).unwrap();
            }
            4 => std::fs::hard_link(&source, source.with_extension("alias")).unwrap(),
            5 => {
                std::fs::remove_file(&source).unwrap();
                // sockaddr_un is shorter than the fixture's canonical path.
                let socket_path = std::env::temp_dir().join(format!("s-{}", fs::nonce()));
                let _socket = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
                std::fs::rename(socket_path, &source).unwrap();
            }
            6 => {
                std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
                f.write("rust/src", b"parent is now a file");
            }
            _ => unreachable!(),
        }
        assert_eq!(
            ws.revalidate().unwrap_err().kind,
            ErrorKind::Conflict,
            "change {change}"
        );
    }
}
#[test]
fn captured_source_changes_during_actual_reads_are_conflicts() {
    for change in 0..6 {
        let f = Fixture::new();
        let ws = f.open();
        let name = "rust/src/lib.rs";
        let source = f.path.join(name);
        let (bytes, identity) = fs::read(&ws.directory, name, 1024).unwrap();
        let result = fs::verify_captured_with_reader(
            &ws.directory,
            name,
            &bytes,
            &identity,
            |file, target| {
                let half = target.len() / 2;
                file.read_exact_at(&mut target[..half], 0)?;
                match change {
                    0 => f.write(name, b"pub fn value() -> u32 { 7 } // larger"),
                    1 => f.write(name, b"shorter"),
                    2 => f.write(name, b"pub fn value() -> u32 { 8 }"),
                    3 => std::fs::remove_file(&source).unwrap(),
                    4 => {
                        let replacement = source.with_extension("replacement");
                        std::fs::write(&replacement, &bytes).unwrap();
                        std::fs::rename(&replacement, &source).unwrap();
                    }
                    5 => {
                        std::fs::rename(&source, source.with_extension("old")).unwrap();
                        symlink("../../consumer/main.c", &source).unwrap();
                    }
                    _ => unreachable!(),
                }
                file.read_exact_at(&mut target[half..], half as u64)
            },
        );
        assert_eq!(
            result.unwrap_err().kind,
            ErrorKind::Conflict,
            "change {change}"
        );
    }
}
#[test]
fn captured_source_io_failures_keep_host_classification() {
    let f = Fixture::new();
    let ws = f.open();
    let name = "rust/src/lib.rs";
    for changed in [false, true] {
        let (bytes, identity) = fs::read(&ws.directory, name, 1024).unwrap();
        let write_only = std::fs::OpenOptions::new()
            .write(true)
            .open(f.path.join(name))
            .unwrap();
        let result =
            fs::verify_captured_with_reader(&ws.directory, name, &bytes, &identity, |_, target| {
                if changed {
                    f.write(name, b"a concurrent edit must not hide real IO failure");
                }
                let error = write_only.read_exact_at(target, 0).unwrap_err();
                assert_eq!(
                    error.raw_os_error(),
                    Some(rustix::io::Errno::BADF.raw_os_error())
                );
                Err(error)
            });
        assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
    }
    let (bytes, identity) = fs::read(&ws.directory, name, 1024).unwrap();
    let eof_without_drift =
        fs::verify_captured_with_reader(&ws.directory, name, &bytes, &identity, |file, target| {
            file.read_exact_at(target, bytes.len() as u64)
        });
    assert_eq!(eof_without_drift.unwrap_err().kind, ErrorKind::Host);
    std::fs::set_permissions(f.path.join(name), std::fs::Permissions::from_mode(0o0)).unwrap();
    // Privileged Unix users may still read mode 000: that is an observed
    // identity change, while an actual permission denial must remain Host.
    let expected_kind = match File::open(f.path.join(name)) {
        Ok(_) => {
            eprintln!("mode-000 observation: readable by current user; expected Conflict");
            ErrorKind::Conflict
        }
        Err(error) => {
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            eprintln!(
                "mode-000 observation: {:?}, errno {:?}; expected Host",
                error.kind(),
                error.raw_os_error()
            );
            ErrorKind::Host
        }
    };
    let result = fs::verify_captured(&ws.directory, name, &bytes, &identity);
    std::fs::set_permissions(f.path.join(name), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(result.unwrap_err().kind, expected_kind);
}
#[test]
fn unknown_origin_and_state_are_preserved_and_rejected() {
    let f = Fixture::new();
    f.write(ORIGIN, b"foreign");
    assert!(f.open().capture(&f.descriptor, &f.original).is_err());
    assert_eq!(std::fs::read(f.path.join(ORIGIN)).unwrap(), b"foreign");
    let g = Fixture::new();
    g.write(".madb/project-state/handwritten.txt", b"retain");
    assert!(g.open().capture(&g.descriptor, &g.original).is_err());
    assert_eq!(
        std::fs::read(g.path.join(".madb/project-state/handwritten.txt")).unwrap(),
        b"retain"
    );
}
#[test]
fn output_inventory_must_be_complete_and_owned() {
    let f = Fixture::new();
    let (ws, a) = f.prepared();
    let output = outputs(&a, b"one");
    ws.verify_outputs(&a, &output).unwrap();
    let mut forged = a.clone();
    forged.root = AbsolutePath::new("/private/tmp/other").unwrap();
    assert!(ws.verify_outputs(&forged, &output).is_err());
    std::fs::write(
        PathBuf::from(a.root.as_str()).join("native/candidate/extra"),
        b"undeclared",
    )
    .unwrap();
    assert!(ws.verify_outputs(&a, &output).is_err());
}
#[test]
fn publication_preserves_old_attempt_and_rejects_stale_cas() {
    let f = Fixture::new();
    let (mut first, a) = f.prepared();
    let (mut stale, b) = f.prepared();
    let one = outputs(&a, b"one");
    let two = outputs(&b, b"two");
    let control = CommitControl::default();
    let published = first.publish(&a, &one, &control);
    assert!(published.error.is_none(), "{:?}", published.error);
    assert_eq!(published.published, Some(true));
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    let current = std::fs::read(f.path.join(".madb/project-state/current.json")).unwrap();
    let rejected = stale.publish(&b, &two, &CommitControl::default());
    assert_eq!(rejected.published, Some(false));
    assert!(rejected.error.is_some());
    assert_eq!(
        std::fs::read(f.path.join(".madb/project-state/current.json")).unwrap(),
        current
    );
    let (mut next, c) = f.prepared();
    assert_eq!(c.previous.as_ref().unwrap().artifact_root, a.root);
    let three = outputs(&c, b"three");
    assert_eq!(
        next.publish(&c, &three, &CommitControl::default())
            .published,
        Some(true)
    );
    assert_eq!(
        std::fs::read(PathBuf::from(a.root.as_str()).join("native/candidate/libvalues.a")).unwrap(),
        b"one"
    );
}
#[test]
fn cancellation_and_durability_tail_preserve_publication_fact() {
    let f = Fixture::new();
    let (mut ws, a) = f.prepared();
    let output = outputs(&a, b"one");
    ws.fault = Some(publish::Fault::CancelBeforePublish);
    let control = CommitControl::default();
    let rejected = ws.publish(&a, &output, &control);
    assert_eq!(rejected.published, Some(false));
    assert!(control.rejected_by_cancellation());
    assert!(!f.path.join(".madb/project-state/current.json").exists());
    let (mut ws, a) = f.prepared();
    let output = outputs(&a, b"two");
    ws.fault = Some(publish::Fault::AfterPublish);
    let control = CommitControl::default();
    let tail = ws.publish(&a, &output, &control);
    assert_eq!(tail.published, Some(true));
    assert_eq!(tail.durability, Durability::Uncertain);
    assert!(tail.error.is_some());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert!(f.path.join(".madb/project-state/current.json").is_file());
}
#[test]
fn corrupt_previous_output_blocks_new_build_without_cleaning() {
    let f = Fixture::new();
    let (mut ws, a) = f.prepared();
    let output = outputs(&a, b"one");
    assert_eq!(
        ws.publish(&a, &output, &CommitControl::default()).published,
        Some(true)
    );
    let old = PathBuf::from(a.root.as_str()).join("native/candidate/libvalues.a");
    std::fs::write(&old, b"corrupt").unwrap();
    assert!(f.open().capture(&f.descriptor, &f.original).is_err());
    assert_eq!(std::fs::read(old).unwrap(), b"corrupt");
}
#[test]
fn install_has_one_acceptance_point_and_never_overwrites_existing_bytes() {
    let f = Fixture::new();
    let mut ws = f.open();
    ws.check_origin(&f.descriptor, &f.original).unwrap();
    let control = CommitControl::default();
    ws.fault = Some(publish::Fault::InstallTail);
    let installed = ws.install_bytes(&f.descriptor, b"tool bytes", &control);
    assert!(installed.installed.is_some());
    assert!(installed.error.is_some());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    ws.fault = None;
    let repeated = ws.install_bytes(&f.descriptor, b"tool bytes", &CommitControl::default());
    assert!(repeated.error.is_none());
    assert!(repeated.installed.is_some());
    std::fs::set_permissions(
        f.path.join(".madb/wrapper/madb"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    f.write(".madb/wrapper/madb", b"different");
    let failed = ws.install_bytes(&f.descriptor, b"tool bytes", &CommitControl::default());
    assert!(failed.installed.is_none());
    assert!(failed.error.is_some());
    assert_eq!(
        std::fs::read(f.path.join(".madb/wrapper/madb")).unwrap(),
        b"different"
    );
}
#[test]
fn cancelled_install_does_not_create_mirror() {
    let f = Fixture::new();
    let mut ws = f.open();
    ws.check_origin(&f.descriptor, &f.original).unwrap();
    let control = CommitControl::default();
    control.cancel();
    let result = ws.install_bytes(&f.descriptor, b"tool bytes", &control);
    assert!(result.installed.is_none());
    assert!(control.rejected_by_cancellation());
    assert!(!f.path.join(".madb/wrapper/madb").exists());
}

#[test]
fn competing_publishers_have_one_winner_and_keep_both_attempts() {
    let f = Fixture::new();
    let (mut one, a) = f.prepared();
    let (mut two, b) = f.prepared();
    let oa = outputs(&a, b"one");
    let ob = outputs(&b, b"two");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = barrier.clone();
    let roots = [a.root.clone(), b.root.clone()];
    let left = std::thread::spawn(move || {
        barrier.wait();
        one.publish(&a, &oa, &CommitControl::default()).published
    });
    let right = std::thread::spawn(move || {
        other.wait();
        two.publish(&b, &ob, &CommitControl::default()).published
    });
    let values = [left.join().unwrap(), right.join().unwrap()];
    assert_eq!(values.iter().filter(|v| **v == Some(true)).count(), 1);
    assert_eq!(values.iter().filter(|v| **v == Some(false)).count(), 1);
    for root in roots {
        assert!(
            PathBuf::from(root.as_str())
                .join("project-output.json")
                .is_file()
        );
    }
}
#[test]
fn unknown_replacement_is_not_reported_as_absent_or_success() {
    let f = Fixture::new();
    let (mut ws, a) = f.prepared();
    let output = outputs(&a, b"one");
    ws.fault = Some(publish::Fault::UnknownAfterPublish);
    let control = CommitControl::default();
    let result = ws.publish(&a, &output, &control);
    assert_eq!(result.published, None);
    assert_eq!(result.durability, Durability::Uncertain);
    assert!(result.error.is_some());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert!(f.path.join(".madb/project-state/current.json").is_file());
    assert!(
        PathBuf::from(a.root.as_str())
            .join("project-output.json")
            .is_file()
    );
}
#[test]
fn tool_rename_error_retains_only_verified_same_installed_inode() {
    let f = Fixture::new();
    let mut ws = f.open();
    ws.check_origin(&f.descriptor, &f.original).unwrap();
    ws.fault = Some(publish::Fault::InstallRenameError);
    let result = ws.install_bytes(&f.descriptor, b"tool bytes", &CommitControl::default());
    assert!(result.installed.is_some());
    assert!(result.error.is_some());
    assert_eq!(
        std::fs::read(f.path.join(".madb/wrapper/madb")).unwrap(),
        b"tool bytes"
    );
}
#[test]
fn malformed_attempt_marker_and_same_bytes_index_replacement_are_rejected() {
    let f = Fixture::new();
    let (mut ws, a) = f.prepared();
    let output = outputs(&a, b"one");
    assert_eq!(
        ws.publish(&a, &output, &CommitControl::default()).published,
        Some(true)
    );
    let (mut next, b) = f.prepared();
    let second = outputs(&b, b"two");
    let current = f.path.join(".madb/project-state/current.json");
    let bytes = std::fs::read(&current).unwrap();
    let replacement = current.with_extension("replacement");
    std::fs::write(&replacement, &bytes).unwrap();
    std::fs::rename(replacement, &current).unwrap();
    assert_eq!(
        next.publish(&b, &second, &CommitControl::default())
            .published,
        Some(false)
    );
    let marker = PathBuf::from(a.root.as_str()).join(".attempt.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    value["files"] = serde_json::json!([]);
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&marker, records::encode(&value).unwrap()).unwrap();
    assert!(f.open().capture(&f.descriptor, &f.original).is_err());
    assert!(marker.is_file());
}

#[test]
fn created_attempt_is_reportable_when_copy_preparation_fails() {
    let f = Fixture::new();
    let mut workspace = f.open();
    let captured = workspace.capture(&f.descriptor, &f.original).unwrap();
    workspace.fault = Some(publish::Fault::AttemptAfterCreate);
    assert!(workspace.begin_attempt(&captured).is_err());
    let actual = workspace.created_attempt_root().unwrap();
    assert!(PathBuf::from(actual.as_str()).is_dir());
    assert!(!PathBuf::from(actual.as_str()).join("inputs").exists());
    assert!(workspace.attempt.is_none());
    assert!(workspace.begin_attempt(&captured).is_err());
}

#[test]
fn simultaneous_first_installers_share_only_the_exact_final_mirror() {
    let f = Fixture::new();
    let mut one = f.open();
    let mut two = f.open();
    one.check_origin(&f.descriptor, &f.original).unwrap();
    two.check_origin(&f.descriptor, &f.original).unwrap();
    let a = f.descriptor.clone();
    let b = a.clone();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = barrier.clone();
    let left = std::thread::spawn(move || {
        barrier.wait();
        one.install_bytes(&a, b"tool bytes", &CommitControl::default())
    });
    let right = std::thread::spawn(move || {
        other.wait();
        two.install_bytes(&b, b"tool bytes", &CommitControl::default())
    });
    for result in [left.join().unwrap(), right.join().unwrap()] {
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.installed.is_some());
    }
    assert_eq!(
        std::fs::read(f.path.join(".madb/wrapper/madb")).unwrap(),
        b"tool bytes"
    );
}

#[test]
fn post_acceptance_read_failure_retains_installed_bytes_and_sealed_cancel() {
    let f = Fixture::new();
    let mut workspace = f.open();
    workspace.check_origin(&f.descriptor, &f.original).unwrap();
    workspace.fault = Some(publish::Fault::InstallAfterAcceptRead);
    for _ in 0..2 {
        let control = CommitControl::default();
        let result = workspace.install_bytes(&f.descriptor, b"tool bytes", &control);
        assert!(result.installed.is_some());
        assert!(result.error.is_some());
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
        assert_eq!(
            std::fs::read(f.path.join(".madb/wrapper/madb")).unwrap(),
            b"tool bytes"
        );
    }
}
