//! Explicit /2 package admission over frozen Source and Build parents.
//! Execution, cancellation, byte capture and atomic publication remain in app/host.
use super::{
    BoundCompositionInputs, ResolvedResponsibility,
    context::{checked_context, invalid},
};
use crate::{
    Document, DocumentKind,
    artifacts::{ProvidedFile, validate_port_pair},
    execution::{
        ExecutionRecipe, PackageContext, bind_tool_values,
        files::{check_bytes, declared_file, references, require_file},
        validate_package_files, validate_package_parents,
    },
    query::{SnapshotAdapter, format_error, reference},
};
use madb_app::{
    Digest, LogicalPath, Result,
    inputs::{FrozenInputs, check_input_inventory},
    pipeline::{ArtifactDeclaration, ArtifactInvocation, ArtifactKind, ArtifactRecipe},
    ports::FileReference,
    process::{MAX_STDIN_BYTES, ProcessRequest},
};
use madb_protocol::{DigestDomain, semantic_digest};
use serde_json::json;

/// A fixed host recipe may place a verified input under a public package path.
/// This mapping is not a helper-supplied claim or a new wire format.
pub struct PackageFileMapping {
    pub source: FileReference,
    pub destination: LogicalPath,
}

pub struct CompositionPackageRequest<'a> {
    pub project: &'a Document,
    pub contract: &'a Document,
    pub required_responsibilities: &'a [ResolvedResponsibility],
    pub recipe: &'a ExecutionRecipe,
    pub request: ProvidedFile<'a>,
    pub lock: ProvidedFile<'a>,
    pub source: &'a FrozenInputs,
    pub build: &'a FrozenInputs,
    pub provided: &'a FrozenInputs,
    pub files: &'a [PackageFileMapping],
}

pub struct BoundCompositionPackage {
    adapter: SnapshotAdapter,
    request: Document,
    invocation: ArtifactInvocation,
    input_digest: Digest,
    profile_digest: Digest,
    task_digest: Digest,
    files: Vec<FileReference>,
}

impl BoundCompositionPackage {
    pub fn new(
        inputs: &BoundCompositionInputs,
        request: CompositionPackageRequest<'_>,
    ) -> Result<Self> {
        let context = checked_context(
            inputs,
            request.project,
            request.contract,
            request.required_responsibilities,
        )?;
        let recipe = request.recipe;
        if request.request.bytes.len() > MAX_STDIN_BYTES {
            return Err(invalid(
                "EXECUTION001",
                "request",
                "package request exceeds stdin budget",
            ));
        }
        if request.request.path != recipe.request.path.as_str() {
            return Err(invalid(
                "FILE004",
                "request",
                "package request path differs from recipe",
            ));
        }
        check_bytes(&recipe.request, request.request.bytes)?;
        check_input_inventory(&recipe.inputs)?;
        if recipe.inputs.iter().any(|f| f.path == recipe.request.path) {
            return Err(invalid(
                "EXECUTION001",
                "inputs",
                "request cannot also be a materialized input",
            ));
        }
        for file in &recipe.inputs {
            require_file(request.provided, file)?;
        }
        let adapter = SnapshotAdapter::new()?;
        let document = adapter.decode(DocumentKind::PackageRequest, request.request.bytes)?;
        let q = document.value();
        let lock = adapter.decode(DocumentKind::Lock, request.lock.bytes)?;
        if lock.value() != context.lock.value() {
            return Err(invalid(
                "LOCK003",
                "lock",
                "package lock differs from bound composition",
            ));
        }
        let lock_ref = reference(&q["lock"])?;
        if q["lock"]["schema"] != "madb.lock/1" || lock_ref.path.as_str() != request.lock.path {
            return Err(invalid("LOCK003", "lock", "package lock reference differs"));
        }
        check_bytes(&lock_ref, request.lock.bytes)?;
        declared_file(&recipe.inputs, &lock_ref)?;
        if q["selector"] != context.selector
            || q["contract_digest"] != request.contract.value()["structure_digest"]
            || q["policy_digest"] != request.contract.value()["policy_digest"]
        {
            return Err(invalid(
                "EXECUTION002",
                "contract",
                "package selector or contract identity differs",
            ));
        }
        bind_tool_values(
            &context.profile,
            &context.lock,
            &recipe.tool,
            &recipe.installed_tool,
        )?;
        // Actual image byte capture belongs to the process host; logical image
        // identity is fixed here, exactly as for the original package adapter.
        validate_package_parents(request.request.bytes, request.source, request.build)?;
        validate_package_files(
            &adapter,
            PackageContext {
                profile: &context.profile,
                lock: &context.lock,
                policy: context.policy,
            },
            &document,
            request.provided,
            &context.source_digest,
            &context.symbols,
            &recipe.inputs,
        )?;
        // All parent bytes must retain their identities in the execution input
        // closure, not just the evidence manifest used by the helper.
        for file in request
            .source
            .references()
            .chain(request.build.references())
        {
            declared_file(&recipe.inputs, file)?;
            require_file(request.provided, file)?;
        }
        let mut files = Vec::new();
        for mapping in request.files {
            declared_file(&recipe.inputs, &mapping.source)?;
            require_file(request.provided, &mapping.source)?;
            if mapping.destination == recipe.response_path {
                return Err(invalid(
                    "FILE001",
                    "files",
                    "package asset collides with manifest",
                ));
            }
            files.push(FileReference {
                path: mapping.destination.clone(),
                digest: mapping.source.digest.clone(),
                size: mapping.source.size,
            });
        }
        check_input_inventory(&files)?;
        let mut physical_files = files.clone();
        physical_files.push(FileReference {
            path: recipe.response_path.clone(),
            digest: Digest::of_bytes(&[]),
            size: 0,
        });
        check_input_inventory(&physical_files)?;
        if files.is_empty() {
            return Err(invalid(
                "FILE003",
                "files",
                "fixed package recipe has no assets",
            ));
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        // These public references are carried unchanged by candidate/1.
        for field in ["dependencies", "build_evidence"] {
            for file in references(&q[field])? {
                declared_file(&files, &file)?;
            }
        }
        let task_digest = Digest::new(
            semantic_digest(
                DigestDomain::Task,
                &json!({
                    "operation":"package", "request":q, "execution":recipe.projection()?
                }),
            )
            .map_err(format_error)?,
        )
        .map_err(|e| invalid("HASH004", "task_digest", e.to_string()))?;
        let invocation = ArtifactInvocation {
            id: recipe.id.clone(),
            process: ProcessRequest {
                tool: recipe.installed_tool.clone(),
                working_directory: recipe.working_directory.clone(),
                arguments: recipe.arguments.clone(),
                environment: recipe.environment.clone(),
                stdin: request.request.bytes.to_vec(),
            },
            inputs: recipe.inputs.clone(),
            response_path: recipe.response_path.clone(),
        };
        invocation.validate()?;
        Ok(Self {
            adapter,
            request: document,
            invocation,
            input_digest: context.source_digest,
            profile_digest: context.profile_digest,
            task_digest,
            files,
        })
    }
}

impl ArtifactRecipe for BoundCompositionPackage {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }

    fn validate_result(&self, stdout: &[u8]) -> Result<ArtifactDeclaration> {
        let candidate = self.adapter.decode(DocumentKind::Candidate, stdout)?;
        validate_port_pair(&self.request, &candidate).map_err(format_error)?;
        let v = candidate.value();
        if v["manifest_path"].as_str() != Some(self.invocation.response_path.as_str()) {
            return Err(invalid(
                "EXECUTION002",
                "manifest_path",
                "candidate manifest path differs",
            ));
        }
        let mut files = references(&v["files"])?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if files != self.files {
            return Err(invalid(
                "FILE004",
                "files",
                "candidate assets differ from the fixed verified package mapping",
            ));
        }
        let digest = |field: &str| {
            Digest::new(v[field].as_str().expect("checked digest"))
                .map_err(|e| invalid("HASH004", field, e.to_string()))
        };
        Ok(ArtifactDeclaration {
            kind: ArtifactKind::Candidate,
            identity: digest("candidate_digest")?,
            input_digest: self.input_digest.clone(),
            task_digest: self.task_digest.clone(),
            profile_digest: self.profile_digest.clone(),
            generation_id: Some(digest("generation_id")?),
            files,
        })
    }
}
