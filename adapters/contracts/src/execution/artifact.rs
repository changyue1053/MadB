//! Bind artifact requests and validate their declared results.
use super::{
    dependencies::bind_tool,
    digest,
    files::{check_bytes, declared_file, references, require_file},
    invalid,
    package::validate_package,
    spec::{ExecutionOperation, ExecutionRecipe, generation_task_digest},
};
use crate::{
    Document, DocumentKind,
    artifacts::validate_port_pair,
    query::{PreparedQuery, SnapshotAdapter, format_error, reference},
};
use madb_app::{
    Digest, Result,
    inputs::FrozenInputs,
    pipeline::{ArtifactDeclaration, ArtifactInvocation, ArtifactKind, ArtifactRecipe},
    ports::{ConfigurationBytes, SnapshotFormat},
    process::{MAX_STDIN_BYTES, ProcessRequest},
};
use madb_protocol::{DigestDomain, semantic_digest};
use serde_json::json;
use std::collections::BTreeSet;

pub struct BoundArtifactRecipe {
    adapter: SnapshotAdapter,
    prepared: PreparedQuery,
    request: Document,
    invocation: ArtifactInvocation,
    operation: ExecutionOperation,
    input_digest: Digest,
    task_digest: Digest,
    profile_digest: Digest,
    symbols: BTreeSet<String>,
}
impl BoundArtifactRecipe {
    /// Bind real port requests against checked configuration and immutable supplied bytes.
    pub fn new(
        configuration: ConfigurationBytes<'_>,
        snapshot: Option<&[u8]>,
        recipe: &ExecutionRecipe,
        request: &[u8],
        provided: &FrozenInputs,
    ) -> Result<Self> {
        if request.len() > MAX_STDIN_BYTES {
            return Err(invalid(
                "EXECUTION001",
                "request",
                "request exceeds stdin budget",
            ));
        }
        check_bytes(&recipe.request, request)?;
        if recipe
            .inputs
            .iter()
            .any(|file| file.path == recipe.request.path)
        {
            return Err(invalid(
                "EXECUTION001",
                "inputs",
                "request cannot also be an input",
            ));
        }
        let adapter = SnapshotAdapter::new()?;
        let raw = madb_protocol::strict_json(request).map_err(format_error)?;
        let (operation, kind) = match raw["schema"].as_str() {
            Some("madb.read-request/1") => {
                (ExecutionOperation::Metadata, DocumentKind::ReadRequest)
            }
            Some("madb.generate-request/1") => {
                (ExecutionOperation::Generate, DocumentKind::GenerateRequest)
            }
            Some("madb.package-request/1") => {
                (ExecutionOperation::Package, DocumentKind::PackageRequest)
            }
            _ => {
                return Err(invalid(
                    "EXECUTION001",
                    "request",
                    "unsupported artifact request",
                ));
            }
        };
        let request_doc = adapter.decode(kind, request)?;
        let request_files = adapter.request_files(configuration.request)?;
        check_bytes(&request_files.lock, configuration.lock)?;
        check_bytes(&request_files.profile, configuration.profile)?;
        let read_request_bytes = configuration.request;
        let (prepared, required) = adapter.prepare(configuration)?;
        for file in &required {
            declared_file(&recipe.inputs, file)?;
            require_file(provided, file)?;
        }
        for file in &recipe.inputs {
            require_file(provided, file)?;
        }
        bind_tool(&prepared, &recipe.tool, &recipe.installed_tool)?;
        let q = request_doc.value();
        let read = prepared.request.value();
        if q["selector"] != read["selector"] {
            return Err(invalid(
                "LOCK001",
                "selector",
                "artifact request targets another profile",
            ));
        }
        if q["lock"] != read["lock"] {
            return Err(invalid(
                "LOCK003",
                "lock",
                "artifact request uses different lock bytes",
            ));
        }
        let profile_digest = digest(&read["selector"]["profile_digest"])?;
        let input_digest =
            Digest::new(crate::source_digest(&read["inputs"]).map_err(format_error)?)
                .map_err(|e| invalid("HASH004", "input_digest", e.to_string()))?;
        let mut symbols = BTreeSet::new();
        if operation == ExecutionOperation::Metadata {
            if request != read_request_bytes || snapshot.is_some() {
                return Err(invalid(
                    "EXECUTION001",
                    "request",
                    "metadata must execute the exact context read request without a snapshot",
                ));
            }
        } else {
            let snapshot = snapshot
                .ok_or_else(|| invalid("EXECUTION001", "contract", "checked snapshot required"))?;
            let contract = adapter.check(&prepared, snapshot)?;
            symbols = contract
                .symbols
                .iter()
                .map(|s| s.public_id.as_str().to_owned())
                .collect();
            if q["contract_digest"].as_str() != Some(contract.structure_digest.as_str())
                || q["policy_digest"].as_str() != Some(contract.policy_digest.as_str())
            {
                return Err(invalid(
                    "EXECUTION002",
                    "contract_digest",
                    "artifact request differs from checked contract or policy",
                ));
            }
            if operation == ExecutionOperation::Generate {
                check_bytes(&reference(&q["contract"])?, snapshot)?;
                if q["contract"]["schema"] != "madb.contract-ir/1"
                    || q["profile"] != read["profile"]
                    || q["boundary_id"] != read["boundary_id"]
                    || q["input_digest"].as_str() != Some(input_digest.as_str())
                    || crate::source_digest(&q["inputs"]).map_err(format_error)?
                        != input_digest.as_str()
                {
                    return Err(invalid(
                        "EXECUTION002",
                        "request",
                        "generation context or source lineage differs",
                    ));
                }
                declared_file(&recipe.inputs, &reference(&q["contract"])?)?;
                require_file(provided, &reference(&q["contract"])?)?;
            } else {
                validate_package(
                    &adapter,
                    &prepared,
                    &request_doc,
                    provided,
                    &input_digest,
                    &symbols,
                    &recipe.inputs,
                )?;
            }
        }
        if kind != DocumentKind::PackageRequest
            && !prepared.lock.value()["components"]
                .as_array()
                .expect("validated")
                .contains(&q["component"])
        {
            return Err(invalid(
                "LOCK004",
                "component",
                "request component is not exactly locked",
            ));
        }
        let task_digest = if operation == ExecutionOperation::Generate {
            let computed = generation_task_digest(&request_doc, &recipe.projection()?)
                .map_err(format_error)?;
            if q["task_digest"].as_str() != Some(computed.as_str()) {
                return Err(invalid(
                    "PLAN002",
                    "task_digest",
                    "generation request/execution digest differs",
                ));
            }
            Digest::new(computed).map_err(|e| invalid("HASH004", "task_digest", e.to_string()))?
        } else {
            Digest::new(semantic_digest(DigestDomain::Task, &json!({"operation":if operation == ExecutionOperation::Metadata {"metadata"} else {"package"},"request":q,"execution":recipe.projection()?})).map_err(format_error)?)
                .map_err(|e| invalid("HASH004", "task_digest", e.to_string()))?
        };
        let invocation = ArtifactInvocation {
            id: recipe.id.clone(),
            process: ProcessRequest {
                tool: recipe.installed_tool.clone(),
                working_directory: recipe.working_directory.clone(),
                arguments: recipe.arguments.clone(),
                environment: recipe.environment.clone(),
                stdin: request.to_vec(),
            },
            inputs: recipe.inputs.clone(),
            response_path: recipe.response_path.clone(),
        };
        invocation.validate()?;
        Ok(Self {
            adapter,
            prepared,
            request: request_doc,
            invocation,
            operation,
            input_digest,
            task_digest,
            profile_digest,
            symbols,
        })
    }
    pub fn operation(&self) -> ExecutionOperation {
        self.operation
    }
}
impl ArtifactRecipe for BoundArtifactRecipe {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<ArtifactDeclaration> {
        let (kind, document_kind) = match self.operation {
            ExecutionOperation::Metadata => (ArtifactKind::Metadata, DocumentKind::ContractIr),
            ExecutionOperation::Generate => (ArtifactKind::Generation, DocumentKind::OutputSet),
            ExecutionOperation::Package => (ArtifactKind::Candidate, DocumentKind::Candidate),
            ExecutionOperation::Build => unreachable!("artifact binding excludes build"),
        };
        let response = self.adapter.decode(document_kind, stdout)?;
        validate_port_pair(&self.request, &response).map_err(format_error)?;
        let value = response.value();
        let (identity, generation_id, files) = if kind == ArtifactKind::Metadata {
            self.adapter.check(&self.prepared, stdout)?;
            (Digest::of_bytes(stdout), None, Vec::new())
        } else {
            if value["manifest_path"].as_str() != Some(self.invocation.response_path.as_str()) {
                return Err(invalid(
                    "EXECUTION002",
                    "manifest_path",
                    "response manifest differs from bound path",
                ));
            }
            if kind == ArtifactKind::Generation {
                validate_sources(&response, &self.symbols)?;
            }
            let identity = digest(
                &value[if kind == ArtifactKind::Generation {
                    "generation_id"
                } else {
                    "candidate_digest"
                }],
            )?;
            (
                identity,
                Some(digest(&value["generation_id"])?),
                references(&value["files"])?,
            )
        };
        Ok(ArtifactDeclaration {
            kind,
            identity,
            input_digest: self.input_digest.clone(),
            task_digest: self.task_digest.clone(),
            profile_digest: self.profile_digest.clone(),
            generation_id,
            files,
        })
    }
}

pub(super) fn validate_sources(output_set: &Document, symbols: &BTreeSet<String>) -> Result<()> {
    for file in output_set.value()["files"].as_array().expect("validated") {
        for source in file["sources"].as_array().expect("validated") {
            if !symbols.contains(source.as_str().expect("validated")) {
                return Err(invalid(
                    "EXECUTION002",
                    "files/sources",
                    "generated source is absent from checked IR symbols",
                ));
            }
        }
    }
    Ok(())
}
