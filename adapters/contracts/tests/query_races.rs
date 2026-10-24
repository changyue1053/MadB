mod common;
use madb_app::ports::{
    ConfigurationBytes, FileReference, ReadWorkspace, RequestFiles, SnapshotFormat,
};
use madb_app::query::{QueryOutcome, QueryRequest, query};
use madb_app::{Digest, ErrorKind, LogicalPath, Result, TaskGraph};
use madb_model::contract::Contract;
use std::{cell::RefCell, collections::BTreeMap};

fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn reference(value: &str) -> FileReference {
    FileReference {
        path: path(value),
        digest: Digest::of_bytes(value.as_bytes()),
        size: value.len() as u64,
    }
}
struct Format;
impl SnapshotFormat for Format {
    type Prepared = ();
    fn request_files(&self, _: &[u8]) -> Result<RequestFiles> {
        Ok(RequestFiles {
            lock: reference("lock"),
            profile: reference("profile"),
        })
    }
    fn prepare(&self, _: ConfigurationBytes<'_>) -> Result<((), Vec<FileReference>)> {
        Ok(((), vec![reference("source")]))
    }
    fn check(&self, _: &(), _: &[u8]) -> Result<Contract> {
        Ok(
            madb_contracts::contract(&common::document(madb_contracts::DocumentKind::ContractIr))
                .unwrap(),
        )
    }
    fn task_graph(&self, _: &(), _: &[Vec<u8>]) -> Result<TaskGraph> {
        panic!("no tasks requested")
    }
}
struct Workspace {
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    change_on_snapshot: bool,
    calls: RefCell<Vec<String>>,
}
impl Workspace {
    fn new(change: bool) -> Self {
        Self {
            files: RefCell::new(
                [
                    "project", "request", "lock", "profile", "source", "snapshot",
                ]
                .into_iter()
                .map(|s| (s.into(), s.as_bytes().to_vec()))
                .collect(),
            ),
            change_on_snapshot: change,
            calls: RefCell::new(Vec::new()),
        }
    }
}
impl ReadWorkspace for Workspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.calls.borrow_mut().push(path.as_str().into());
        if path.as_str() == "snapshot" && self.change_on_snapshot {
            self.files
                .borrow_mut()
                .insert("source".into(), b"different".to_vec());
        }
        let value = self.files.borrow().get(path.as_str()).cloned();
        if value.as_ref().is_some_and(|v| v.len() > limit) {
            return Err(madb_app::QueryError::new(
                ErrorKind::InvalidInput,
                "HOST003",
                path.as_str(),
                "read limit",
            ));
        }
        Ok(value)
    }
}
fn request() -> QueryRequest {
    QueryRequest {
        project: path("project"),
        request: path("request"),
        snapshot: path("snapshot"),
        tasks: vec![],
        target: None,
        active_tasks: vec![],
    }
}

#[test]
fn unchanged_observations_are_rechecked_before_success() {
    let workspace = Workspace::new(false);
    assert!(matches!(
        query(&workspace, &Format, &request()).unwrap(),
        QueryOutcome::Checked {
            observed_files: 6,
            ..
        }
    ));
    assert_eq!(workspace.calls.borrow().len(), 12);
}

#[test]
fn changed_source_between_capture_and_return_is_conflict() {
    let workspace = Workspace::new(true);
    let error = query(&workspace, &Format, &request()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert_eq!(error.findings[0].code, "QUERY002");
}

#[test]
fn missing_source_appearing_during_query_is_conflict_not_stale_result() {
    let workspace = Workspace::new(true);
    workspace.files.borrow_mut().remove("source");
    let error = query(&workspace, &Format, &request()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert_eq!(error.findings[0].code, "QUERY002");
}

#[test]
fn missing_snapshot_is_metadata_requirement() {
    let workspace = Workspace::new(false);
    workspace.files.borrow_mut().remove("snapshot");
    assert!(matches!(
        query(&workspace, &Format, &request()).unwrap(),
        QueryOutcome::MetadataRequired { .. }
    ));
}

#[test]
fn configuration_reference_failure_does_not_reach_format_check() {
    let workspace = Workspace::new(false);
    workspace
        .files
        .borrow_mut()
        .insert("profile".into(), b"tampered".to_vec());
    let error = query(&workspace, &Format, &request()).unwrap_err();
    assert_eq!(error.findings[0].code, "FILE004");
    assert!(!workspace.calls.borrow().contains(&"snapshot".into()));
}

#[test]
fn duplicate_document_paths_are_rejected_before_io() {
    let workspace = Workspace::new(false);
    let mut request = request();
    request.tasks = vec![path("large-task.json"); 4096];
    let error = query(&workspace, &Format, &request).unwrap_err();
    assert_eq!(error.findings[0].code, "PLAN001");
    assert!(workspace.calls.borrow().is_empty());
}
