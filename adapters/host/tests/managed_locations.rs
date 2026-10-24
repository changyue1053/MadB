#![cfg(unix)]
use madb_app::LogicalPath;
use madb_host::locations::{directory, temporary_parent, unique_run_name};
use std::path::PathBuf;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = temporary_parent().unwrap().join(unique_run_name().as_str());
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}

#[test]
fn readonly_missing_lookup_has_no_write_side_effect() {
    let root = Root::new();
    assert!(
        directory(&root.0, &path(".madb/runs"), false)
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
}
#[test]
fn create_then_read_returns_same_confined_directory() {
    let root = Root::new();
    let first = directory(&root.0, &path(".madb/runs"), true)
        .unwrap()
        .unwrap();
    assert!(first.is_dir());
    assert_eq!(
        directory(&root.0, &path(".madb/runs"), false).unwrap(),
        Some(first)
    );
}
#[test]
fn existing_symlink_or_file_never_becomes_managed_directory() {
    let root = Root::new();
    let outside = Root::new();
    std::os::unix::fs::symlink(&outside.0, root.0.join("link")).unwrap();
    std::fs::write(root.0.join("file"), b"owned by user").unwrap();
    for create in [true, false] {
        assert!(directory(&root.0, &path("link/child"), create).is_err());
        assert!(directory(&root.0, &path("file/child"), create).is_err());
    }
    assert_eq!(std::fs::read_dir(&outside.0).unwrap().count(), 0);
    assert_eq!(
        std::fs::read(root.0.join("file")).unwrap(),
        b"owned by user"
    );
}
#[test]
fn allocation_names_are_single_components_and_distinct() {
    let first = unique_run_name();
    let second = unique_run_name();
    assert_ne!(first, second);
    assert!(!first.as_str().contains('/'));
}
