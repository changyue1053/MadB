//! Bind declared recipes to one checked configuration and result payloads.
//! This adapter has no filesystem or process access and does not certify a batch.
use crate::{
    Document, DocumentKind,
    artifacts::validate_port_pair,
    query::{PreparedQuery, SnapshotAdapter, format_error, reference},
};
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, Phase, QueryError, Result,
    dag::{ExecutionGraph, GraphRecipe},
    ports::{ConfigurationBytes, FileReference, SnapshotFormat},
    process::{MAX_STDIN_BYTES, MAX_TOOL_BYTES, ProcessRequest},
    task::{BuildRecipe, TaskFileReferences, TaskInvocation},
};
use std::{collections::BTreeMap, sync::Arc};

const MAX_GRAPH_TASKS: usize = 4096;
const MAX_GRAPH_TASK_BYTES: usize = 16 * 1024 * 1024;

/// A fully checked configuration/task binding. Only the host-supplied installed
/// image location becomes executable; task component identifiers are not paths.
pub struct BoundBuildTask {
    adapter: Arc<SnapshotAdapter>,
    task: Document,
    invocation: TaskInvocation,
    dependencies: Vec<Id>,
    phase: Phase,
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
        check_task_size(task)?;
        let context = CheckedContext::new(configuration, snapshot)?;
        let bound = context.bind(task, installed_tool)?;
        if !bound.dependencies.is_empty() {
            return Err(invalid(
                "TASK001",
                "dependencies",
                "single-task recipes require no dependencies",
            ));
        }
        Ok(bound)
    }
}

/// One task document and its host-resolved, locked executable image.
/// The bytes and image are checked together before any recipe can execute.
pub struct TaskBinding<'a> {
    pub task: &'a [u8],
    pub installed_tool: FileReference,
}

/// Bind the complete graph against one configuration, then select a target.
///
/// Even unselected nodes must pass binding and full graph validation. Their
/// source files are not read here. Known input hashes are preserved; this does
/// not discover output identities, certify a generation or execute any task.
pub fn bind_graph(
    configuration: ConfigurationBytes<'_>,
    snapshot: &[u8],
    tasks: Vec<TaskBinding<'_>>,
    target: Option<&Id>,
    active: &[Id],
) -> Result<ExecutionGraph<BoundBuildTask>> {
    if tasks.is_empty() || tasks.len() > MAX_GRAPH_TASKS {
        return Err(invalid(
            "TASK001",
            "graph",
            "graph binding size out of range",
        ));
    }
    let mut bytes = 0usize;
    for binding in &tasks {
        check_task_size(binding.task)?;
        // The count and per-task limit above also bound this addition.
        bytes += binding.task.len();
        if bytes > MAX_GRAPH_TASK_BYTES {
            return Err(invalid(
                "TASK001",
                "graph",
                "task documents exceed graph binding byte budget",
            ));
        }
    }
    let context = CheckedContext::new(configuration, snapshot)?;
    let recipes = tasks
        .into_iter()
        .map(|binding| context.bind(binding.task, binding.installed_tool))
        .collect::<Result<Vec<_>>>()?;
    ExecutionGraph::new(recipes, target, active)
}

/// Configuration and schema compilation are shared across the whole binding;
/// each retained recipe needs only the registry, its own document and invocation.
struct CheckedContext {
    adapter: Arc<SnapshotAdapter>,
    prepared: PreparedQuery,
    source_digest: Digest,
}
impl CheckedContext {
    fn new(configuration: ConfigurationBytes<'_>, snapshot: &[u8]) -> Result<Self> {
        let adapter = Arc::new(SnapshotAdapter::new()?);
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
        Ok(Self {
            adapter,
            prepared,
            source_digest: contract.source_digest,
        })
    }

    fn bind(&self, task: &[u8], installed_tool: FileReference) -> Result<BoundBuildTask> {
        let adapter = &self.adapter;
        let prepared = &self.prepared;
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
        if value["input_digest"].as_str() != Some(self.source_digest.as_str()) {
            return Err(invalid(
                "TASK001",
                "input_digest",
                "task source lineage differs from the checked contract",
            ));
        }
        if value["working_directory"] != "work" {
            return Err(invalid(
                "TASK001",
                "working_directory",
                "task recipes require work/",
            ));
        }
        bind_tool(prepared, &document, &installed_tool)?;
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
        let dependencies = value["dependencies"]
            .as_array()
            .expect("validated")
            .iter()
            .map(|id| {
                Id::new(id.as_str().expect("validated"))
                    .map_err(|e| invalid("TASK001", "dependencies", e.to_string()))
            })
            .collect::<Result<_>>()?;
        let phase = match value["phase"].as_str().expect("validated") {
            "metadata" => Phase::Metadata,
            "generate" => Phase::Generate,
            "compile" => Phase::Compile,
            "link" => Phase::Link,
            "run" => Phase::Run,
            _ => unreachable!("schema validated phase"),
        };
        Ok(BoundBuildTask {
            adapter: Arc::clone(adapter),
            task: document,
            invocation,
            dependencies,
            phase,
        })
    }
}

impl BuildRecipe for BoundBuildTask {
    fn invocation(&self) -> &TaskInvocation {
        &self.invocation
    }
    fn dependencies(&self) -> &[Id] {
        &self.dependencies
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

impl GraphRecipe for BoundBuildTask {
    fn phase(&self) -> Phase {
        self.phase
    }
}

fn check_task_size(task: &[u8]) -> Result<()> {
    if task.len() > MAX_STDIN_BYTES {
        return Err(invalid(
            "TASK001",
            "task",
            "task payload exceeds stdin budget",
        ));
    }
    Ok(())
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
