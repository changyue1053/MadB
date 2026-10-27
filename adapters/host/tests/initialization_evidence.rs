#![cfg(target_os = "macos")]
use madb_app::use_cases::initialize::{AbsolutePath, FileIdentity, InitEvidencePort};
use madb_app::{Digest, ErrorKind, LogicalPath, ports::FileReference};
use madb_host::initialization::LocalInitEvidence;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/madb-init-evidence-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn root(&self) -> AbsolutePath {
        AbsolutePath::new(self.0.join("证据 空格").to_str().unwrap()).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn reference(path: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: LogicalPath::new(path).unwrap(),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn identity(reference: &FileReference) -> FileIdentity {
    FileIdentity {
        path: reference.path.clone(),
        sha256: reference.digest.clone(),
        size: reference.size,
    }
}

#[test]
fn saved_evidence_observes_actual_bytes_and_result_in_separate_root() {
    let sandbox = Sandbox::new();
    let root = sandbox.root();
    let mut evidence = LocalInitEvidence::create(&root).unwrap();
    let original = reference("input/sub/file.bin", b"original\0bytes");
    evidence.store_file(&original, b"original\0bytes").unwrap();
    let observation = evidence
        .save_result(&root, b"{\"result\":true}\n", &[identity(&original)])
        .unwrap();
    assert_eq!(observation.root, root);
    assert_eq!(observation.files.len(), 2);
    assert_eq!(
        fs::read(PathBuf::from(root.as_str()).join("init-result.json")).unwrap(),
        b"{\"result\":true}\n"
    );
    assert_eq!(
        observation
            .files
            .iter()
            .find(|file| file.path.as_str() == "input/sub/file.bin")
            .unwrap(),
        &identity(&original)
    );
    assert_eq!(
        evidence.save_result(&root, b"other", &[]).unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        evidence
            .store_file(&reference("later", b"x"), b"x")
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
}

#[test]
fn repeated_roots_files_aliases_and_bad_bytes_are_rejected() {
    let sandbox = Sandbox::new();
    let root = sandbox.root();
    let mut evidence = LocalInitEvidence::create(&root).unwrap();
    assert!(LocalInitEvidence::create(&root).is_err());
    let r = reference("Data/file.bin", b"value");
    assert_eq!(
        evidence.store_file(&r, b"wrong").unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    assert!(!PathBuf::from(root.as_str()).join("Data").exists());
    evidence.store_file(&r, b"value").unwrap();
    for path in [
        "Data/file.bin",
        "Data/FILE.bin",
        "data/other",
        "Data",
        "Data/file.bin/child",
        "init-result.json",
        "INIT-RESULT.JSON",
    ] {
        assert_eq!(
            evidence
                .store_file(&reference(path, b"x"), b"x")
                .unwrap_err()
                .kind,
            ErrorKind::Conflict,
            "{path}"
        );
    }
    assert_eq!(
        fs::read(PathBuf::from(root.as_str()).join("Data/file.bin")).unwrap(),
        b"value"
    );
}

#[test]
fn required_evidence_must_be_unique_saved_and_match_frozen_bytes() {
    for mode in 0..6 {
        let sandbox = Sandbox::new();
        let root = sandbox.root();
        let mut evidence = LocalInitEvidence::create(&root).unwrap();
        let r = reference("file", b"frozen");
        evidence.store_file(&r, b"frozen").unwrap();
        let mut required = vec![identity(&r)];
        match mode {
            0 => required.push(identity(&r)),
            1 => required[0].sha256 = Digest::of_bytes(b"changed"),
            2 => required[0].path = LogicalPath::new("missing").unwrap(),
            3 => fs::write(PathBuf::from(root.as_str()).join("file"), b"mutate").unwrap(),
            4 => fs::write(PathBuf::from(root.as_str()).join("extra"), b"not recorded").unwrap(),
            _ => evidence
                .store_file(&reference("stored-but-unreferenced", b"extra"), b"extra")
                .unwrap(),
        }
        assert!(evidence.save_result(&root, b"result", &required).is_err());
        assert!(
            !PathBuf::from(root.as_str())
                .join("init-result.json")
                .exists()
        );
    }
}

#[test]
fn evidence_link_and_root_replacement_never_follow_external_content() {
    for mode in 0..3 {
        let sandbox = Sandbox::new();
        let root = sandbox.root();
        let mut evidence = LocalInitEvidence::create(&root).unwrap();
        let r = reference("file", b"frozen");
        evidence.store_file(&r, b"frozen").unwrap();
        let outside = sandbox.0.join("outside");
        fs::write(&outside, b"frozen").unwrap();
        let root_path = PathBuf::from(root.as_str());
        if mode < 2 {
            fs::remove_file(root_path.join("file")).unwrap();
            if mode == 0 {
                std::os::unix::fs::symlink(&outside, root_path.join("file")).unwrap();
            } else {
                fs::hard_link(&outside, root_path.join("file")).unwrap();
            }
        } else {
            fs::rename(&root_path, sandbox.0.join("retained")).unwrap();
            fs::create_dir(&root_path).unwrap();
        }
        assert!(
            evidence
                .save_result(&root, b"result", &[identity(&r)])
                .is_err()
        );
        assert_eq!(fs::read(&outside).unwrap(), b"frozen");
        assert!(!root_path.join("init-result.json").exists());
    }
}

#[test]
fn competing_result_is_preserved_and_allocation_uses_distinct_roots() {
    let sandbox = Sandbox::new();
    let parent = AbsolutePath::new(sandbox.0.to_str().unwrap()).unwrap();
    let mut first = LocalInitEvidence::allocate(&parent).unwrap();
    let second = LocalInitEvidence::allocate(&parent).unwrap();
    assert_ne!(first.root(), second.root());
    let root = first.root().clone();
    fs::write(
        PathBuf::from(root.as_str()).join("init-result.json"),
        b"competitor",
    )
    .unwrap();
    assert_eq!(
        first.save_result(&root, b"ours", &[]).unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        fs::read(PathBuf::from(root.as_str()).join("init-result.json")).unwrap(),
        b"competitor"
    );
}

#[test]
fn allocation_does_not_apply_initial_tree_budget_to_existing_host_siblings() {
    let sandbox = Sandbox::new();
    for index in 0..16_385 {
        fs::File::create(sandbox.0.join(format!("unrelated-{index:05}"))).unwrap();
    }
    let parent = AbsolutePath::new(sandbox.0.to_str().unwrap()).unwrap();
    let mut evidence = LocalInitEvidence::allocate(&parent).unwrap();
    let root = evidence.root().clone();
    evidence.save_result(&root, b"result", &[]).unwrap();
    assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 16_386);
    assert_eq!(
        fs::read(PathBuf::from(root.as_str()).join("init-result.json")).unwrap(),
        b"result"
    );
}

#[test]
fn allocation_preserves_noncollision_parent_error() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.0.join("CaseParent")).unwrap();
    let alias = sandbox.0.join("caseparent");
    let aliases_on_this_volume = fs::metadata(&alias).is_ok();
    let parent = AbsolutePath::new(alias.to_str().unwrap()).unwrap();
    let error = match LocalInitEvidence::allocate(&parent) {
        Ok(_) => panic!("incorrect parent spelling must not be accepted"),
        Err(error) => error,
    };
    if aliases_on_this_volume {
        assert_eq!(error.kind, ErrorKind::Conflict);
        assert!(error.to_string().contains("path spelling differs"));
    } else {
        assert_eq!(error.kind, ErrorKind::Host);
    }
    assert!(!error.to_string().contains("collision budget"));
    assert_eq!(
        fs::read_dir(sandbox.0.join("CaseParent")).unwrap().count(),
        0
    );
}
