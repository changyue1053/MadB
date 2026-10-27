mod common;
use madb_app::ports::{
    ConfigurationBytes, FileReference as InputRef, ReadWorkspace, RequestFiles, SnapshotFormat,
};
use madb_app::use_cases::{doctor::*, initialize::VerifyLevel};
use madb_app::{Digest, ErrorKind, LogicalPath, Result, TaskGraph};
use madb_model::contract::Contract;
use std::{cell::RefCell, collections::BTreeMap};

fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn reference(value: &str) -> InputRef {
    InputRef {
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
    fn prepare(&self, _: ConfigurationBytes<'_>) -> Result<((), Vec<InputRef>)> {
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

impl DoctorFormat for Format {
    fn context(
        &self,
        _: &(),
        project: FileIdentity,
        encoding: ProjectEncoding,
        read: FileIdentity,
    ) -> Result<Context> {
        let c =
            madb_contracts::contract(&common::document(madb_contracts::DocumentKind::ContractIr))
                .unwrap();
        Ok(Context {
            project,
            project_format: encoding,
            read_request: FileReference {
                file: read,
                schema: common::id("madb.read-request/1"),
            },
            lock: domain_ref("lock", "madb.lock/1"),
            profile: domain_ref("profile", "madb.profile/1"),
            snapshot: None,
            selector: c.selector,
            recipe: None,
        })
    }
    fn sdk_reference(&self, _: &()) -> Result<FileReference> {
        Ok(domain_ref("sdk", "madb.native-toolchain/1"))
    }
}
fn domain_ref(name: &str, schema: &str) -> FileReference {
    FileReference {
        file: FileIdentity {
            path: path(name),
            sha256: Digest::of_bytes(name.as_bytes()),
            size: name.len() as u64,
        },
        schema: common::id(schema),
    }
}
fn request() -> Request {
    Request {
        root: RootPath::new("/design-only").unwrap(),
        project: path("project"),
        read_request: path("request"),
        snapshot: path("snapshot"),
        verify: VerifyLevel::Structure,
    }
}
#[test]
fn full_check_is_required_before_configuration_passes() {
    let workspace = Workspace::new(false);
    let mut query = DoctorQuery::new(&workspace);
    let mut report = Report::default();
    assert!(query.check(&Format, &request(), &mut report).is_some());
    query.revalidate(&mut report);
    assert_eq!(
        report
            .checks
            .iter()
            .find(|v| v.id.as_str() == "context.configuration")
            .unwrap()
            .status,
        Status::Passed
    );
    assert_eq!(workspace.calls.borrow().len(), 12);
}
#[test]
fn missing_snapshot_and_stale_source_have_precise_dependency_states() {
    for missing in ["snapshot", "source"] {
        let workspace = Workspace::new(false);
        workspace.files.borrow_mut().remove(missing);
        let mut query = DoctorQuery::new(&workspace);
        let mut report = Report::default();
        assert!(query.check(&Format, &request(), &mut report).is_none());
        query.revalidate(&mut report);
        report.finish().unwrap();
        let failed = if missing == "snapshot" {
            "context.snapshot"
        } else {
            "context.declared-inputs"
        };
        let check = report
            .checks
            .iter()
            .find(|v| v.id.as_str() == failed)
            .unwrap();
        assert_eq!(check.status, Status::Failed);
        assert_eq!(check.error_kind, Some(ErrorKind::InvalidInput));
        assert_eq!(check.diagnostics[0].code.as_str(), "META001");
        assert_eq!(report.context.as_ref().unwrap().snapshot, None);
        assert_eq!(
            report
                .checks
                .iter()
                .find(|v| v.id.as_str() == "context.configuration")
                .unwrap()
                .status,
            Status::Blocked
        );
    }
}
#[test]
fn failed_query_also_rechecks_captured_bytes() {
    let workspace = Workspace::new(false);
    workspace.files.borrow_mut().remove("snapshot");
    let mut query = DoctorQuery::new(&workspace);
    let mut report = Report::default();
    assert!(query.check(&Format, &request(), &mut report).is_none());
    workspace
        .files
        .borrow_mut()
        .insert("source".into(), b"changed".to_vec());
    query.revalidate(&mut report);
    report.finish().unwrap();
    assert_eq!(report.exit_code, 4);
}
#[test]
fn malformed_configuration_blocks_without_reading_source_or_environment() {
    let workspace = Workspace::new(false);
    workspace
        .files
        .borrow_mut()
        .insert("profile".into(), b"changed".to_vec());
    let mut query = DoctorQuery::new(&workspace);
    let mut report = Report::default();
    assert!(query.check(&Format, &request(), &mut report).is_none());
    query.revalidate(&mut report);
    report.finish().unwrap();
    assert!(report.context.is_none());
    assert!(
        !workspace
            .calls
            .borrow()
            .iter()
            .any(|v| v == "source" || v == "snapshot")
    );
    assert_eq!(report.exit_code, 2);
}
