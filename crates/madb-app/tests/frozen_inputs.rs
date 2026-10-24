use std::cell::RefCell;
use std::collections::BTreeMap;

use madb_app::inputs::capture_inputs;
use madb_app::ports::{FileReference, ReadWorkspace};
use madb_app::{Digest, ErrorKind, LogicalPath, QueryError, Result};

#[derive(Default)]
struct Workspace {
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    reads: RefCell<Vec<(String, usize)>>,
    change: RefCell<Option<Change>>,
}

struct Change {
    at_read: usize,
    path: String,
    bytes: Option<Vec<u8>>,
}

impl Workspace {
    fn put(&self, path: &str, bytes: &[u8]) -> FileReference {
        self.files.borrow_mut().insert(path.into(), bytes.into());
        reference(path, bytes)
    }

    fn change_at(&self, at_read: usize, path: &str, bytes: Option<&[u8]>) {
        *self.change.borrow_mut() = Some(Change {
            at_read,
            path: path.into(),
            bytes: bytes.map(Vec::from),
        });
    }
}

impl ReadWorkspace for Workspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.reads.borrow_mut().push((path.as_str().into(), limit));
        let mut change = self.change.borrow_mut();
        if change
            .as_ref()
            .is_some_and(|change| change.at_read == self.reads.borrow().len())
        {
            let change = change.take().unwrap();
            match change.bytes {
                Some(bytes) => {
                    self.files.borrow_mut().insert(change.path, bytes);
                }
                None => {
                    self.files.borrow_mut().remove(&change.path);
                }
            }
        }
        // Deliberately return oversized data too: app must enforce its port budget.
        Ok(self.files.borrow().get(path.as_str()).cloned())
    }
}

fn reference(path: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: LogicalPath::new(path).unwrap(),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn assert_error(error: QueryError, kind: ErrorKind, code: &str) {
    assert_eq!(error.kind, kind);
    assert_eq!(error.findings.len(), 1);
    assert_eq!(error.findings[0].code, code);
}

fn rejects_before_read(references: Vec<FileReference>, code: &str) {
    let workspace = Workspace::default();
    assert_error(
        capture_inputs(&workspace, references).unwrap_err(),
        ErrorKind::InvalidInput,
        code,
    );
    assert!(workspace.reads.borrow().is_empty());
}

#[test]
fn capture_orders_files_and_all_parent_directories_and_rechecks() {
    let workspace = Workspace::default();
    let z = workspace.put("z/root.txt", b"z");
    let top = workspace.put("top.txt", b"");
    let a = workspace.put("a/deep/item.txt", b"abc");
    let sibling = workspace.put("a/second.txt", b"de");
    let frozen = capture_inputs(&workspace, vec![z, top, sibling, a]).unwrap();

    assert_eq!(frozen.total_bytes(), 6);
    assert_eq!(
        frozen
            .references()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>(),
        ["a/deep/item.txt", "a/second.txt", "top.txt", "z/root.txt"]
    );
    assert_eq!(
        frozen
            .directories()
            .map(LogicalPath::as_str)
            .collect::<Vec<_>>(),
        ["a", "a/deep", "z"]
    );
    let expected = vec![
        ("a/deep/item.txt".into(), 3),
        ("a/second.txt".into(), 2),
        ("top.txt".into(), 0),
        ("z/root.txt".into(), 1),
    ];
    assert_eq!(
        *workspace.reads.borrow(),
        [expected.clone(), expected].concat()
    );
    assert_eq!(
        frozen.files().map(|(_, bytes)| bytes).collect::<Vec<_>>(),
        [b"abc".as_slice(), b"de", b"", b"z"]
    );
}

#[test]
fn rejects_empty_and_excessive_file_counts_without_reads() {
    rejects_before_read(vec![], "INPUT001");
    rejects_before_read(
        (0..4097)
            .map(|i| reference(&format!("f{i}"), b""))
            .collect(),
        "INPUT002",
    );
}

#[test]
fn rejects_declared_byte_budgets_without_allocating_file_contents() {
    let mut too_large = reference("large", b"");
    too_large.size = 32 * 1024 * 1024 + 1;
    rejects_before_read(vec![too_large], "INPUT002");
    let total = (0..5)
        .map(|i| {
            let mut r = reference(&format!("file{i}"), b"");
            r.size = 32 * 1024 * 1024;
            r
        })
        .collect();
    rejects_before_read(total, "INPUT002");
    let mut enormous = reference("huge", b"");
    enormous.size = u64::MAX;
    rejects_before_read(vec![enormous], "INPUT002");
}

#[test]
fn exact_declared_byte_limits_pass_preflight_without_allocating_contents() {
    let workspace = Workspace::default();
    let references = (0..4)
        .map(|i| {
            let mut r = reference(&format!("file{i}"), b"");
            r.size = 32 * 1024 * 1024;
            r
        })
        .collect();
    assert_error(
        capture_inputs(&workspace, references).unwrap_err(),
        ErrorKind::InvalidInput,
        "INPUT003",
    );
    assert_eq!(
        *workspace.reads.borrow(),
        [("file0".into(), 32 * 1024 * 1024)]
    );
}

#[test]
fn rejects_non_ascii_and_excessive_path_depth_without_reads() {
    rejects_before_read(vec![reference("src/文件.kt", b"")], "INPUT001");
    rejects_before_read(vec![reference(&["dir"; 33].join("/"), b"")], "INPUT002");
    let workspace = Workspace::default();
    let allowed = workspace.put(&["dir"; 32].join("/"), b"");
    assert_eq!(
        capture_inputs(&workspace, vec![allowed])
            .unwrap()
            .directories()
            .count(),
        31
    );
}

#[test]
fn rejects_duplicates_and_file_and_directory_case_aliases_before_reads() {
    for paths in [
        ["dir/file", "dir/file"],
        ["dir/File", "dir/file"],
        ["Dir/one", "dir/two"],
        ["root/Dir/one", "root/dir/two"],
    ] {
        rejects_before_read(paths.map(|p| reference(p, b"")).to_vec(), "INPUT001");
    }
}

#[test]
fn rejects_file_directory_ancestor_conflicts_in_either_order() {
    for paths in [
        ["a", "a/b"],
        ["a/b", "a"],
        ["A", "a/b"],
        ["root/a/b", "root/a"],
    ] {
        rejects_before_read(paths.map(|p| reference(p, b"")).to_vec(), "INPUT001");
    }
}

#[test]
fn directory_and_file_count_limits_accept_exactly_and_reject_overflow() {
    let workspace = Workspace::default();
    let exact = (0..4096)
        .map(|i| workspace.put(&format!("a{i}/b{i}/file"), b""))
        .collect();
    let frozen = capture_inputs(&workspace, exact).unwrap();
    assert_eq!(frozen.references().count(), 4096);
    assert_eq!(frozen.directories().count(), 8192);
    let too_many = (0..4096)
        .map(|i| reference(&format!("a{i}/b{i}/c{i}/file"), b""))
        .collect();
    rejects_before_read(too_many, "INPUT002");
}

#[test]
fn missing_wrong_digest_and_shorter_contents_fail_capture() {
    let workspace = Workspace::default();
    assert_error(
        capture_inputs(&workspace, vec![reference("missing", b"a")]).unwrap_err(),
        ErrorKind::InvalidInput,
        "INPUT003",
    );
    workspace.put("file", b"abc");
    for declared in [reference("file", b"xyz"), reference("file", b"abcd")] {
        assert_error(
            capture_inputs(&workspace, vec![declared]).unwrap_err(),
            ErrorKind::InvalidInput,
            "INPUT003",
        );
    }
}

#[test]
fn oversized_port_return_is_rejected_without_large_allocation() {
    let workspace = Workspace::default();
    workspace.put("file", b"ab");
    assert_error(
        capture_inputs(&workspace, vec![reference("file", b"a")]).unwrap_err(),
        ErrorKind::InvalidInput,
        "INPUT002",
    );
    assert_eq!(*workspace.reads.borrow(), [("file".into(), 1)]);
}

#[test]
fn capture_detects_change_or_disappearance_before_returning_a_snapshot() {
    for changed in [Some(b"xyz".as_slice()), Some(b"longer"), None] {
        let workspace = Workspace::default();
        let reference = workspace.put("file", b"abc");
        workspace.change_at(2, "file", changed);
        assert_error(
            capture_inputs(&workspace, vec![reference]).unwrap_err(),
            ErrorKind::Conflict,
            "INPUT004",
        );
    }
}

#[test]
fn mutation_while_capturing_another_file_is_rejected_by_final_recheck() {
    let workspace = Workspace::default();
    let a = workspace.put("a", b"old");
    let b = workspace.put("b", b"stable");
    workspace.change_at(2, "a", Some(b"new"));
    assert_error(
        capture_inputs(&workspace, vec![a, b]).unwrap_err(),
        ErrorKind::Conflict,
        "INPUT004",
    );
}

#[test]
fn later_source_changes_do_not_mutate_captured_or_cloned_bytes() {
    let workspace = Workspace::default();
    let reference = workspace.put("file", b"original");
    let frozen = capture_inputs(&workspace, vec![reference]).unwrap();
    let clone = frozen.clone();
    assert!(std::ptr::eq(
        frozen.files().next().unwrap().1,
        clone.files().next().unwrap().1
    ));
    workspace.put("file", b"modified");
    assert_error(
        frozen.verify(&workspace).unwrap_err(),
        ErrorKind::Conflict,
        "INPUT004",
    );
    assert_eq!(frozen.files().next().unwrap().1, b"original");
    assert_eq!(clone.files().next().unwrap().1, b"original");
    workspace.files.borrow_mut().remove("file");
    assert_error(
        clone.verify(&workspace).unwrap_err(),
        ErrorKind::Conflict,
        "INPUT004",
    );
}

#[test]
fn verify_ignores_undeclared_files_and_succeeds_for_identical_bytes() {
    let workspace = Workspace::default();
    let reference = workspace.put("declared", b"same");
    let frozen = capture_inputs(&workspace, vec![reference]).unwrap();
    workspace.put("undeclared", b"new");
    frozen.verify(&workspace).unwrap();
    assert_eq!(workspace.reads.borrow().len(), 3);
}

#[test]
fn port_errors_are_preserved_on_capture_and_reject_later_verification() {
    struct FailedWorkspace;
    impl ReadWorkspace for FailedWorkspace {
        fn read(&self, path: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
            Err(QueryError::new(
                ErrorKind::Host,
                "HOST_TEST",
                path.as_str(),
                "read failed",
            ))
        }
    }
    let workspace = Workspace::default();
    let reference = workspace.put("file", b"abc");
    assert_error(
        capture_inputs(&FailedWorkspace, vec![reference.clone()]).unwrap_err(),
        ErrorKind::Host,
        "HOST_TEST",
    );
    let frozen = capture_inputs(&workspace, vec![reference]).unwrap();
    let error = frozen.verify(&FailedWorkspace).unwrap_err();
    assert!(error.findings[0].message.contains("HOST_TEST"));
    assert_error(error, ErrorKind::Conflict, "INPUT004");
}
