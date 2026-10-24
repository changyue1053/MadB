//! Decode execution bundles and retain their semantic recipe identities.
use super::{
    digest,
    files::{check_bytes, file_value, reference_schema, references},
    id, invalid,
};
use crate::{
    Document, DocumentKind, FormatError, SchemaRegistry,
    query::{format_error, reference},
};
use madb_app::{
    Digest, Id, LogicalPath, Result,
    ports::{ConfigurationBytes, FileReference},
};
use madb_protocol::{DigestDomain, Value, semantic_digest};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionOperation {
    Metadata,
    Generate,
    Build,
    Package,
}
#[derive(Clone, Debug)]
pub struct ExecutionSpec {
    pub operation: ExecutionOperation,
    pub context: ExecutionContext,
    pub recipe: Option<ExecutionRecipe>,
    pub tasks: Vec<ExecutionTask>,
    pub store: LogicalPath,
    pub run_parent: LogicalPath,
    pub cache: Option<LogicalPath>,
    pub watch: ExecutionWatch,
    pub dependencies: Option<DependencyFiles>,
}
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub project: FileReference,
    pub read_request: FileReference,
    pub lock: FileReference,
    pub profile: FileReference,
    pub contract: Option<FileReference>,
}
#[derive(Clone, Debug)]
pub struct ExecutionRecipe {
    pub id: Id,
    pub request: FileReference,
    pub tool: ExecutionTool,
    pub installed_tool: FileReference,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: LogicalPath,
    pub response_path: LogicalPath,
    pub inputs: Vec<FileReference>,
    /// Wire schema identities are retained for the complete generation projection.
    pub input_schemas: BTreeMap<LogicalPath, Id>,
}
#[derive(Clone, Debug)]
pub struct ExecutionTool {
    pub id: Id,
    pub version: String,
    pub digest: Digest,
}
#[derive(Clone, Debug)]
pub struct ExecutionTask {
    pub request: FileReference,
    pub installed_tool: FileReference,
}
#[derive(Clone, Debug)]
pub struct ExecutionWatch {
    pub absent: Vec<LogicalPath>,
    pub directories: Vec<LogicalPath>,
}
#[derive(Clone, Debug)]
pub struct DependencyFiles {
    pub request: FileReference,
    pub receipt: Option<FileReference>,
}

/// Decode strictly, reject duplicate set identities, and expose only owned domain values.
pub fn read_execution(bytes: &[u8]) -> Result<ExecutionSpec> {
    let document = SchemaRegistry::new()
        .map_err(format_error)?
        .decode(DocumentKind::Execution, bytes)
        .map_err(format_error)?;
    let v = document.value();
    let operation = match v["operation"].as_str().expect("schema checked") {
        "metadata" => ExecutionOperation::Metadata,
        "generate" => ExecutionOperation::Generate,
        "build" => ExecutionOperation::Build,
        "package" => ExecutionOperation::Package,
        _ => unreachable!("schema checked"),
    };
    let c = &v["context"];
    for (field, kind) in [
        ("project", DocumentKind::Project),
        ("read_request", DocumentKind::ReadRequest),
        ("lock", DocumentKind::Lock),
        ("profile", DocumentKind::Profile),
    ] {
        reference_schema(&c[field], kind)?;
    }
    if !c["contract"].is_null() {
        reference_schema(&c["contract"], DocumentKind::ContractIr)?;
    }
    if !v["recipe"].is_null() {
        reference_schema(
            &v["recipe"]["request"],
            match operation {
                ExecutionOperation::Metadata => DocumentKind::ReadRequest,
                ExecutionOperation::Generate => DocumentKind::GenerateRequest,
                ExecutionOperation::Package => DocumentKind::PackageRequest,
                ExecutionOperation::Build => unreachable!("schema checked"),
            },
        )?;
    }
    for task in v["tasks"].as_array().expect("schema checked") {
        reference_schema(&task["request"], DocumentKind::BuildTask)?;
    }
    let context = ExecutionContext {
        project: reference(&c["project"])?,
        read_request: reference(&c["read_request"])?,
        lock: reference(&c["lock"])?,
        profile: reference(&c["profile"])?,
        contract: optional_reference(&c["contract"])?,
    };
    let recipe = (!v["recipe"].is_null())
        .then(|| recipe_from_value(&v["recipe"]))
        .transpose()?;
    let tasks = v["tasks"]
        .as_array()
        .expect("schema checked")
        .iter()
        .map(|t| {
            Ok(ExecutionTask {
                request: reference(&t["request"])?,
                installed_tool: reference(&t["installed_tool"])?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut seen = BTreeSet::new();
    for task in &tasks {
        if !seen.insert(&task.request.path) {
            return Err(invalid("EXECUTION001", "tasks", "duplicate task request"));
        }
    }
    let dependencies = (!v["dependencies"].is_null())
        .then(|| {
            Ok(DependencyFiles {
                request: reference(&v["dependencies"]["request"])?,
                receipt: optional_reference(&v["dependencies"]["receipt"])?,
            })
        })
        .transpose()?;
    Ok(ExecutionSpec {
        operation,
        context,
        recipe,
        tasks,
        store: path(&v["store"])?,
        run_parent: path(&v["run_parent"])?,
        cache: (!v["cache"].is_null())
            .then(|| path(&v["cache"]))
            .transpose()?,
        watch: ExecutionWatch {
            absent: paths(&v["watch"]["absent"])?,
            directories: paths(&v["watch"]["directories"])?,
        },
        dependencies,
    })
}
impl ExecutionContext {
    /// Check the bundle's physical references in addition to read-request semantic checks.
    pub fn validate(
        &self,
        configuration: ConfigurationBytes<'_>,
        snapshot: Option<&[u8]>,
    ) -> Result<()> {
        for (file, bytes) in [
            (&self.project, configuration.project),
            (&self.read_request, configuration.request),
            (&self.lock, configuration.lock),
            (&self.profile, configuration.profile),
        ] {
            check_bytes(file, bytes)?;
        }
        match (&self.contract, snapshot) {
            (Some(file), Some(bytes)) => check_bytes(file, bytes),
            (None, None) => Ok(()),
            _ => Err(invalid(
                "EXECUTION001",
                "contract",
                "snapshot presence differs from execution context",
            )),
        }
    }
}
fn recipe_from_value(v: &Value) -> Result<ExecutionRecipe> {
    let mut environment = BTreeMap::new();
    for p in v["environment"].as_array().expect("schema checked") {
        let name = p["name"].as_str().expect("schema checked");
        if environment
            .insert(
                name.to_owned(),
                p["value"].as_str().expect("schema checked").to_owned(),
            )
            .is_some()
        {
            return Err(invalid(
                "EXECUTION001",
                "environment",
                "duplicate environment name",
            ));
        }
    }
    let recipe = ExecutionRecipe {
        id: id(&v["id"])?,
        request: reference(&v["request"])?,
        tool: ExecutionTool {
            id: id(&v["tool"]["id"])?,
            version: v["tool"]["version"]
                .as_str()
                .expect("schema checked")
                .to_owned(),
            digest: digest(&v["tool"]["sha256"])?,
        },
        installed_tool: reference(&v["installed_tool"])?,
        arguments: v["arguments"]
            .as_array()
            .expect("schema checked")
            .iter()
            .map(|s| s.as_str().expect("schema checked").to_owned())
            .collect(),
        environment,
        working_directory: path(&v["working_directory"])?,
        response_path: path(&v["response_path"])?,
        inputs: references(&v["inputs"])?,
        input_schemas: v["inputs"]
            .as_array()
            .expect("schema checked")
            .iter()
            .map(|file| Ok((path(&file["path"])?, id(&file["schema"])?)))
            .collect::<Result<_>>()?,
    };
    if recipe.inputs.iter().any(|f| f.path == recipe.request.path) {
        return Err(invalid(
            "EXECUTION001",
            "inputs",
            "request is stdin and must not also be a recipe input",
        ));
    }
    Ok(recipe)
}
impl ExecutionRecipe {
    pub(super) fn projection(&self) -> Result<Value> {
        let inputs = self
            .inputs
            .iter()
            .map(|file| {
                let mut value = file_value(file);
                value["schema"] = json!(
                    self.input_schemas
                        .get(&file.path)
                        .ok_or_else(|| invalid(
                            "EXECUTION001",
                            "inputs",
                            "input schema identity missing"
                        ))?
                        .as_str()
                );
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(
            json!({"id":self.id.as_str(),"tool":{"id":self.tool.id.as_str(),"version":self.tool.version,"sha256":self.tool.digest.as_str()},
            "arguments":self.arguments,"environment":self.environment.iter().map(|(name,value)| json!({"name":name,"value":value})).collect::<Vec<_>>(),
            "working_directory":self.working_directory.as_str(),"response_path":self.response_path.as_str(),
            "inputs":inputs}),
        )
    }
}

/// Generation has an explicit operation-tagged task projection. Request location
/// and installed image location are excluded; their content identities are checked separately.
pub fn generation_task_digest(
    request: &Document,
    recipe: &Value,
) -> std::result::Result<String, FormatError> {
    if request.kind() != DocumentKind::GenerateRequest {
        return Err(FormatError::new(
            "FMT003",
            "schema",
            "expected generation request",
        ));
    }
    let mut request = request.value().clone();
    request
        .as_object_mut()
        .expect("validated document")
        .remove("task_digest");
    request["inputs"] = sorted(&request["inputs"], "path")?;
    let mut execution = serde_json::Map::new();
    for field in [
        "id",
        "tool",
        "arguments",
        "working_directory",
        "response_path",
    ] {
        execution.insert(
            field.into(),
            recipe
                .get(field)
                .cloned()
                .ok_or_else(|| FormatError::new("HASH004", field, "missing recipe field"))?,
        );
    }
    execution.insert(
        "environment".into(),
        sorted(&recipe["environment"], "name")?,
    );
    execution.insert("inputs".into(), sorted(&recipe["inputs"], "path")?);
    semantic_digest(
        DigestDomain::Task,
        &json!({"operation":"generate","request":request,"execution":execution}),
    )
}
fn sorted(value: &Value, key: &str) -> std::result::Result<Value, FormatError> {
    let mut values = value
        .as_array()
        .ok_or_else(|| FormatError::new("HASH004", key, "expected set"))?
        .clone();
    let mut seen = BTreeSet::new();
    for value in &values {
        let name = value[key]
            .as_str()
            .ok_or_else(|| FormatError::new("HASH004", key, "missing set identity"))?;
        if !seen.insert(name) {
            return Err(FormatError::new("HASH004", key, "duplicate set identity"));
        }
    }
    values.sort_by(|a, b| {
        a[key]
            .as_str()
            .expect("checked")
            .encode_utf16()
            .cmp(b[key].as_str().expect("checked").encode_utf16())
    });
    Ok(Value::Array(values))
}

fn optional_reference(value: &Value) -> Result<Option<FileReference>> {
    (!value.is_null()).then(|| reference(value)).transpose()
}
fn path(value: &Value) -> Result<LogicalPath> {
    LogicalPath::new(
        value
            .as_str()
            .ok_or_else(|| invalid("DTO001", "path", "expected path"))?,
    )
    .map_err(|e| invalid("FILE001", "path", e.to_string()))
}
fn paths(value: &Value) -> Result<Vec<LogicalPath>> {
    let result = value
        .as_array()
        .expect("schema checked")
        .iter()
        .map(path)
        .collect::<Result<Vec<_>>>()?;
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(invalid("EXECUTION001", "watch", "duplicate watched path"));
    }
    Ok(result)
}
