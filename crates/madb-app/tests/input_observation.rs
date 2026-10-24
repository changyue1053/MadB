//! Port-level observation tests use independent mutable listings, never a real
//! filesystem or production enumeration to construct the expected result.
use madb_app::{
    ErrorKind, LogicalPath, QueryError, Result,
    input_observation::{
        DirectoryEntry, DirectoryEntryKind, InputObservation, InputWatchSet, ObserveWorkspace,
    },
    ports::ReadWorkspace,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
};

const NAMES_LIMIT: usize = 2 * 1024 * 1024;

fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn entry(name: &str, kind: DirectoryEntryKind) -> DirectoryEntry {
    DirectoryEntry {
        name: path(name),
        kind,
    }
}
fn file(name: &str) -> DirectoryEntry {
    entry(name, DirectoryEntryKind::File)
}
fn directory(name: &str) -> DirectoryEntry {
    entry(name, DirectoryEntryKind::Directory)
}
fn watch(absent: &[&str], directories: &[&str]) -> InputWatchSet {
    InputWatchSet::new(
        absent.iter().map(|name| path(name)).collect(),
        directories.iter().map(|name| path(name)).collect(),
    )
    .unwrap()
}
fn assert_error(error: QueryError, kind: ErrorKind, code: &str) {
    assert_eq!(error.kind, kind, "{error}");
    assert_eq!(error.findings[0].code, code, "{error}");
}

#[derive(Debug, PartialEq)]
enum Call {
    Exists(String),
    List(String, usize, usize),
}
enum Change {
    Present(String),
    Entries(String, Vec<DirectoryEntry>),
}
#[derive(Default)]
struct Workspace {
    present: RefCell<BTreeSet<String>>,
    entries: RefCell<BTreeMap<String, Vec<DirectoryEntry>>>,
    calls: RefCell<Vec<Call>>,
    change: RefCell<Option<(usize, Change)>>,
    fail_at: Cell<usize>,
}
impl Workspace {
    fn put(&self, name: &str, entries: Vec<DirectoryEntry>) {
        self.entries.borrow_mut().insert(name.into(), entries);
    }
    fn record(&self, call: Call) -> Result<()> {
        self.calls.borrow_mut().push(call);
        let count = self.calls.borrow().len();
        let change = self.change.borrow_mut().take();
        match change {
            Some((at, change)) if at == count => match change {
                Change::Present(name) => {
                    self.present.borrow_mut().insert(name);
                }
                Change::Entries(name, entries) => self.put(&name, entries),
            },
            other => *self.change.borrow_mut() = other,
        }
        if self.fail_at.get() == count {
            return Err(QueryError::new(
                ErrorKind::Host,
                "HOST002",
                "fixture",
                "injected directory access failure",
            ));
        }
        Ok(())
    }
}
impl ReadWorkspace for Workspace {
    fn read(&self, _: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
        panic!("watch observations must not read member contents")
    }
}
impl ObserveWorkspace for Workspace {
    fn exists(&self, path: &LogicalPath) -> Result<bool> {
        self.record(Call::Exists(path.as_str().into()))?;
        Ok(self.present.borrow().contains(path.as_str()))
    }
    fn list_directory(
        &self,
        path: &LogicalPath,
        entry_limit: usize,
        name_bytes_limit: usize,
    ) -> Result<Vec<DirectoryEntry>> {
        self.record(Call::List(
            path.as_str().into(),
            entry_limit,
            name_bytes_limit,
        ))?;
        // Deliberately do not enforce limits: application must check its port.
        self.entries
            .borrow()
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| {
                QueryError::new(
                    ErrorKind::Host,
                    "HOST002",
                    path.as_str(),
                    "directory missing",
                )
            })
    }
}

#[test]
fn empty_watch_captures_and_verifies_without_io() {
    let workspace = Workspace::default();
    let observation = InputObservation::capture(&workspace, &watch(&[], &[])).unwrap();
    observation.verify(&workspace).unwrap();
    assert!(workspace.calls.borrow().is_empty());
}

#[test]
fn watch_order_is_canonical_and_nested_directories_are_valid() {
    let watched = watch(
        &["z/optional", "a/missing", "a/missing/deep"],
        &["z", "a", "a/sub"],
    );
    assert_eq!(
        watched
            .absent()
            .iter()
            .map(LogicalPath::as_str)
            .collect::<Vec<_>>(),
        ["a/missing", "a/missing/deep", "z/optional"]
    );
    assert_eq!(
        watched
            .directories()
            .iter()
            .map(LogicalPath::as_str)
            .collect::<Vec<_>>(),
        ["a", "a/sub", "z"]
    );
}

#[test]
fn impossible_watch_layouts_are_rejected_without_a_workspace() {
    for (absent, directories) in [
        (vec!["same", "same"], vec![]),
        (vec![], vec!["same", "same"]),
        (vec!["Dir/optional"], vec!["dir"]),
        (vec![], vec!["Dir/sub", "dir/other"]),
        (vec!["missing"], vec!["missing"]),
        (vec!["missing"], vec!["missing/child"]),
        (vec!["café"], vec![]),
    ] {
        let error = InputWatchSet::new(
            absent.into_iter().map(path).collect(),
            directories.into_iter().map(path).collect(),
        )
        .err()
        .unwrap();
        assert_error(error, ErrorKind::InvalidInput, "OBS001");
    }
}

#[test]
fn watch_count_and_depth_budgets_are_checked_before_io() {
    for (absent, directories) in [
        (
            (0..4097).map(|i| path(&format!("absent-{i}"))).collect(),
            vec![],
        ),
        (
            vec![],
            (0..257).map(|i| path(&format!("dir-{i}"))).collect(),
        ),
        (
            vec![path(
                &std::iter::repeat_n("level", 33)
                    .collect::<Vec<_>>()
                    .join("/"),
            )],
            vec![],
        ),
    ] {
        let error = InputWatchSet::new(absent, directories).err().unwrap();
        assert_error(error, ErrorKind::InvalidInput, "OBS002");
    }
    assert!(
        InputWatchSet::new(
            (0..4096).map(|i| path(&format!("absent-{i}"))).collect(),
            vec![]
        )
        .is_ok()
    );
    assert!(
        InputWatchSet::new(
            vec![],
            (0..256).map(|i| path(&format!("dir-{i}"))).collect()
        )
        .is_ok()
    );
}

#[test]
fn capture_checks_absence_and_rechecks_before_returning() {
    let workspace = Workspace::default();
    InputObservation::capture(&workspace, &watch(&["optional"], &[])).unwrap();
    assert_eq!(
        *workspace.calls.borrow(),
        [
            Call::Exists("optional".into()),
            Call::Exists("optional".into())
        ]
    );
    workspace.present.borrow_mut().insert("optional".into());
    let error = InputObservation::capture(&workspace, &watch(&["optional"], &[]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS003");
}

#[test]
fn absence_created_during_capture_is_a_conflict() {
    let workspace = Workspace::default();
    *workspace.change.borrow_mut() = Some((2, Change::Present("optional".into())));
    let error = InputObservation::capture(&workspace, &watch(&["optional"], &[]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::Conflict, "OBS003");
}

#[test]
fn directory_order_does_not_change_observation() {
    let workspace = Workspace::default();
    workspace.put("src", vec![file("z.h"), directory("nested"), file("a.h")]);
    *workspace.change.borrow_mut() = Some((
        2,
        Change::Entries(
            "src".into(),
            vec![file("a.h"), file("z.h"), directory("nested")],
        ),
    ));
    let observation = InputObservation::capture(&workspace, &watch(&[], &["src"])).unwrap();
    observation.verify(&workspace).unwrap();
}

#[test]
fn directory_add_remove_and_kind_change_are_conflicts() {
    for changed in [
        vec![file("a.h"), directory("nested"), file("new.h")],
        vec![file("a.h")],
        vec![directory("a.h"), directory("nested")],
        vec![file("a.h"), file("nested")],
    ] {
        let workspace = Workspace::default();
        workspace.put("src", vec![file("a.h"), directory("nested")]);
        let observation = InputObservation::capture(&workspace, &watch(&[], &["src"])).unwrap();
        workspace.put("src", changed);
        assert_error(
            observation.verify(&workspace).err().unwrap(),
            ErrorKind::Conflict,
            "OBS003",
        );
    }
}

#[test]
fn optional_creation_after_capture_is_a_conflict() {
    let workspace = Workspace::default();
    let observation =
        InputObservation::capture(&workspace, &watch(&["config/optional"], &[])).unwrap();
    workspace
        .present
        .borrow_mut()
        .insert("config/optional".into());
    assert_error(
        observation.verify(&workspace).err().unwrap(),
        ErrorKind::Conflict,
        "OBS003",
    );
}

#[test]
fn directory_initial_and_later_errors_remain_failures() {
    let workspace = Workspace::default();
    let error = InputObservation::capture(&workspace, &watch(&[], &["missing"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::Host, "HOST002");

    workspace.put("src", vec![file("a.h")]);
    let observation = InputObservation::capture(&workspace, &watch(&[], &["src"])).unwrap();
    workspace.entries.borrow_mut().remove("src");
    let error = observation.verify(&workspace).err().unwrap();
    assert_error(error, ErrorKind::Conflict, "OBS003");
}

#[test]
fn capture_and_verify_preserve_existence_error_distinction() {
    let workspace = Workspace::default();
    workspace.fail_at.set(1);
    let error = InputObservation::capture(&workspace, &watch(&["optional"], &[]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::Host, "HOST002");

    workspace.fail_at.set(3);
    let error = InputObservation::capture(&workspace, &watch(&["optional"], &[]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::Conflict, "OBS003");
}

#[test]
fn invalid_directory_members_are_not_silently_filtered() {
    for members in [
        vec![file("same"), file("same")],
        vec![file("Same"), file("same")],
        vec![file("nested/member")],
        vec![file("café")],
    ] {
        let workspace = Workspace::default();
        workspace.put("src", members);
        let error = InputObservation::capture(&workspace, &watch(&[], &["src"]))
            .err()
            .unwrap();
        assert_error(error, ErrorKind::InvalidInput, "OBS001");
    }
}

#[test]
fn capture_detects_member_changes_during_its_recheck() {
    let workspace = Workspace::default();
    workspace.put("src", vec![file("a.h")]);
    *workspace.change.borrow_mut() = Some((
        2,
        Change::Entries("src".into(), vec![file("a.h"), file("b.h")]),
    ));
    let error = InputObservation::capture(&workspace, &watch(&[], &["src"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::Conflict, "OBS003");
}

#[test]
fn directory_entry_budget_is_aggregate_and_passed_to_each_read() {
    let workspace = Workspace::default();
    let entries = |count| {
        (0..count)
            .map(|i| file(&format!("entry-{i:04}")))
            .collect::<Vec<_>>()
    };
    workspace.put("a", entries(4096));
    workspace.put("b", entries(4097));
    let error = InputObservation::capture(&workspace, &watch(&[], &["a", "b"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS002");
    assert_eq!(
        *workspace.calls.borrow(),
        [
            Call::List("a".into(), 8192, NAMES_LIMIT),
            Call::List("b".into(), 4096, NAMES_LIMIT - 4096 * 10),
        ]
    );
}

#[test]
fn directory_name_budget_is_enforced_even_below_the_entry_limit() {
    let workspace = Workspace::default();
    let names = (0..2049)
        .map(|i| file(&format!("{i:08}{}", "x".repeat(1016))))
        .collect();
    workspace.put("src", names);
    let error = InputObservation::capture(&workspace, &watch(&[], &["src"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS002");
    assert_eq!(workspace.calls.borrow().len(), 1);
}

#[test]
fn unobserved_subdirectory_members_do_not_change_direct_listing() {
    let workspace = Workspace::default();
    workspace.put("src", vec![directory("nested")]);
    workspace.put("src/nested", vec![file("before")]);
    let observation = InputObservation::capture(&workspace, &watch(&[], &["src"])).unwrap();
    workspace.put("src/nested", vec![file("after")]);
    observation.verify(&workspace).unwrap();
    assert!(
        workspace
            .calls
            .borrow()
            .iter()
            .all(|call| matches!(call, Call::List(name, _, _) if name == "src"))
    );
}

#[test]
fn declaration_prefix_budget_rejects_many_deep_paths_without_io() {
    let absent = (0..512)
        .map(|index| {
            let suffix = std::iter::repeat_n("nested", 16)
                .collect::<Vec<_>>()
                .join("/");
            path(&format!("root-{index:04}/{suffix}"))
        })
        .collect();
    let error = InputWatchSet::new(absent, vec![]).err().unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS002");
}

#[test]
fn name_byte_budget_is_reduced_before_the_next_directory_call() {
    let workspace = Workspace::default();
    let names = |count| {
        (0..count)
            .map(|i| file(&format!("{i:08}{}", "x".repeat(1016))))
            .collect::<Vec<_>>()
    };
    workspace.put("a", names(1024));
    workspace.put("b", names(1025));
    let error = InputObservation::capture(&workspace, &watch(&[], &["a", "b"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS002");
    assert_eq!(
        *workspace.calls.borrow(),
        [
            Call::List("a".into(), 8192, NAMES_LIMIT),
            Call::List("b".into(), 7168, 1024 * 1024),
        ]
    );
}

#[test]
fn absence_observation_must_agree_with_watched_parent_membership() {
    let workspace = Workspace::default();
    // exists returns false, but the independently supplied listing says present.
    workspace.put("inc", vec![file("a")]);
    let error = InputObservation::capture(&workspace, &watch(&["inc/a"], &["inc"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS001");
}

#[test]
fn watched_parent_must_include_an_explicitly_watched_child_directory() {
    let workspace = Workspace::default();
    workspace.put("inc", vec![]);
    workspace.put("inc/sub", vec![]);
    let error = InputObservation::capture(&workspace, &watch(&[], &["inc", "inc/sub"]))
        .err()
        .unwrap();
    assert_error(error, ErrorKind::InvalidInput, "OBS001");
}

#[test]
fn parent_listing_cannot_alias_or_change_kind_of_a_watched_child() {
    for member in [directory("Sub"), file("sub")] {
        let workspace = Workspace::default();
        workspace.put("inc", vec![member]);
        workspace.put("inc/sub", vec![]);
        let error = InputObservation::capture(&workspace, &watch(&[], &["inc", "inc/sub"]))
            .err()
            .unwrap();
        assert_error(error, ErrorKind::InvalidInput, "OBS001");
    }
}

#[test]
fn one_directory_can_use_the_complete_member_budget() {
    let workspace = Workspace::default();
    workspace.put(
        "inc",
        (0..8192).map(|i| file(&format!("entry-{i:04}"))).collect(),
    );
    let observation = InputObservation::capture(&workspace, &watch(&[], &["inc"])).unwrap();
    observation.verify(&workspace).unwrap();
    assert_eq!(workspace.calls.borrow().len(), 3);
}
