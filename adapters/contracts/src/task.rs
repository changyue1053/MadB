//! Bind a declared one-shot recipe to checked configuration and result payloads.
//! This adapter has no filesystem or process access and does not certify a batch.
use crate::{
    Document, DocumentKind,
    artifacts::validate_port_pair,
    query::{PreparedQuery, SnapshotAdapter, format_error, reference},
};
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    ports::{ConfigurationBytes, FileReference, SnapshotFormat},
    process::{MAX_STDIN_BYTES, MAX_TOOL_BYTES, ProcessRequest},
    task::{BuildRecipe, TaskFileReferences, TaskInvocation},
};
use std::collections::BTreeMap;

/// A fully checked configuration/task binding. Only the host-supplied installed
/// image location becomes executable; task component identifiers are not paths.
pub struct BoundBuildTask {
    adapter: SnapshotAdapter,
    task: Document,
    invocation: TaskInvocation,
}
impl BoundBuildTask {
    /// Check configuration byte references, contract semantics, task identity
    /// and the selected profile's locked image before creating an invocation.
    ///
    /// This slice accepts no dependencies and requires work/. input_digest is
    /// the checked source lineage; individual task inputs have separate byte
    /// identities. generation_id is matched in the response, not certified here.
    pub fn new(
        configuration: ConfigurationBytes<'_>,
        snapshot: &[u8],
        task: &[u8],
        installed_tool: FileReference,
    ) -> Result<Self> {
        if task.len() > MAX_STDIN_BYTES {
            return Err(invalid(
                "TASK001",
                "task",
                "task payload exceeds stdin budget",
            ));
        }
        let adapter = SnapshotAdapter::new()?;
        let references = adapter.request_files(configuration.request)?;
        for (reference, bytes) in [
            (&references.lock, configuration.lock),
            (&references.profile, configuration.profile),
        ] {
            if reference.size != bytes.len() as u64 || reference.digest != Digest::of_bytes(bytes) {
                return Err(invalid(
                    "FILE004",
                    reference.path.as_str(),
                    "configuration bytes differ from the read request",
                ));
            }
        }
        let (prepared, _) = adapter.prepare(configuration)?;
        let contract = adapter.check(&prepared, snapshot)?;
        let document = adapter.decode(DocumentKind::BuildTask, task)?;
        let value = document.value();
        if value["task_digest"].as_str()
            != Some(
                crate::task_digest(&document)
                    .map_err(format_error)?
                    .as_str(),
            )
        {
            return Err(invalid(
                "PLAN002",
                "task_digest",
                "task content digest differs",
            ));
        }
        if value["selector"] != prepared.request.value()["selector"] {
            return Err(invalid(
                "PLAN002",
                "selector",
                "task targets a different resolved profile",
            ));
        }
        if value["input_digest"].as_str() != Some(contract.source_digest.as_str()) {
            return Err(invalid(
                "TASK001",
                "input_digest",
                "task source lineage differs from the checked contract",
            ));
        }
        if !value["dependencies"]
            .as_array()
            .expect("validated")
            .is_empty()
            || value["working_directory"] != "work"
        {
            return Err(invalid(
                "TASK001",
                "task",
                "single-task recipes require no dependencies and work/",
            ));
        }
        bind_tool(&prepared, &document, &installed_tool)?;
        let invocation = TaskInvocation {
            id: Id::new(value["id"].as_str().expect("validated"))
                .map_err(|e| invalid("TASK001", "id", e.to_string()))?,
            process: ProcessRequest {
                tool: installed_tool,
                working_directory: logical_path("work")?,
                arguments: value["arguments"]
                    .as_array()
                    .expect("validated")
                    .iter()
                    .map(|v| v.as_str().expect("validated").to_owned())
                    .collect(),
                environment: environment(&document)?,
                // Preserve the exact checked bytes, including formatting.
                stdin: task.to_vec(),
            },
            inputs: value["inputs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(reference)
                .collect::<Result<_>>()?,
            outputs: value["outputs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(|v| logical_path(v.as_str().expect("validated")))
                .collect::<Result<_>>()?,
        };
        invocation.validate()?;
        Ok(Self {
            adapter,
            task: document,
            invocation,
        })
    }
}

impl BuildRecipe for BoundBuildTask {
    fn invocation(&self) -> &TaskInvocation {
        &self.invocation
    }
    /// A helper's passed status only permits host byte verification. Actual
    /// exit, cleanup, cancellation and output reads remain application duties.
    fn validate_result(&self, stdout: &[u8]) -> Result<TaskFileReferences> {
        let evidence = self.adapter.decode(DocumentKind::BuildEvidence, stdout)?;
        validate_port_pair(&self.task, &evidence).map_err(format_error)?;
        let value = evidence.value();
        if value["status"] != "passed" || value["exit_code"] != 0 {
            return Err(invalid(
                "PORT004",
                "status",
                "build evidence did not report passed with exit zero",
            ));
        }
        Ok(TaskFileReferences {
            outputs: value["outputs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(reference)
                .collect::<Result<_>>()?,
            logs: value["logs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(reference)
                .collect::<Result<_>>()?,
        })
    }
}

fn bind_tool(prepared: &PreparedQuery, task: &Document, installed: &FileReference) -> Result<()> {
    let component = &task.value()["tool"];
    let tool = prepared.profile.value()["tools"]
        .as_array()
        .expect("validated")
        .iter()
        .find(|tool| {
            ["id", "version", "sha256"]
                .iter()
                .all(|field| tool[field] == component[field])
        })
        .ok_or_else(|| {
            invalid(
                "LOCK004",
                "tool",
                "task tool is not an image in the selected profile",
            )
        })?;
    if !prepared.lock.value()["tools"]
        .as_array()
        .expect("validated")
        .contains(tool)
    {
        return Err(invalid(
            "LOCK004",
            "tool",
            "selected tool image is not exactly locked",
        ));
    }
    if installed.size == 0
        || installed.size > MAX_TOOL_BYTES
        || component["sha256"].as_str() != Some(installed.digest.as_str())
    {
        return Err(invalid(
            "LOCK004",
            "installed_tool",
            "installed image identity or size is invalid",
        ));
    }
    Ok(())
}

fn environment(task: &Document) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for pair in task.value()["environment"].as_array().expect("validated") {
        let name = pair["name"].as_str().expect("validated");
        let value = pair["value"].as_str().expect("validated");
        if values.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(invalid(
                "TASK001",
                "environment",
                "duplicate environment name",
            ));
        }
    }
    Ok(values)
}
fn logical_path(path: &str) -> Result<LogicalPath> {
    LogicalPath::new(path).map_err(|e| invalid("TASK001", path, e.to_string()))
}
fn invalid(code: &'static str, subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}
