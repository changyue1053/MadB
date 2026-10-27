//! Explicit /2 reader proof. Results still require real process admission and
//! immutable host capture; this type never authorizes a Source commit.
use super::{
    BoundCompositionInputs, Result, admission::CheckedCompositionPins,
    check_composition_snapshot_binding, invalid, outputs::check_ir_identities,
};
use crate::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::{ProvidedFile, validate_port_pair, verify_reference},
    execution::ExecutionRecipe,
};
use madb_app::{
    Digest, Id, LogicalPath,
    inputs::{FrozenInputs, check_input_inventory},
    pipeline::{ArtifactDeclaration, ArtifactInvocation, ArtifactKind, ArtifactRecipe},
    ports::FileReference,
    process::ProcessRequest,
};
use madb_protocol::{DigestDomain, semantic_digest};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub struct CompositionReadRequest<'a> {
    pub project: &'a Document,
    pub recipe: ExecutionRecipe,
    pub pins: CheckedCompositionPins,
    pub producer: Id,
    pub output_root: LogicalPath,
    pub read_request_schema: LogicalPath,
    pub request: ProvidedFile<'a>,
    pub lock: ProvidedFile<'a>,
    pub profile: ProvidedFile<'a>,
    pub installed_image: ProvidedFile<'a>,
}
pub struct BoundCompositionRead {
    inputs: BoundCompositionInputs,
    project: Document,
    pins: CheckedCompositionPins,
    request: Document,
    invocation: ArtifactInvocation,
    output_root: LogicalPath,
    task_digest: Digest,
    request_path: LogicalPath,
}
impl BoundCompositionRead {
    pub fn new(
        inputs: BoundCompositionInputs,
        request: CompositionReadRequest<'_>,
    ) -> Result<Self> {
        let recipe = &request.recipe;
        if !request
            .pins
            .matches(&inputs, recipe, request.producer.as_str())
        {
            return Err(invalid(
                "ExtensionIdentityMismatch",
                "recipe",
                "reader differs from checked pins",
            ));
        }
        let component = request.pins.manifest().value()["components"]
            .as_array()
            .expect("checked manifest")
            .iter()
            .find(|c| c["id"] == request.pins.component_id())
            .expect("checked component");
        if component["port"] != "contract-reader" {
            return Err(invalid(
                "InputInvalid",
                "component",
                "reader requires contract-reader port",
            ));
        }
        let expected_features = inputs.profile().value()["required_features"]
            .as_array()
            .expect("checked features")
            .iter()
            .map(|f| f.as_str().expect("feature"))
            .collect::<BTreeSet<_>>();
        if request
            .pins
            .required_features()
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != expected_features
        {
            return Err(invalid(
                "CapabilityUnsupported",
                "features",
                "reader must negotiate fixed profile semantics",
            ));
        }
        let registry = SchemaRegistry::new()?;
        let schema_path = request.read_request_schema.as_str();
        let locked_schema = inputs.locked_document.value()["schemas"]
            .as_array()
            .expect("schema refs")
            .iter()
            .find(|f| f["path"] == schema_path)
            .ok_or_else(|| invalid("LOCK005", schema_path, "reader decoder not locked"))?;
        let (_, schema_bytes) = inputs
            .frozen_inputs
            .files()
            .find(|(f, _)| f.path.as_str() == schema_path)
            .ok_or_else(|| invalid("FILE003", schema_path, "reader decoder not frozen"))?;
        verify_reference(
            locked_schema,
            &ProvidedFile {
                path: schema_path,
                bytes: schema_bytes,
            },
        )?;
        if schema_bytes
            != include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/schemas/read-request.schema.json"
            ))
        {
            return Err(invalid(
                "FILE004",
                schema_path,
                "reader decoder differs from bundled schema",
            ));
        }
        let lock = registry.decode(DocumentKind::Lock, request.lock.bytes)?;
        if lock.value() != inputs.locked_document.value()
            || crate::project_digest(request.project)? != lock.value()["project_digest"]
        {
            return Err(invalid(
                "LOCK003",
                "context",
                "reader project or lock differs",
            ));
        }
        check_file(&inputs.profile_reference, &request.profile)?;
        check_file(&recipe.request, &request.request)?;
        check_file(&recipe.installed_tool, &request.installed_image)?;
        let document = registry.decode(DocumentKind::ReadRequest, request.request.bytes)?;
        let q = document.value();
        let p = inputs.profile().value();
        if q["boundary_id"].as_str() != Some(inputs.boundary_id())
            || q["selector"]
                != json!({"profile_id":p["id"],"profile_digest":inputs.profile_digest(),"route":p["route"],"direction":p["direction"],"target":p["target"]})
            || q["output_root"] != request.output_root.as_str()
        {
            return Err(invalid(
                "LOCK001",
                "selector",
                "reader selection or output domain differs",
            ));
        }
        for (key, schema, file) in [
            ("lock", "madb.lock/1", &request.lock),
            ("profile", "madb.profile/2", &request.profile),
        ] {
            if q[key]["schema"] != schema {
                return Err(invalid("FMT003", key, "reader context schema differs"));
            }
            verify_reference(&q[key], file)?;
        }
        let selected = inputs.composition().value()["components"]
            .as_array()
            .expect("components")
            .iter()
            .find(|c| c["id"] == request.producer.as_str())
            .ok_or_else(|| invalid("LOCK004", "component", "reader is not selected"))?;
        if &q["component"] != selected
            || crate::source_digest(&q["inputs"])? != inputs.source_digest()
        {
            return Err(invalid(
                "LOCK004",
                "request",
                "reader component or source closure differs",
            ));
        }
        let mut expected = inputs
            .frozen_inputs
            .references()
            .map(|f| (f.path.clone(), f.clone()))
            .collect::<BTreeMap<_, _>>();
        for file in [&request.lock, &request.profile] {
            let f = FileReference {
                path: logical(file.path)?,
                digest: Digest::of_bytes(file.bytes),
                size: file.bytes.len() as u64,
            };
            if let Some(old) = expected.insert(f.path.clone(), f.clone())
                && old != f
            {
                return Err(invalid(
                    "FILE004",
                    file.path,
                    "reader context aliases different frozen bytes",
                ));
            }
        }
        let actual = recipe
            .inputs
            .iter()
            .map(|f| (f.path.clone(), f.clone()))
            .collect::<BTreeMap<_, _>>();
        if actual != expected
            || actual.len() != recipe.inputs.len()
            || actual.contains_key(&recipe.request.path)
            || recipe.input_schemas.len() != actual.len()
            || recipe.input_schemas.keys().ne(actual.keys())
        {
            return Err(invalid(
                "LOCK005",
                "inputs",
                "reader recipe requires exact immutable input closure",
            ));
        }
        for reference in q["inputs"]
            .as_array()
            .expect("inputs")
            .iter()
            .chain([&q["lock"], &q["profile"]])
        {
            let path = logical(reference["path"].as_str().expect("path"))?;
            if recipe.input_schemas.get(&path).map(Id::as_str) != reference["schema"].as_str() {
                return Err(invalid(
                    "FMT003",
                    path.as_str(),
                    "reader input schema differs",
                ));
            }
        }
        let mut all = recipe.inputs.clone();
        all.push(recipe.request.clone());
        check_input_inventory(&all).map_err(query_error)?;
        all.push(FileReference {
            path: recipe.response_path.clone(),
            digest: Digest::of_bytes(b"reserved output"),
            size: 15,
        });
        check_input_inventory(&all).map_err(query_error)?;
        let task_digest = Digest::new(semantic_digest(DigestDomain::Task, &json!({"operation":"metadata","request":q,"execution":recipe.projection().map_err(query_error)?}))?).map_err(|e| invalid("HASH004", "task", e.to_string()))?;
        let checked = request.pins.process();
        let invocation = ArtifactInvocation {
            id: recipe.id.clone(),
            process: ProcessRequest {
                tool: checked.tool.clone(),
                arguments: checked.arguments.clone(),
                environment: checked.environment.clone(),
                working_directory: checked.working_directory.clone(),
                stdin: request.request.bytes.to_vec(),
            },
            inputs: recipe.inputs.clone(),
            response_path: recipe.response_path.clone(),
        };
        invocation.validate().map_err(query_error)?;
        Ok(Self {
            inputs,
            project: request.project.clone(),
            pins: request.pins,
            request: document,
            invocation,
            output_root: request.output_root,
            task_digest,
            request_path: recipe.request.path.clone(),
        })
    }
    pub fn pins(&self) -> &CheckedCompositionPins {
        &self.pins
    }
    pub fn output_root(&self) -> &LogicalPath {
        &self.output_root
    }
    pub fn request(&self) -> &Document {
        &self.request
    }
    pub fn check_session_inputs(&self, inputs: &FrozenInputs) -> Result<()> {
        let mut expected = self.invocation.inputs.clone();
        expected.push(FileReference {
            path: logical(self.request_path())?,
            digest: Digest::of_bytes(&self.invocation.process.stdin),
            size: self.invocation.process.stdin.len() as u64,
        });
        expected.sort_by(|a, b| a.path.cmp(&b.path));
        if expected.iter().ne(inputs.references()) {
            return Err(invalid(
                "InputInvalid",
                "inputs",
                "reader session input closure differs",
            ));
        }
        for (r, b) in inputs.files() {
            if Digest::of_bytes(b) != r.digest || b.len() as u64 != r.size {
                return Err(invalid(
                    "FILE004",
                    r.path.as_str(),
                    "reader input bytes differ",
                ));
            }
        }
        Ok(())
    }
    fn request_path(&self) -> &str {
        self.request_path.as_str()
    }
}
impl ArtifactRecipe for BoundCompositionRead {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }
    fn validate_result(&self, bytes: &[u8]) -> madb_app::Result<ArtifactDeclaration> {
        let checked = || -> Result<()> {
            let ir = SchemaRegistry::new()?.decode(DocumentKind::ContractIr, bytes)?;
            validate_port_pair(&self.request, &ir)?;
            check_composition_snapshot_binding(&self.inputs, &ir)?;
            check_ir_identities(&ir)?;
            let capability = crate::convert::composition_capability(&self.inputs)?;
            crate::check_contract_with_capability(&ir, &capability)
                .map_err(|e| invalid("InputInvalid", "contract", e.to_string()))?;
            let (defaults, origin) = crate::convert::composition_profile_defaults(&self.inputs)?;
            crate::check_policy_resolution_with_defaults(
                &ir,
                &self.project,
                &defaults,
                &origin,
                &[],
            )
            .map_err(|e| invalid("InputInvalid", "policy", e.to_string()))?;
            if ir.value()["required_features"]
                .as_array()
                .expect("features")
                .iter()
                .any(|f| !self.pins.required_features().iter().any(|v| f == v))
            {
                return Err(invalid(
                    "CapabilityUnsupported",
                    "features",
                    "reader emitted unnegotiated semantics",
                ));
            }
            Ok(())
        };
        checked().map_err(|e| {
            madb_app::QueryError::new(madb_app::ErrorKind::InvalidInput, e.code, e.path, e.message)
        })?;
        Ok(ArtifactDeclaration {
            kind: ArtifactKind::Metadata,
            identity: Digest::of_bytes(bytes),
            input_digest: Digest::new(self.inputs.source_digest()).expect("checked source"),
            task_digest: self.task_digest.clone(),
            profile_digest: Digest::new(self.inputs.profile_digest()).expect("checked profile"),
            generation_id: None,
            files: vec![],
        })
    }
}
fn check_file(r: &FileReference, file: &ProvidedFile<'_>) -> Result<()> {
    if r.path.as_str() != file.path {
        return Err(invalid("FILE003", file.path, "reader path differs"));
    }
    if r.digest != Digest::of_bytes(file.bytes) || r.size != file.bytes.len() as u64 {
        return Err(invalid("FILE004", file.path, "reader bytes differ"));
    }
    Ok(())
}
fn logical(s: &str) -> Result<LogicalPath> {
    LogicalPath::new(s).map_err(|e| invalid("FILE001", s, e.to_string()))
}
fn query_error(e: madb_app::QueryError) -> crate::FormatError {
    invalid("InputInvalid", "reader", e.to_string())
}
