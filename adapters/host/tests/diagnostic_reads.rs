#![cfg(unix)]
use madb_app::{ErrorKind, LogicalPath, ports::ReadWorkspace};
use madb_host::{DiagnosticWorkspace, LocalWorkspace};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "madb-explain-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn read(&self, name: &str, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        DiagnosticWorkspace::open(&self.0)?.read(&LogicalPath::new(name).unwrap(), limit)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn bounded_reads_and_missing_paths_remain_explicit() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("sources")).unwrap();
    fs::write(root.0.join("sources/Actual.c"), b"abc").unwrap();
    assert_eq!(
        root.read("sources/Actual.c", 3).unwrap(),
        Some(b"abc".to_vec())
    );
    assert_eq!(root.read("missing.c", 3).unwrap(), None);
    assert_eq!(root.read("missing/leaf.c", 3).unwrap(), None);
    assert_eq!(
        root.read("sources/Actual.c", 2).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    assert_eq!(
        root.read("sources", 100).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
}

#[test]
fn links_and_spelling_aliases_never_authorize_source_identity() {
    let root = Fixture::new();
    fs::write(root.0.join("Actual.c"), b"abc").unwrap();
    symlink("Actual.c", root.0.join("link.c")).unwrap();
    assert_eq!(
        root.read("link.c", 3).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    // On a case-sensitive host this spelling is absent; on a case-insensitive
    // host the open succeeds but the exact directory spelling must reject it.
    match root.read("actual.c", 3) {
        Ok(None) => assert!(!root.0.join("actual.c").exists()),
        Err(error) => assert_eq!(error.kind, ErrorKind::InvalidInput),
        Ok(Some(_)) => panic!("case alias admitted"),
    }
    fs::hard_link(root.0.join("Actual.c"), root.0.join("hard.c")).unwrap();
    assert_eq!(
        root.read("hard.c", 3).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
}

#[test]
fn actual_io_failure_has_host_class_without_changing_legacy_queries() {
    let root = Fixture::new();
    let missing = root.0.join("absent-root");
    assert!(matches!(DiagnosticWorkspace::open(&missing), Err(e) if e.kind == ErrorKind::Host));
    assert!(matches!(LocalWorkspace::open(&missing), Err(e) if e.kind == ErrorKind::InvalidInput));
    let source = root.0.join("private.c");
    fs::write(&source, b"abc").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o0)).unwrap();
    let strict = root.read("private.c", 3);
    let legacy = LocalWorkspace::open(&root.0)
        .unwrap()
        .read(&LogicalPath::new("private.c").unwrap(), 3);
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    // A privileged test process can bypass mode bits; missing-root above still
    // proves distinct I/O classification on that host.
    if let Err(error) = strict {
        assert_eq!(error.kind, ErrorKind::Host);
        assert_eq!(legacy.unwrap_err().kind, ErrorKind::InvalidInput);
    }
}
