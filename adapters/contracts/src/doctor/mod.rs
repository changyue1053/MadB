//! Explicit doctor wire/domain conversion. No filesystem or process authority.
mod report;
use crate::project_init::wire::{digest, id, reference, string};
use crate::query::{PreparedQuery, SnapshotAdapter, format_error};
use madb_app::ports::{
    ConfigurationBytes, FileReference as InputRef, RequestFiles, SnapshotFormat,
};
use madb_app::use_cases::doctor::*;
use madb_app::{ErrorKind, QueryError, Result, TaskGraph};
use madb_model::contract::{Contract, Direction, ResolvedSelector};
use madb_protocol::{DocumentKind, SchemaRegistry, Value};

pub struct DoctorAdapter {
    snapshot: SnapshotAdapter,
    registry: SchemaRegistry,
}
impl DoctorAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            snapshot: SnapshotAdapter::new()?,
            registry: SchemaRegistry::new().map_err(format_error)?,
        })
    }
    /// Decoded observations are not a checked context or an execution permit.
    pub fn decode_result(&self, bytes: &[u8]) -> Result<Report> {
        let doc = self
            .registry
            .decode(DocumentKind::DoctorResult, bytes)
            .map_err(format_error)?;
        let report = report::decode(doc.value())?;
        report.validate()?;
        Ok(report)
    }
    pub fn encode_result(&self, value: &Report) -> Result<(Report, Vec<u8>)> {
        value.validate()?;
        let bytes = canonical(value)?;
        if bytes.len() <= RESULT_LIMIT {
            self.registry
                .validate(DocumentKind::DoctorResult, report::encode(value))
                .map_err(format_error)?;
            return Ok((value.clone(), bytes));
        }
        let short = value.output_limit();
        short.validate()?;
        let bytes = self.encode_unbounded(&short)?;
        if bytes.len() > RESULT_LIMIT {
            return Err(QueryError::new(
                ErrorKind::Host,
                "DOCTOR001",
                "doctor result",
                "bounded summary exceeds the result budget",
            ));
        }
        Ok((short, bytes))
    }
    fn encode_unbounded(&self, report: &Report) -> Result<Vec<u8>> {
        let value = report::encode(report);
        // Schema validation does not confer preflight success; app validates
        // the independent built-in set and all status/identity implications.
        self.registry
            .validate(DocumentKind::DoctorResult, value.clone())
            .map_err(format_error)?;
        let mut bytes = madb_protocol::canonical_json(&value).map_err(format_error)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
impl SnapshotFormat for DoctorAdapter {
    type Prepared = PreparedQuery;
    fn request_files(&self, bytes: &[u8]) -> Result<RequestFiles> {
        self.snapshot.request_files(bytes)
    }
    fn prepare(&self, bytes: ConfigurationBytes<'_>) -> Result<(PreparedQuery, Vec<InputRef>)> {
        self.snapshot.prepare(bytes)
    }
    fn check(&self, p: &PreparedQuery, bytes: &[u8]) -> Result<Contract> {
        self.snapshot.check(p, bytes)
    }
    fn task_graph(&self, p: &PreparedQuery, bytes: &[Vec<u8>]) -> Result<TaskGraph> {
        self.snapshot.task_graph(p, bytes)
    }
}
impl DoctorFormat for DoctorAdapter {
    fn context(
        &self,
        p: &PreparedQuery,
        project: FileIdentity,
        encoding: ProjectEncoding,
        request: FileIdentity,
    ) -> Result<Context> {
        Ok(Context {
            project,
            project_format: encoding,
            read_request: FileReference {
                file: request,
                schema: madb_app::Id::new("madb.read-request/1").expect("fixed"),
            },
            lock: reference(&p.request.value()["lock"])?,
            profile: reference(&p.request.value()["profile"])?,
            snapshot: None,
            selector: selector(&p.request.value()["selector"])?,
            recipe: None,
        })
    }
    fn sdk_reference(&self, p: &PreparedQuery) -> Result<FileReference> {
        reference(&p.profile.value()["sdk"])
    }
}
fn selector(v: &Value) -> Result<ResolvedSelector> {
    Ok(ResolvedSelector {
        route: id(&v["route"])?,
        profile_id: id(&v["profile_id"])?,
        profile_digest: digest(&v["profile_digest"])?,
        target: id(&v["target"])?,
        direction: match string(&v["direction"])? {
            "consumer-to-provider" => Direction::ConsumerToProvider,
            "host-callback" => Direction::HostCallback,
            "reverse-export" => Direction::ReverseExport,
            _ => return Err(invalid("unknown selector direction")),
        },
    })
}
fn invalid(message: &str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", "doctor result", message)
}

fn canonical(report: &Report) -> Result<Vec<u8>> {
    let mut bytes = madb_protocol::canonical_json(&report::encode(report)).map_err(format_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Fixed-schema fallback for failures before the schema registry is available.
pub fn early_error(error: &QueryError) -> (Report, Vec<u8>) {
    let mut report = Report::default();
    report.fail(error);
    report.recovery();
    let bytes = serde_json::to_vec(&report::encode(&report))
        .expect("fixed report contains only JSON values");
    let mut bytes = bytes;
    bytes.push(b'\n');
    (report, bytes)
}
