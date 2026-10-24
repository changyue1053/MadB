#![cfg(unix)]
use madb_app::{
    LogicalPath,
    input_observation::{DirectoryEntryKind, InputObservation, InputWatchSet, ObserveWorkspace},
};
use madb_host::LocalWorkspace;
use std::{
    fs, io,
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    },
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "madb-observation-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(fs::canonicalize(path).unwrap()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create test directory: {e}"),
            }
        }
    }
    fn workspace(&self) -> LocalWorkspace {
        LocalWorkspace::open(&self.0).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn watch(absent: &[&str], directories: &[&str]) -> InputWatchSet {
    InputWatchSet::new(
        absent.iter().map(|s| path(s)).collect(),
        directories.iter().map(|s| path(s)).collect(),
    )
    .unwrap()
}

#[test]
fn existence_distinguishes_missing_entries_from_terminal_symlinks() {
    let root = Directory::new();
    let source = root.workspace();
    assert!(!source.exists(&path("missing")).unwrap());
    assert!(!source.exists(&path("missing/child")).unwrap());
    fs::write(root.0.join("file"), b"data").unwrap();
    fs::create_dir(root.0.join("directory")).unwrap();
    symlink("does-not-exist", root.0.join("dangling")).unwrap();
    for name in ["file", "directory", "dangling"] {
        assert!(source.exists(&path(name)).unwrap());
        let error = InputObservation::capture(&source, &watch(&[name], &[])).unwrap_err();
        assert_eq!(error.findings[0].code, "OBS003");
    }
}

#[test]
fn symlink_or_non_directory_parent_is_not_misreported_as_absence() {
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    fs::write(root.0.join("file"), b"data").unwrap();
    symlink("dir", root.0.join("alias")).unwrap();
    symlink("missing", root.0.join("dangling")).unwrap();
    let source = root.workspace();
    for name in ["alias/absent", "dangling/absent", "file/absent"] {
        assert_eq!(
            source.exists(&path(name)).unwrap_err().findings[0].code,
            "HOST002"
        );
    }
}

#[test]
fn listing_has_direct_member_types_and_a_fresh_stream_on_each_call() {
    let root = Directory::new();
    fs::create_dir_all(root.0.join("include/nested")).unwrap();
    fs::write(root.0.join("include/header.h"), b"int value;").unwrap();
    fs::write(root.0.join("include/nested/inner.h"), b"int inner;").unwrap();
    let source = root.workspace();
    for _ in 0..3 {
        let members = source.list_directory(&path("include"), 2, 14).unwrap();
        let names: Vec<_> = members.iter().map(|e| (e.name.as_str(), &e.kind)).collect();
        assert_eq!(
            names,
            [
                ("header.h", &DirectoryEntryKind::File),
                ("nested", &DirectoryEntryKind::Directory)
            ]
        );
    }
}

#[test]
fn listing_enforces_both_budgets_and_does_not_truncate() {
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    let source = root.workspace();
    assert!(
        source
            .list_directory(&path("dir"), 0, 0)
            .unwrap()
            .is_empty()
    );
    fs::write(root.0.join("dir/a"), b"").unwrap();
    fs::write(root.0.join("dir/bb"), b"").unwrap();
    assert_eq!(source.list_directory(&path("dir"), 2, 3).unwrap().len(), 2);
    for (entries, bytes) in [(1, 3), (2, 2), (0, 0)] {
        let error = source
            .list_directory(&path("dir"), entries, bytes)
            .unwrap_err();
        assert_eq!(error.findings[0].code, "HOST003");
    }
}

#[test]
fn missing_file_and_link_are_not_empty_directories() {
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    fs::write(root.0.join("file"), b"").unwrap();
    symlink("dir", root.0.join("alias")).unwrap();
    let source = root.workspace();
    for name in ["missing", "file", "alias"] {
        assert_eq!(
            source
                .list_directory(&path(name), 10, 1024)
                .unwrap_err()
                .findings[0]
                .code,
            "HOST002"
        );
    }
}

#[test]
fn unsupported_members_are_not_silently_ignored_or_followed() {
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    let source = root.workspace();
    let member = root.0.join("dir/entry");
    symlink("missing", &member).unwrap();
    assert_eq!(
        source
            .list_directory(&path("dir"), 10, 1024)
            .unwrap_err()
            .findings[0]
            .code,
        "HOST002"
    );
    fs::remove_file(&member).unwrap();
    let socket = UnixListener::bind(&member).unwrap();
    assert!(source.exists(&path("dir/entry")).unwrap());
    assert_eq!(
        source
            .list_directory(&path("dir"), 10, 1024)
            .unwrap_err()
            .findings[0]
            .code,
        "HOST002"
    );
    drop(socket);
    fs::remove_file(&member).unwrap();
    fs::write(root.0.join("original"), b"").unwrap();
    fs::hard_link(root.0.join("original"), &member).unwrap();
    assert_eq!(
        source
            .list_directory(&path("dir"), 10, 1024)
            .unwrap_err()
            .findings[0]
            .code,
        "HOST002"
    );
}

#[test]
fn unsupported_member_names_fail_without_lossy_conversion() {
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    let source = root.workspace();
    for name in ["中文", "bad:name"] {
        let file = root.0.join("dir").join(name);
        fs::write(&file, b"").unwrap();
        assert_eq!(
            source
                .list_directory(&path("dir"), 10, 1024)
                .unwrap_err()
                .findings[0]
                .code,
            "HOST002"
        );
        fs::remove_file(file).unwrap();
    }
}

#[test]
fn observations_detect_absent_appearance_and_directory_changes() {
    let root = Directory::new();
    fs::create_dir(root.0.join("include")).unwrap();
    let source = root.workspace();
    let observed =
        InputObservation::capture(&source, &watch(&["include/optional.h"], &["include"])).unwrap();
    observed.verify(&source).unwrap();
    fs::write(root.0.join("include/optional.h"), b"").unwrap();
    assert_eq!(
        observed.verify(&source).unwrap_err().findings[0].code,
        "OBS003"
    );
    fs::remove_file(root.0.join("include/optional.h")).unwrap();
    observed.verify(&source).unwrap(); // Checkpoint matching is deliberately not ABA detection.
    fs::create_dir(root.0.join("include/new")).unwrap();
    assert_eq!(
        observed.verify(&source).unwrap_err().findings[0].code,
        "OBS003"
    );
}

#[test]
fn permissions_are_errors_not_missing_when_access_is_denied() {
    let root = Directory::new();
    fs::create_dir(root.0.join("locked")).unwrap();
    let source = root.workspace();
    fs::set_permissions(root.0.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    // Root legitimately bypasses mode bits. Ordinary host/CI users must get
    // access errors, and neither port may translate those into missing/empty.
    let result = source.exists(&path("locked/optional"));
    let listing = source.list_directory(&path("locked"), 1, 10);
    fs::set_permissions(root.0.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
    if rustix::process::geteuid().is_root() {
        assert!(!result.unwrap());
        assert!(listing.unwrap().is_empty());
    } else {
        assert_eq!(result.unwrap_err().findings[0].code, "HOST002");
        assert_eq!(listing.unwrap_err().findings[0].code, "HOST002");
    }
}

// Linux permits raw non-UTF-8 names. The macOS fixture filesystem rejects
// creation with EILSEQ, so its unsupported-name cases above use valid UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_member_is_rejected_without_lossy_conversion() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let root = Directory::new();
    fs::create_dir(root.0.join("dir")).unwrap();
    fs::write(root.0.join("dir").join(OsString::from_vec(vec![0xff])), b"").unwrap();
    let error = root
        .workspace()
        .list_directory(&path("dir"), 10, 1024)
        .unwrap_err();
    assert_eq!(error.findings[0].code, "HOST002");
}
