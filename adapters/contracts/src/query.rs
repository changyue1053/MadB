//! The read-only application adapter; parsing never opens files or invokes tools.
use crate::wire::Wire;
use crate::{ContractError, Document, DocumentKind, FormatError, SchemaRegistry};
use madb_app::ports::{ConfigurationBytes, FileReference, RequestFiles, SnapshotFormat};
use madb_app::{ErrorKind, Id, LogicalPath, Phase, QueryError, Result, Task, TaskGraph};
use madb_model::contract::Contract;
use std::collections::BTreeMap;

pub struct SnapshotAdapter {
    registry: SchemaRegistry,
}
pub struct PreparedQuery {
    pub(crate) request: Document,
    pub(crate) project: Document,
    pub(crate) lock: Document,
    pub(crate) profile: Document,
}

impl SnapshotAdapter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry: SchemaRegistry::new().map_err(format_error)?,
        })
    }
    pub(crate) fn decode(&self, kind: DocumentKind, bytes: &[u8]) -> Result<Document> {
        self.registry.decode(kind, bytes).map_err(format_error)
    }
}

impl SnapshotFormat for SnapshotAdapter {
    type Prepared = PreparedQuery;
    fn request_files(&self, bytes: &[u8]) -> Result<RequestFiles> {
        let request = self.decode(DocumentKind::ReadRequest, bytes)?;
        Ok(RequestFiles {
            lock: reference(&request.value()["lock"])?,
            profile: reference(&request.value()["profile"])?,
        })
    }
    fn prepare(
        &self,
        bytes: ConfigurationBytes<'_>,
    ) -> Result<(Self::Prepared, Vec<FileReference>)> {
        let request = self.decode(DocumentKind::ReadRequest, bytes.request)?;
        let project = if bytes.project.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            self.decode(DocumentKind::Project, bytes.project)?
        } else {
            let text = std::str::from_utf8(bytes.project)
                .map_err(|e| invalid("FMT001", "project", e.to_string()))?;
            self.registry
                .decode_project_toml(text)
                .map_err(format_error)?
        };
        let lock = self.decode(DocumentKind::Lock, bytes.lock)?;
        let profile = self.decode(DocumentKind::Profile, bytes.profile)?;
        let q = request.value();
        let l = lock.value();
        let p = profile.value();
        if q["lock"]["schema"] != DocumentKind::Lock.schema_id()
            || q["profile"]["schema"] != DocumentKind::Profile.schema_id()
        {
            return Err(invalid(
                "LOCK001",
                "request",
                "request references have wrong schema identities",
            ));
        }
        if l["project_digest"].as_str()
            != Some(
                crate::project_digest(&project)
                    .map_err(format_error)?
                    .as_str(),
            )
        {
            return Err(invalid(
                "LOCK002",
                "project",
                "project differs from the lock; explicit preparation is required",
            ));
        }
        if q["selector"]["profile_id"] != p["id"]
            || q["selector"]["profile_digest"].as_str()
                != Some(
                    crate::profile_digest(&profile)
                        .map_err(format_error)?
                        .as_str(),
                )
            || ["route", "target", "direction"]
                .iter()
                .any(|key| q["selector"][key] != p[key])
        {
            return Err(invalid(
                "LOCK001",
                "profile",
                "read request and profile differ",
            ));
        }
        if !l["components"]
            .as_array()
            .expect("validated")
            .contains(&q["component"])
        {
            return Err(invalid(
                "LOCK004",
                "component",
                "reader component is not exactly locked",
            ));
        }
        if crate::source_digest(&q["inputs"]).map_err(format_error)?
            != crate::source_digest(&l["inputs"]).map_err(format_error)?
        {
            return Err(invalid(
                "LOCK005",
                "inputs",
                "request and lock track different source inventories",
            ));
        }
        let mut files = BTreeMap::<LogicalPath, FileReference>::new();
        for item in l["inputs"]
            .as_array()
            .expect("validated")
            .iter()
            .chain(l["schemas"].as_array().expect("validated"))
        {
            let reference = reference(item)?;
            if files.insert(reference.path.clone(), reference).is_some() {
                return Err(invalid(
                    "LOCK006",
                    "inputs/schemas",
                    "duplicate tracked path",
                ));
            }
        }
        // Cross-document profile/tool/boundary checks run with the snapshot in
        // check_frozen_inputs. No missing snapshot can be treated as checked.
        Ok((
            PreparedQuery {
                request,
                project,
                lock,
                profile,
            },
            files.into_values().collect(),
        ))
    }
    fn check(&self, prepared: &PreparedQuery, bytes: &[u8]) -> Result<Contract> {
        let snapshot = self.decode(DocumentKind::ContractIr, bytes)?;
        crate::artifacts::validate_port_pair(&prepared.request, &snapshot).map_err(format_error)?;
        crate::check_frozen_inputs(
            &snapshot,
            &prepared.project,
            &prepared.profile,
            &prepared.lock,
        )
        .map_err(format_error)?;
        // Ensure referenced profile bytes are the same reference used by the
        // project and lock, not merely a document with an equal semantic hash.
        let profile_id = &prepared.profile.value()["id"];
        let locked = prepared.lock.value()["profiles"]
            .as_array()
            .expect("validated")
            .iter()
            .find(|p| &p["id"] == profile_id)
            .expect("checked frozen profile");
        if locked["file"] != prepared.request.value()["profile"] {
            return Err(invalid(
                "LOCK003",
                "profile",
                "request and lock reference different profile bytes",
            ));
        }
        let model = crate::check_contract(&snapshot, &prepared.profile).map_err(contract_error)?;
        // Reader annotation channels require a real reader (C05). Never copy
        // alleged source annotations from the IR back into authoritative input.
        crate::check_policy_resolution(&snapshot, &prepared.project, &prepared.profile, &[])
            .map_err(contract_error)?;
        Ok(model)
    }
    fn task_graph(&self, prepared: &PreparedQuery, bytes: &[Vec<u8>]) -> Result<TaskGraph> {
        let mut tasks = Vec::new();
        for bytes in bytes {
            let task = self.decode(DocumentKind::BuildTask, bytes)?;
            let v = task.value();
            if v["task_digest"].as_str()
                != Some(crate::task_digest(&task).map_err(format_error)?.as_str())
            {
                return Err(invalid(
                    "PLAN002",
                    "task_digest",
                    "task content digest differs",
                ));
            }
            if v["selector"] != prepared.request.value()["selector"] {
                return Err(invalid(
                    "PLAN002",
                    "selector",
                    "task targets a different resolved profile",
                ));
            }
            let component = &v["tool"];
            let l = prepared.lock.value();
            let locked = l["components"]
                .as_array()
                .expect("validated")
                .contains(component)
                || prepared.profile.value()["tools"]
                    .as_array()
                    .expect("validated")
                    .iter()
                    .any(|tool| {
                        ["id", "version", "sha256"]
                            .iter()
                            .all(|field| tool[field] == component[field])
                    });
            if !locked {
                return Err(invalid(
                    "LOCK004",
                    "tool",
                    "task tool is not exactly locked",
                ));
            }
            let phase = match v["phase"].as_str().expect("validated") {
                "metadata" => Phase::Metadata,
                "generate" => Phase::Generate,
                "compile" => Phase::Compile,
                "link" => Phase::Link,
                "run" => Phase::Run,
                _ => unreachable!("schema phases"),
            };
            let wire = Wire::root(&task, DocumentKind::BuildTask).map_err(format_error)?;
            let dependencies = wire
                .field("dependencies")
                .map_err(format_error)?
                .array()
                .map_err(format_error)?
                .into_iter()
                .map(|v| v.id().map_err(format_error))
                .collect::<Result<Vec<_>>>()?;
            let cwd = v["working_directory"].as_str().expect("validated");
            let outputs = v["outputs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(|p| {
                    LogicalPath::new(format!("{cwd}/{}", p.as_str().expect("validated")))
                        .map_err(|e| invalid("PLAN001", "outputs", e.to_string()))
                })
                .collect::<Result<Vec<_>>>()?;
            tasks.push(Task {
                id: Id::new(v["id"].as_str().expect("validated"))
                    .map_err(|e| invalid("PLAN001", "id", e.to_string()))?,
                phase,
                dependencies,
                outputs,
            });
        }
        TaskGraph::new(tasks)
            .map_err(|e| QueryError::new(ErrorKind::CheckFailed, "PLAN001", "tasks", e.to_string()))
    }
}

pub(crate) fn reference(value: &madb_protocol::Value) -> Result<FileReference> {
    let wire = Wire::at(value, "reference");
    Ok(FileReference {
        path: LogicalPath::new(
            wire.field("path")
                .map_err(format_error)?
                .string()
                .map_err(format_error)?,
        )
        .map_err(|e| invalid("FILE001", "path", e.to_string()))?,
        digest: wire
            .field("sha256")
            .map_err(format_error)?
            .digest()
            .map_err(format_error)?,
        size: wire
            .field("size")
            .map_err(format_error)?
            .uint()
            .map_err(format_error)?,
    })
}
fn invalid(code: &'static str, path: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, path, message)
}
pub(crate) fn format_error(error: FormatError) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        error.code,
        error.path,
        error.message,
    )
}
fn contract_error(error: ContractError) -> QueryError {
    match error {
        ContractError::Format(e) => format_error(e),
        ContractError::Findings(findings) => QueryError {
            kind: ErrorKind::CheckFailed,
            findings,
        },
    }
}
