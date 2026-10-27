//! File domains for E02. Control references are relative to `work`; payload
//! paths remain relative to the immutable input or per-request output domain.
//! Keeping those paths unchanged also keeps existing semantic identities intact.
use crate::{
    Document, DocumentKind, FormatError, SchemaRegistry,
    artifacts::{ProvidedFile, validate_port_pair, verify_files, verify_reference},
    composition::execution::BoundCompositionGeneration,
    execution::{BoundArtifactRecipe, ExecutionOperation},
    wire::Wire,
};
use madb_app::{
    Digest, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    pipeline::ArtifactRecipe,
    ports::{FileReference, ReadWorkspace},
};
use madb_model::evidence::RequiredCase;
use madb_protocol::Value;
use std::collections::{BTreeMap, BTreeSet};

pub struct BoundExtensionFiles {
    operation: String,
    input_ref: Value,
    output_root: String,
    inputs: FrozenInputs,
    request: Document,
    registry: SchemaRegistry,
    artifact: Option<BoundArtifactRecipe>,
    composition: Option<BoundCompositionGeneration>,
}

impl BoundExtensionFiles {
    /// `inputs` contains domain-local paths, including the request payload.
    /// Its bytes must have been captured from the host's trusted input selection.
    pub fn new(
        operation: &str,
        input_ref: &Value,
        output_root: &str,
        inputs: FrozenInputs,
    ) -> Result<Self, FormatError> {
        if matches!(operation, "read" | "generate" | "package") {
            return Err(FormatError::new(
                "InputInvalid",
                "/context",
                "artifact operation requires a checked BoundArtifactRecipe",
            ));
        }
        Self::bind(operation, input_ref, output_root, inputs)
            .map_err(|e| contextual("InputInvalid", e))
    }

    /// Reuse C04's checked project/profile/lock, source lineage and completion
    /// policy. The proof and its result validator remain owned by this session.
    pub fn from_artifact_recipe(
        input_ref: &Value,
        output_root: &str,
        inputs: FrozenInputs,
        artifact: BoundArtifactRecipe,
    ) -> Result<Self, FormatError> {
        let operation = match artifact.operation() {
            ExecutionOperation::Metadata => "read",
            ExecutionOperation::Generate => "generate",
            ExecutionOperation::Package => "package",
            ExecutionOperation::Build => {
                return Err(FormatError::new(
                    "InputInvalid",
                    "/context",
                    "not an artifact recipe",
                ));
            }
        };
        let mut bound = Self::bind(operation, input_ref, output_root, inputs)
            .map_err(|e| contextual("InputInvalid", e))?;
        let (kind, _) = kinds(operation)?;
        let reference = control_reference(input_ref, "inputs", kind)?;
        if frozen_bytes(&bound.inputs, &reference)? != artifact.invocation().process.stdin {
            return Err(FormatError::new(
                "InputInvalid",
                "/request",
                "checked recipe and session request bytes differ",
            ));
        }
        for input in &artifact.invocation().inputs {
            frozen_bytes(&bound.inputs, input).map_err(|e| contextual("InputInvalid", e))?;
        }
        bound.artifact = Some(artifact);
        Ok(bound)
    }

    /// Explicit /2 proof. Legacy constructors still reject composition inputs.
    pub fn from_composition_recipe(
        input_ref: &Value,
        inputs: FrozenInputs,
        recipe: BoundCompositionGeneration,
    ) -> Result<Self, FormatError> {
        let reference = control_reference(input_ref, "inputs", DocumentKind::GenerateRequest)?;
        recipe.check_session_inputs(&inputs)?;
        if frozen_bytes(&inputs, &reference)? != recipe.invocation().process.stdin {
            return Err(invalid(
                "/request",
                "checked composition and session request bytes differ",
            ));
        }
        let output_root = recipe.output_root().as_str().to_owned();
        domain_root(&output_root)?;
        let registry = SchemaRegistry::new()?;
        let request = registry.decode(
            DocumentKind::GenerateRequest,
            frozen_bytes(&inputs, &reference)?,
        )?;
        if request.value()["output_root"] != output_root {
            return Err(invalid("/output_root", "composition output domain differs"));
        }
        Ok(Self {
            operation: "generate".into(),
            input_ref: input_ref.clone(),
            output_root,
            inputs,
            request,
            registry,
            artifact: None,
            composition: Some(recipe),
        })
    }
    pub fn composition_recipe(&self) -> Option<&BoundCompositionGeneration> {
        self.composition.as_ref()
    }

    fn bind(
        operation: &str,
        input_ref: &Value,
        output_root: &str,
        inputs: FrozenInputs,
    ) -> Result<Self, FormatError> {
        let (request_kind, _) = kinds(operation)?;
        let local_input = control_reference(input_ref, "inputs", request_kind)?;
        domain_root(output_root)?;
        let registry = SchemaRegistry::new()?;
        let bytes = frozen_bytes(&inputs, &local_input)?;
        let request = registry.decode(request_kind, bytes)?;
        if request_kind != DocumentKind::BuildTask && request.value()["output_root"] != output_root
        {
            return Err(invalid(
                "/output_root",
                "payload and control output domains differ",
            ));
        }
        let bound = Self {
            operation: operation.into(),
            input_ref: input_ref.clone(),
            output_root: output_root.into(),
            inputs,
            request,
            registry,
            artifact: None,
            composition: None,
        };
        bound.check_inputs()?;
        Ok(bound)
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn input_ref(&self) -> &Value {
        &self.input_ref
    }
    pub fn output_root(&self) -> &str {
        &self.output_root
    }
    pub fn inputs(&self) -> &FrozenInputs {
        &self.inputs
    }
    pub fn request(&self) -> &Document {
        &self.request
    }
    pub(crate) fn artifact_process(&self) -> Option<&madb_app::process::ProcessRequest> {
        self.artifact
            .as_ref()
            .map(|artifact| &artifact.invocation().process)
            .or_else(|| self.composition.as_ref().map(|r| &r.invocation().process))
    }

    fn check_inputs(&self) -> Result<(), FormatError> {
        let value = self.request.value();
        let mut root = self.input_ref.clone();
        root["path"] = Value::String(
            control_reference(&root, "inputs", self.request.kind())?
                .path
                .as_str()
                .to_owned(),
        );
        let closure = capture_closure(&self.registry, &FrozenWorkspace(&self.inputs), root, true)?;
        if closure.references().ne(self.inputs.references()) {
            return Err(invalid(
                "/inputs",
                "frozen inventory contains files outside the declared reference closure",
            ));
        }
        if self.request.kind() == DocumentKind::BuildTask {
            if value["task_digest"].as_str() != Some(crate::task_digest(&self.request)?.as_str()) {
                return Err(invalid(
                    "/task_digest",
                    "build task semantic identity mismatch",
                ));
            }
            for output in value["outputs"].as_array().into_iter().flatten() {
                logical(output.as_str().unwrap_or_default())?;
            }
            logical(value["working_directory"].as_str().unwrap_or_default())?;
        }
        if self.request.kind() == DocumentKind::VerifyRequest {
            let candidate = self.input_document(&value["candidate"], DocumentKind::Candidate)?;
            let coverage = self.input_document(&value["coverage"], DocumentKind::Coverage)?;
            if candidate.value()["selector"] != value["selector"]
                || coverage.value()["selector"] != value["selector"]
                || candidate.value()["candidate_digest"] != value["identity"]["candidate_digest"]
                || value["identity"]["profile_digest"] != value["selector"]["profile_digest"]
            {
                return Err(invalid(
                    "/candidate",
                    "verification input identity or selector mismatch",
                ));
            }
            let mut build_ids = BTreeSet::new();
            for reference in candidate.value()["build_evidence"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let evidence = self.input_document(reference, DocumentKind::BuildEvidence)?;
                let e = evidence.value();
                if e["generation_id"] != candidate.value()["generation_id"]
                    || e["input_digest"] != value["identity"]["input_digest"]
                    || e["profile_digest"] != value["identity"]["profile_digest"]
                    || e["target"] != value["selector"]["target"]
                    || !build_ids.insert(e["task_id"].as_str().unwrap_or_default().to_owned())
                {
                    return Err(invalid(
                        "/candidate/build_evidence",
                        "verification candidate has mixed or duplicate build lineage",
                    ));
                }
            }
            let mut cases = BTreeSet::new();
            for reference in value["cases"].as_array().into_iter().flatten() {
                let case = self.input_document(reference, DocumentKind::Case)?;
                if case.value()["selector"] != value["selector"]
                    || !cases.insert(case.value()["id"].as_str().unwrap_or_default().to_owned())
                {
                    return Err(invalid(
                        "/cases",
                        "duplicate case or case selector mismatch",
                    ));
                }
            }
            let (_, requirements) = crate::requirements(&coverage)?;
            let mut required = candidate.value()["required_cases"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            required.extend(
                requirements
                    .iter()
                    .filter(|r| r.applicability == madb_model::evidence::Applicability::Required)
                    .flat_map(|r| r.case_ids.iter().map(|id| id.as_str().to_owned())),
            );
            if !required.is_subset(&cases) {
                return Err(invalid(
                    "/cases",
                    "request omits cases required by frozen coverage or candidate",
                ));
            }
        }
        if matches!(
            self.request.kind(),
            DocumentKind::ReadRequest | DocumentKind::GenerateRequest
        ) {
            let profile = self.input_document(&value["profile"], DocumentKind::Profile)?;
            if value["selector"]["profile_id"] != profile.value()["id"]
                || value["selector"]["profile_digest"].as_str()
                    != Some(crate::profile_digest(&profile)?.as_str())
            {
                return Err(invalid("/profile", "input profile identity mismatch"));
            }
            self.input_document(&value["lock"], DocumentKind::Lock)?;
            if self.request.kind() == DocumentKind::GenerateRequest {
                let contract = self.input_document(&value["contract"], DocumentKind::ContractIr)?;
                crate::check_contract(&contract, &profile).map_err(|e| invalid("/contract", e))?;
                if value["contract_digest"] != contract.value()["structure_digest"]
                    || value["policy_digest"] != contract.value()["policy_digest"]
                    || value["input_digest"] != contract.value()["source_digest"]
                    || value["input_digest"].as_str()
                        != Some(crate::source_digest(&value["inputs"])?.as_str())
                {
                    return Err(invalid(
                        "/contract",
                        "generator used different contract semantics",
                    ));
                }
            }
        }
        Ok(())
    }

    fn input_document(
        &self,
        reference: &Value,
        kind: DocumentKind,
    ) -> Result<Document, FormatError> {
        if reference["schema"] != kind.schema_id() {
            return Err(invalid("/schema", "wrong referenced input format"));
        }
        self.registry.decode(
            kind,
            frozen_bytes(&self.inputs, &reference_from(reference)?)?,
        )
    }

    /// Capture an observation when `result` arrives. `workspace` is rooted at
    /// output_root. This grants no result admission: the host must retain this
    /// snapshot, establish EOF/exit-zero/clean process state, then recheck bytes,
    /// exact inventory and directory identity inside cancellation admission.
    pub fn capture_result<W: ReadWorkspace>(
        &self,
        workspace: &W,
        output_ref: &Value,
    ) -> Result<FrozenExtensionResult, FormatError> {
        self.capture(workspace, output_ref)
            .map_err(|e| contextual("OutputInvalid", e))
    }

    fn capture<W: ReadWorkspace>(
        &self,
        workspace: &W,
        output_ref: &Value,
    ) -> Result<FrozenExtensionResult, FormatError> {
        let (_, response_kind) = kinds(&self.operation)?;
        let manifest_ref = control_reference(output_ref, &self.output_root, response_kind)?;
        let manifest = capture_inputs(workspace, vec![manifest_ref.clone()])
            .map_err(|e| invalid("/output", e))?;
        let document = self
            .registry
            .decode(response_kind, frozen_bytes(&manifest, &manifest_ref)?)?;
        validate_port_pair(&self.request, &document)?;
        if let Some(artifact) = &self.artifact {
            artifact
                .validate_result(frozen_bytes(&manifest, &manifest_ref)?)
                .map_err(|e| invalid("/output", e))?;
        }
        if let Some(recipe) = &self.composition {
            recipe
                .validate_result(frozen_bytes(&manifest, &manifest_ref)?)
                .map_err(|e| invalid("/output", e))?;
        }
        if response_kind == DocumentKind::ContractIr {
            let profile =
                self.input_document(&self.request.value()["profile"], DocumentKind::Profile)?;
            crate::check_contract(&document, &profile).map_err(|e| invalid("/output", e))?;
        }
        if matches!(
            response_kind,
            DocumentKind::OutputSet | DocumentKind::Candidate
        ) && document.value()["manifest_path"] != manifest_ref.path.as_str()
        {
            return Err(invalid(
                "/manifest_path",
                "manifest location differs from control reference",
            ));
        }
        let mut root = output_ref.clone();
        root["path"] = Value::String(manifest_ref.path.as_str().to_owned());
        let files = capture_closure(&self.registry, workspace, root, false)?;
        // Re-capture must not replace the document that was checked above.
        if frozen_bytes(&manifest, &manifest_ref)? != frozen_bytes(&files, &manifest_ref)? {
            return Err(invalid("/output", "result changed while being captured"));
        }
        if matches!(
            response_kind,
            DocumentKind::OutputSet | DocumentKind::Candidate
        ) {
            verify_manifest_snapshot(&document, &files)?;
        }
        if let Some(recipe) = &self.composition {
            recipe.check_outputs(&files)?;
        }
        Ok(FrozenExtensionResult {
            document,
            files,
            reference: output_ref.clone(),
            request: self.request.clone(),
            inputs: self.inputs.clone(),
        })
    }
}

/// A frozen observation, not publication authority. Failed business results can
/// be retained for diagnosis; only the explicit successful admission returns an
/// upstream snapshot. Reports also need independently selected case requirements.
pub struct FrozenExtensionResult {
    document: Document,
    files: FrozenInputs,
    reference: Value,
    request: Document,
    inputs: FrozenInputs,
}
impl FrozenExtensionResult {
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn reference(&self) -> &Value {
        &self.reference
    }
    pub fn ensure_success(&self) -> Result<(), FormatError> {
        if self.document.kind() == DocumentKind::Report {
            return Err(invalid(
                "/report",
                "report requires independent case and evidence-level acceptance",
            ));
        }
        ensure_business_success(&self.document)
    }
    pub fn into_inputs(self) -> Result<FrozenInputs, FormatError> {
        self.ensure_success()?;
        Ok(self.files)
    }
    /// Requirements must come from the caller's coverage policy, not this report.
    /// Every requested case must match its independently frozen definition.
    pub fn accept_report(self, required: &[RequiredCase]) -> Result<FrozenInputs, FormatError> {
        if self.document.kind() != DocumentKind::Report {
            return Err(invalid("/schema", "not a verification report"));
        }
        let registry = SchemaRegistry::new()?;
        let coverage = registry.decode(
            DocumentKind::Coverage,
            frozen_bytes(
                &self.inputs,
                &reference_from(&self.request.value()["coverage"])?,
            )?,
        )?;
        let (selector, _) = crate::requirements(&coverage)?;
        let mut expected = BTreeSet::new();
        for reference in self.request.value()["cases"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let case = registry.decode(
                DocumentKind::Case,
                frozen_bytes(&self.inputs, &reference_from(reference)?)?,
            )?;
            let id = case.value()["id"].as_str().unwrap_or_default();
            if !expected.insert(id.to_owned()) {
                return Err(invalid("/cases", "duplicate input case"));
            }
            let requirement = required
                .iter()
                .find(|r| r.id.as_str() == id)
                .ok_or_else(|| {
                    invalid("/cases", "requested case missing independent acceptance")
                })?;
            if requirement.digest.as_str() != crate::case_digest(&case)? {
                return Err(invalid("/cases", "case expectation identity mismatch"));
            }
            if &crate::required_case(&case, &selector, requirement.minimum_level)? != requirement {
                return Err(invalid(
                    "/cases",
                    "acceptance changed the independently loaded case expectation",
                ));
            }
        }
        if expected.len() != required.len() {
            return Err(invalid(
                "/cases",
                "acceptance has undeclared or duplicate cases",
            ));
        }
        let findings = madb_rules::validate_required_results(
            &crate::evidence_identity(&self.request)?,
            &crate::evidence_identity(&self.document)?,
            required,
            &crate::report_results(&self.document)?,
        );
        if !findings.is_empty() {
            return Err(invalid(
                "/report",
                format!("report failed required evidence: {findings:?}"),
            ));
        }
        Ok(self.files)
    }
}

fn ensure_business_success(document: &Document) -> Result<(), FormatError> {
    if document.kind() == DocumentKind::BuildEvidence
        && (document.value()["status"] != "passed" || document.value()["exit_code"] != 0)
    {
        return Err(FormatError::new(
            "ToolFailed",
            "/status",
            "business build failed; output cannot be used downstream",
        ));
    }
    Ok(())
}
fn kinds(operation: &str) -> Result<(DocumentKind, DocumentKind), FormatError> {
    use DocumentKind::*;
    match operation {
        "read" => Ok((ReadRequest, ContractIr)),
        "generate" => Ok((GenerateRequest, OutputSet)),
        "build" => Ok((BuildTask, BuildEvidence)),
        "verify" => Ok((VerifyRequest, Report)),
        "package" => Ok((PackageRequest, Candidate)),
        _ => Err(invalid("/operation", "unknown extension operation")),
    }
}
fn document_kind(schema: &str) -> Option<DocumentKind> {
    DocumentKind::ALL
        .into_iter()
        .find(|kind| kind.schema_id() == schema)
}
fn logical(path: &str) -> Result<LogicalPath, FormatError> {
    if !path.is_ascii() {
        return Err(invalid(
            path,
            "file paths must use the existing ASCII inventory policy",
        ));
    }
    LogicalPath::new(path).map_err(|e| invalid(path, e))
}
fn domain_root(root: &str) -> Result<(), FormatError> {
    logical(root)?;
    if !root.starts_with("outputs/") {
        return Err(invalid(
            "/output_root",
            "output domain must be under outputs",
        ));
    }
    Ok(())
}
fn reference_from(value: &Value) -> Result<FileReference, FormatError> {
    let wire = Wire::at(value, "/file");
    Ok(FileReference {
        path: logical(wire.field("path")?.string()?)?,
        digest: wire.field("sha256")?.digest()?,
        size: wire.field("size")?.uint()?,
    })
}
fn control_reference(
    value: &Value,
    domain: &str,
    kind: DocumentKind,
) -> Result<FileReference, FormatError> {
    if value["schema"] != kind.schema_id() {
        return Err(invalid(
            "/schema",
            "control reference has wrong port payload format",
        ));
    }
    let mut reference = reference_from(value)?;
    let local = reference
        .path
        .as_str()
        .strip_prefix(&format!("{domain}/"))
        .ok_or_else(|| invalid("/path", "control reference escapes its fixed domain"))?;
    reference.path = logical(local)?;
    Ok(reference)
}
fn frozen_bytes<'a>(
    files: &'a FrozenInputs,
    reference: &FileReference,
) -> Result<&'a [u8], FormatError> {
    let (_, bytes) = files
        .files()
        .find(|(r, _)| r.path == reference.path)
        .ok_or_else(|| invalid(reference.path.as_str(), "file absent from frozen inventory"))?;
    if bytes.len() as u64 != reference.size || Digest::of_bytes(bytes) != reference.digest {
        return Err(invalid(
            reference.path.as_str(),
            "frozen bytes differ from reference",
        ));
    }
    Ok(bytes)
}
fn input_references(document: &Document) -> Vec<&Value> {
    let q = document.value();
    let (arrays, singles): (&[&str], &[&str]) = match document.kind() {
        DocumentKind::ReadRequest => (&["inputs"], &["lock", "profile"]),
        DocumentKind::GenerateRequest => (&["inputs"], &["lock", "profile", "contract"]),
        DocumentKind::BuildTask => (&["inputs"], &[]),
        DocumentKind::VerifyRequest => (&["cases"], &["candidate", "coverage"]),
        DocumentKind::PackageRequest => {
            (&["build_evidence", "dependencies"], &["output_set", "lock"])
        }
        _ => (&[], &[]),
    };
    let mut refs = Vec::new();
    for field in arrays {
        refs.extend(q[field].as_array().into_iter().flatten());
    }
    for field in singles {
        refs.push(&q[field]);
    }
    refs
}
fn output_references(document: &Document) -> Vec<&Value> {
    let q = document.value();
    match document.kind() {
        DocumentKind::OutputSet | DocumentKind::Candidate => {
            q["files"].as_array().into_iter().flatten().collect()
        }
        DocumentKind::BuildEvidence => ["outputs", "logs"]
            .into_iter()
            .flat_map(|f| q[f].as_array().into_iter().flatten())
            .collect(),
        DocumentKind::Report => q["results"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|r| r["attachments"].as_array().into_iter().flatten())
            .collect(),
        _ => vec![],
    }
}
fn verify_manifest_snapshot(document: &Document, files: &FrozenInputs) -> Result<(), FormatError> {
    let mut provided = Vec::new();
    for reference in output_references(document) {
        let parsed = reference_from(reference)?;
        let bytes = frozen_bytes(files, &parsed)?;
        let path = reference["path"].as_str().unwrap_or_default();
        verify_reference(reference, &ProvidedFile { path, bytes })?;
        provided.push(ProvidedFile { path, bytes });
    }
    verify_files(document, &provided)
}

struct FrozenWorkspace<'a>(&'a FrozenInputs);
impl ReadWorkspace for FrozenWorkspace<'_> {
    fn read(&self, path: &LogicalPath, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        match self
            .0
            .files()
            .find(|(reference, _)| &reference.path == path)
        {
            Some((_, bytes)) if bytes.len() <= limit => Ok(Some(bytes.to_vec())),
            Some(_) => Err(madb_app::QueryError::new(
                madb_app::ErrorKind::InvalidInput,
                "INPUT002",
                path.as_str(),
                "frozen file exceeds requested byte budget",
            )),
            None => Ok(None),
        }
    }
}

/// Follow only the file fields of supported payloads, never arbitrary JSON keys,
/// source locations, tool source URLs or schema $refs. Each growing inventory is
/// preflighted by capture_inputs with its existing whole-inventory budgets.
fn capture_closure<W: ReadWorkspace>(
    registry: &SchemaRegistry,
    workspace: &W,
    root: Value,
    upstream: bool,
) -> Result<FrozenInputs, FormatError> {
    let root_path = reference_from(&root)?.path;
    let mut declarations = BTreeMap::<String, FileReference>::new();
    let mut formats = BTreeMap::<String, String>::new();
    let mut pending = std::collections::VecDeque::from([root]);
    let mut captured = ClosureFiles(BTreeMap::new());
    let mut documents = Vec::new();
    while let Some(value) = pending.pop_front() {
        let reference = reference_from(&value)?;
        let path = reference.path.as_str().to_owned();
        if let Some(prior) = declarations.get(&path) {
            if prior != &reference {
                return Err(invalid(&path, "conflicting transitive file references"));
            }
        } else {
            declarations.insert(path.clone(), reference.clone());
            madb_app::inputs::check_input_inventory(
                &declarations.values().cloned().collect::<Vec<_>>(),
            )
            .map_err(|e| invalid(&path, e))?;
            let bytes = workspace
                .read(&reference.path, reference.size as usize)
                .map_err(|e| invalid(&path, e))?
                .ok_or_else(|| invalid(&path, "declared transitive file is missing"))?;
            if bytes.len() as u64 != reference.size || Digest::of_bytes(&bytes) != reference.digest
            {
                return Err(invalid(
                    &path,
                    "transitive file size or byte digest mismatch",
                ));
            }
            captured.0.insert(reference.path.clone(), bytes);
        }
        let Some(schema) = value.get("schema").and_then(Value::as_str) else {
            continue;
        };
        if let Some(prior) = formats.insert(path.clone(), schema.to_owned()) {
            if prior != schema {
                return Err(invalid(
                    &path,
                    "one file has contradictory format declarations",
                ));
            }
            continue;
        }
        let Some(kind) = document_kind(schema) else {
            continue;
        };
        let document = registry.decode(kind, &captured.0[&reference.path])?;
        if kind == DocumentKind::BuildEvidence && (upstream || reference.path != root_path) {
            ensure_business_success(&document)?;
        }
        if matches!(kind, DocumentKind::OutputSet | DocumentKind::Candidate)
            && document.value()["manifest_path"] != path.as_str()
        {
            return Err(invalid(
                &path,
                "referenced manifest path differs from its domain-local location",
            ));
        }
        pending.extend(declared_references(&document)?.into_iter().cloned());
        documents.push(document);
    }
    let files = capture_inputs(&captured, declarations.into_values().collect())
        .map_err(|e| invalid("/files", e))?;
    for document in &documents {
        if matches!(
            document.kind(),
            DocumentKind::OutputSet | DocumentKind::Candidate
        ) {
            verify_manifest_snapshot(document, &files)?;
        }
    }
    files.verify(workspace).map_err(|e| invalid("/files", e))?;
    Ok(files)
}

struct ClosureFiles(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for ClosureFiles {
    fn read(&self, path: &LogicalPath, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        match self.0.get(path) {
            Some(bytes) if bytes.len() <= limit => Ok(Some(bytes.clone())),
            Some(_) => Err(madb_app::QueryError::new(
                madb_app::ErrorKind::InvalidInput,
                "INPUT002",
                path.as_str(),
                "captured file exceeds requested byte budget",
            )),
            None => Ok(None),
        }
    }
}

fn declared_references(document: &Document) -> Result<Vec<&Value>, FormatError> {
    let q = document.value();
    let mut refs = input_references(document);
    refs.extend(output_references(document));
    let mut typed = Vec::new();
    match document.kind() {
        DocumentKind::Composition | DocumentKind::ProfileV2 => {
            return Err(FormatError::new(
                "CapabilityUnsupported",
                "/schema",
                "legacy extension file admission does not support composition formats",
            ));
        }
        DocumentKind::ReadRequest | DocumentKind::GenerateRequest => {
            typed.extend([
                (&q["lock"], DocumentKind::Lock),
                (&q["profile"], DocumentKind::Profile),
            ]);
            if document.kind() == DocumentKind::GenerateRequest {
                typed.push((&q["contract"], DocumentKind::ContractIr));
            }
        }
        DocumentKind::VerifyRequest => {
            typed.extend([
                (&q["candidate"], DocumentKind::Candidate),
                (&q["coverage"], DocumentKind::Coverage),
            ]);
            typed.extend(
                q["cases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|v| (v, DocumentKind::Case)),
            );
        }
        DocumentKind::PackageRequest => {
            typed.extend([
                (&q["output_set"], DocumentKind::OutputSet),
                (&q["lock"], DocumentKind::Lock),
            ]);
        }
        DocumentKind::Candidate => {
            refs.extend(q["dependencies"].as_array().into_iter().flatten());
            refs.extend(q["build_evidence"].as_array().into_iter().flatten());
        }
        DocumentKind::Profile => {
            refs.push(&q["sdk"]);
            if !q["backend_config"].is_null() {
                refs.push(&q["backend_config"]);
            }
        }
        DocumentKind::Lock => {
            refs.extend(q["inputs"].as_array().into_iter().flatten());
            refs.extend(q["schemas"].as_array().into_iter().flatten());
            for profile in q["profiles"].as_array().into_iter().flatten() {
                refs.push(&profile["file"]);
                typed.push((&profile["file"], DocumentKind::Profile));
            }
        }
        DocumentKind::Case => refs.push(&q["fixture"]),
        DocumentKind::Project => {
            for boundary in q["boundaries"].as_array().into_iter().flatten() {
                if boundary["profile"].is_object() {
                    refs.push(&boundary["profile"]);
                    typed.push((&boundary["profile"], DocumentKind::Profile));
                }
            }
        }
        DocumentKind::Execution => {
            for (field, kind) in [
                ("project", DocumentKind::Project),
                ("read_request", DocumentKind::ReadRequest),
                ("lock", DocumentKind::Lock),
                ("profile", DocumentKind::Profile),
                ("contract", DocumentKind::ContractIr),
            ] {
                if !q["context"][field].is_null() {
                    refs.push(&q["context"][field]);
                    typed.push((&q["context"][field], kind));
                }
            }
            if !q["recipe"].is_null() {
                refs.extend([&q["recipe"]["request"], &q["recipe"]["installed_tool"]]);
                refs.extend(q["recipe"]["inputs"].as_array().into_iter().flatten());
            }
            for task in q["tasks"].as_array().into_iter().flatten() {
                refs.extend([&task["request"], &task["installed_tool"]]);
                typed.push((&task["request"], DocumentKind::BuildTask));
            }
            if !q["dependencies"].is_null() {
                refs.push(&q["dependencies"]["request"]);
                if !q["dependencies"]["receipt"].is_null() {
                    refs.push(&q["dependencies"]["receipt"]);
                }
            }
        }
        _ => {}
    }
    if matches!(
        document.kind(),
        DocumentKind::PackageRequest | DocumentKind::Candidate
    ) {
        typed.extend(
            q["build_evidence"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|v| (v, DocumentKind::BuildEvidence)),
        );
    }
    for (reference, kind) in typed {
        if reference["schema"] != kind.schema_id() {
            return Err(invalid(
                "/schema",
                "transitive reference has wrong payload format",
            ));
        }
    }
    Ok(refs)
}
fn invalid(path: &str, error: impl std::fmt::Display) -> FormatError {
    FormatError::new("OutputInvalid", path, error.to_string())
}
fn contextual(code: &'static str, error: FormatError) -> FormatError {
    if error.code == code {
        return error;
    }
    FormatError::new(
        code,
        error.path,
        format!("{}: {}", error.code, error.message),
    )
}
