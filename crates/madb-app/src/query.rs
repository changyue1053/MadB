use crate::ports::{ConfigurationBytes, FileReference, ReadWorkspace, SnapshotFormat};
use crate::{Digest, ErrorKind, Finding, Id, LogicalPath, QueryError, Result, TaskGraph};
use madb_model::contract::Contract;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const DOCUMENT_LIMIT: usize = 1024 * 1024;
pub(crate) const SOURCE_LIMIT: usize = 32 * 1024 * 1024;
const QUERY_LIMIT: usize = 128 * 1024 * 1024;

pub struct QueryRequest {
    pub project: LogicalPath,
    pub request: LogicalPath,
    pub snapshot: LogicalPath,
    /// Explicit existing task documents; never discover or invoke a backend.
    pub tasks: Vec<LogicalPath>,
    pub target: Option<Id>,
    pub active_tasks: Vec<Id>,
}

#[derive(Debug)]
pub enum QueryOutcome {
    /// Only the declared, byte-checked inventory matches. Completeness of a
    /// compiler dependency closure is outside this read-only command's proof.
    Checked {
        contract: Box<Contract>,
        graph: Option<TaskGraph>,
        observed_files: usize,
    },
    MetadataRequired {
        findings: Vec<Finding>,
    },
}

/// All query paths are read-only, including errors. The port has no write,
/// process or cache-update operation. Observed bytes are rechecked before a
/// result is returned; this is conflict detection, not a filesystem snapshot.
pub fn query<W: ReadWorkspace, F: SnapshotFormat>(
    workspace: &W,
    format: &F,
    request: &QueryRequest,
) -> Result<QueryOutcome> {
    if request.tasks.len() > 4096 {
        return Err(QueryError::new(
            ErrorKind::InvalidInput,
            "QUERY001",
            "tasks",
            "too many task documents",
        ));
    }
    let unique: BTreeSet<_> = request.tasks.iter().collect();
    if unique.len() != request.tasks.len() {
        return Err(QueryError::new(
            ErrorKind::InvalidInput,
            "PLAN001",
            "tasks",
            "duplicate task document path",
        ));
    }
    let mut reads = Reads::new(workspace);
    let project = reads.required(&request.project, DOCUMENT_LIMIT)?;
    let reader_request = reads.required(&request.request, DOCUMENT_LIMIT)?;
    let refs = format.request_files(&reader_request)?;
    let lock = reads.required(&refs.lock.path, DOCUMENT_LIMIT)?;
    let profile = reads.required(&refs.profile.path, DOCUMENT_LIMIT)?;
    for (reference, bytes) in [(&refs.lock, &lock), (&refs.profile, &profile)] {
        if !matches(reference, bytes) {
            return Err(QueryError::new(
                ErrorKind::InvalidInput,
                "FILE004",
                reference.path.as_str(),
                "configuration bytes differ from the read request; prepare a matching request and lock",
            ));
        }
    }
    let (prepared, tracked) = format.prepare(ConfigurationBytes {
        request: &reader_request,
        project: &project,
        lock: &lock,
        profile: &profile,
    })?;
    let mut stale = Vec::new();
    for reference in tracked {
        let bytes = reads.optional(&reference.path, SOURCE_LIMIT)?;
        if bytes
            .as_ref()
            .is_none_or(|bytes| !matches(&reference, bytes))
        {
            stale.push(Finding::new(
                "META001",
                reference.path.as_str(),
                "tracked input is missing or changed; metadata preparation is required",
            ));
        }
    }
    let snapshot = reads.optional(&request.snapshot, DOCUMENT_LIMIT)?;
    if snapshot.is_none() {
        stale.push(Finding::new(
            "META001",
            request.snapshot.as_str(),
            "snapshot is missing; metadata preparation is required",
        ));
    }
    if !stale.is_empty() {
        reads.recheck()?;
        return Ok(QueryOutcome::MetadataRequired { findings: stale });
    }
    let contract = match format.check(&prepared, &snapshot.expect("checked presence")) {
        Err(error)
            if error
                .findings
                .iter()
                .any(|f| f.code == "LOCK005" || f.code == "PORT002") =>
        {
            reads.recheck()?;
            return Ok(QueryOutcome::MetadataRequired {
                findings: error.findings,
            });
        }
        result => result?,
    };
    let graph = if request.tasks.is_empty() {
        if request.target.is_some() || !request.active_tasks.is_empty() {
            return Err(QueryError::new(
                ErrorKind::InvalidInput,
                "PLAN001",
                "tasks",
                "named planning requires task documents",
            ));
        }
        None
    } else {
        let payloads = request
            .tasks
            .iter()
            .map(|path| reads.required(path, DOCUMENT_LIMIT))
            .collect::<Result<Vec<_>>>()?;
        let graph = format.task_graph(&prepared, &payloads)?;
        let selected = if let Some(target) = &request.target {
            graph.subgraph(target, &request.active_tasks)
        } else {
            graph.reject_active(&request.active_tasks).map(|()| graph)
        }
        .map_err(|e| QueryError::new(ErrorKind::CheckFailed, "PLAN001", "tasks", e.to_string()))?;
        Some(selected)
    };
    reads.recheck()?;
    Ok(QueryOutcome::Checked {
        contract: Box::new(contract),
        graph,
        observed_files: reads.observed.len(),
    })
}

pub(crate) fn matches(reference: &FileReference, bytes: &[u8]) -> bool {
    reference.size == bytes.len() as u64 && reference.digest == Digest::of_bytes(bytes)
}

pub(crate) struct Reads<'a, W> {
    workspace: &'a W,
    observed: BTreeMap<LogicalPath, Option<Vec<u8>>>,
    bytes: usize,
}
impl<'a, W: ReadWorkspace> Reads<'a, W> {
    pub(crate) fn new(workspace: &'a W) -> Self {
        Self {
            workspace,
            observed: BTreeMap::new(),
            bytes: 0,
        }
    }
    pub(crate) fn optional(&mut self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        if let Some(bytes) = self.observed.get(path) {
            if bytes.as_ref().is_some_and(|b| b.len() > limit) {
                return Err(budget(path));
            }
            return Ok(bytes.clone());
        }
        let bytes = self
            .workspace
            .read(path, limit.min(QUERY_LIMIT - self.bytes))?;
        if let Some(bytes) = &bytes {
            if bytes.len() > limit || bytes.len() > QUERY_LIMIT - self.bytes {
                return Err(budget(path));
            }
            self.bytes += bytes.len();
        }
        self.observed.insert(path.clone(), bytes.clone());
        Ok(bytes)
    }
    pub(crate) fn required(&mut self, path: &LogicalPath, limit: usize) -> Result<Vec<u8>> {
        self.optional(path, limit)?.ok_or_else(|| {
            QueryError::new(
                ErrorKind::InvalidInput,
                "HOST001",
                path.as_str(),
                "required configuration file is missing",
            )
        })
    }
    pub(crate) fn recheck(&self) -> Result<()> {
        for (path, expected) in &self.observed {
            // Read only one byte beyond the first observation to detect growth.
            let actual = self
                .workspace
                .read(path, expected.as_ref().map_or(0, Vec::len));
            match actual {
                Ok(actual) if &actual == expected => {}
                _ => {
                    return Err(QueryError::new(
                        ErrorKind::Conflict,
                        "QUERY002",
                        path.as_str(),
                        "input changed during query; retry on a stable workspace",
                    ));
                }
            }
        }
        Ok(())
    }
}
fn budget(path: &LogicalPath) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "QUERY003",
        path.as_str(),
        "query byte budget exceeded",
    )
}
