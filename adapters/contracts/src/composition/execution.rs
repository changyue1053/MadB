//! Checked single-step /2 generation recipe for existing E02 and C04 ports.
//! No IO, process execution or storage authority is introduced here.
use super::{
    BoundCompositionInputs, GenerationBindingContext, GenerationSchemaPaths,
    GenerationStepObservation, GenerationStepRequirement, ResolvedResponsibility,
    admission::CheckedCompositionPins, bind_generated_outputs, compare_responsibility_sets,
    invalid, outputs::check_generation_request,
};
use crate::{
    Document, DocumentKind, FormatError, SchemaRegistry,
    artifacts::{ProvidedFile, validate_port_pair},
    execution::{ExecutionRecipe, validate_sources},
};
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError,
    inputs::{FrozenInputs, check_input_inventory},
    pipeline::{ArtifactDeclaration, ArtifactInvocation, ArtifactKind, ArtifactRecipe},
    ports::FileReference,
};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, FormatError>;

pub struct CompositionGenerationRequest<'a> {
    pub project: &'a Document,
    pub recipe: ExecutionRecipe,
    pub pins: CheckedCompositionPins,
    pub producer: Id,
    pub slots: Vec<Id>,
    pub output_root: LogicalPath,
    pub schemas: GenerationSchemaPaths,
    pub request: ProvidedFile<'a>,
    pub lock: ProvidedFile<'a>,
    pub profile: ProvidedFile<'a>,
    pub contract: ProvidedFile<'a>,
    pub installed_image: ProvidedFile<'a>,
    /// Must be derived by the fixed host recipe from real declarations/wiring.
    /// Submitted composition responsibility rows are never the requirements.
    pub required_responsibilities: &'a [ResolvedResponsibility],
}

pub struct BoundCompositionGeneration {
    inputs: BoundCompositionInputs,
    recipe: ExecutionRecipe,
    pins: CheckedCompositionPins,
    producer: Id,
    slots: Vec<Id>,
    output_root: LogicalPath,
    schemas: GenerationSchemaPaths,
    request: Document,
    lock: Vec<u8>,
    profile: Vec<u8>,
    contract: Vec<u8>,
    image: Vec<u8>,
    lock_path: String,
    profile_path: String,
    contract_path: String,
    invocation: ArtifactInvocation,
    symbols: BTreeSet<String>,
    expected_paths: BTreeSet<String>,
}
impl BoundCompositionGeneration {
    pub fn new(
        inputs: BoundCompositionInputs,
        request: CompositionGenerationRequest<'_>,
    ) -> Result<Self> {
        if !request
            .pins
            .matches(&inputs, &request.recipe, request.producer.as_str())
        {
            return Err(invalid(
                "ExtensionIdentityMismatch",
                "recipe",
                "execution recipe differs from checked composition pins",
            ));
        }
        compare_responsibility_sets(request.required_responsibilities, inputs.responsibilities())?;
        let registry = SchemaRegistry::new()?;
        let contract = registry.decode(DocumentKind::ContractIr, request.contract.bytes)?;
        let mut required_features = contract.value()["required_features"]
            .as_array()
            .expect("checked features")
            .iter()
            .map(|f| f.as_str().expect("feature"))
            .collect::<BTreeSet<_>>();
        required_features.insert(madb_model::contract::EXTENSION_COMPOSITION);
        if request
            .pins
            .required_features()
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != required_features
        {
            return Err(invalid(
                "CapabilityUnsupported",
                "features",
                "generator handshake must cover actual IR and composition semantics",
            ));
        }
        let capability = crate::convert::composition_capability(&inputs)?;
        crate::check_contract_with_capability(&contract, &capability)
            .map_err(|e| invalid("InputInvalid", "contract", e.to_string()))?;
        if crate::project_digest(request.project)?
            != inputs.locked_document.value()["project_digest"]
        {
            return Err(invalid(
                "LOCK002",
                "project",
                "policy project differs from fixed lock",
            ));
        }
        let (defaults, origin) = crate::convert::composition_profile_defaults(&inputs)?;
        crate::check_policy_resolution_with_defaults(
            &contract,
            request.project,
            &defaults,
            &origin,
            &[],
        )
        .map_err(|e| invalid("InputInvalid", "policy", e.to_string()))?;
        let steps = [GenerationStepRequirement {
            recipe: &request.recipe,
            producer: request.producer.clone(),
            output_slots: request.slots.clone(),
            output_root: request.output_root.clone(),
        }];
        let context = GenerationBindingContext {
            lock: ProvidedFile {
                path: request.lock.path,
                bytes: request.lock.bytes,
            },
            profile: ProvidedFile {
                path: request.profile.path,
                bytes: request.profile.bytes,
            },
            contract: ProvidedFile {
                path: request.contract.path,
                bytes: request.contract.bytes,
            },
            schema_paths: &request.schemas,
            required_steps: &steps,
        };
        check_generation_request(
            &inputs,
            &context,
            &GenerationStepObservation {
                step_id: request.recipe.id.clone(),
                request: ProvidedFile {
                    path: request.request.path,
                    bytes: request.request.bytes,
                },
                installed_tool: ProvidedFile {
                    path: request.installed_image.path,
                    bytes: request.installed_image.bytes,
                },
                outputs: &inputs.frozen_inputs,
            },
        )?;
        let document = registry.decode(DocumentKind::GenerateRequest, request.request.bytes)?;
        let checked_process = request.pins.process();
        let process = madb_app::process::ProcessRequest {
            tool: checked_process.tool.clone(),
            working_directory: checked_process.working_directory.clone(),
            arguments: checked_process.arguments.clone(),
            environment: checked_process.environment.clone(),
            stdin: request.request.bytes.to_vec(),
        };
        let invocation = ArtifactInvocation {
            id: request.recipe.id.clone(),
            process,
            inputs: request.recipe.inputs.clone(),
            response_path: request.recipe.response_path.clone(),
        };
        invocation.validate().map_err(query_error)?;
        let expected_paths = inputs.composition().value()["assets"]
            .as_array()
            .expect("checked assets")
            .iter()
            .filter(|a| a["source"]["kind"] == "generated")
            .flat_map(|a| a["source"]["paths"].as_array().expect("checked paths"))
            .map(|p| p.as_str().expect("path").to_owned())
            .collect();
        let symbols = contract.value()["symbols"]
            .as_array()
            .expect("checked symbols")
            .iter()
            .map(|s| s["public_id"].as_str().expect("public id").to_owned())
            .collect();
        Ok(Self {
            inputs,
            recipe: request.recipe,
            pins: request.pins,
            producer: request.producer,
            slots: request.slots,
            output_root: request.output_root,
            schemas: request.schemas,
            request: document,
            lock: request.lock.bytes.into(),
            profile: request.profile.bytes.into(),
            contract: request.contract.bytes.into(),
            image: request.installed_image.bytes.into(),
            lock_path: request.lock.path.into(),
            profile_path: request.profile.path.into(),
            contract_path: request.contract.path.into(),
            invocation,
            symbols,
            expected_paths,
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
        let mut expected = self.recipe.inputs.clone();
        expected.push(self.recipe.request.clone());
        check_input_inventory(&expected).map_err(query_error)?;
        expected.sort_by(|a, b| a.path.cmp(&b.path));
        if expected.iter().ne(inputs.references()) {
            return Err(invalid(
                "InputInvalid",
                "inputs",
                "session requires the exact fixed composition input closure",
            ));
        }
        for (reference, bytes) in inputs.files() {
            if Digest::of_bytes(bytes) != reference.digest || bytes.len() as u64 != reference.size {
                return Err(invalid(
                    "InputInvalid",
                    "inputs",
                    "session byte identity differs",
                ));
            }
        }
        Ok(())
    }
    pub fn check_outputs(&self, outputs: &FrozenInputs) -> Result<()> {
        let steps = [GenerationStepRequirement {
            recipe: &self.recipe,
            producer: self.producer.clone(),
            output_slots: self.slots.clone(),
            output_root: self.output_root.clone(),
        }];
        bind_generated_outputs(
            &self.inputs,
            GenerationBindingContext {
                lock: ProvidedFile {
                    path: &self.lock_path,
                    bytes: &self.lock,
                },
                profile: ProvidedFile {
                    path: &self.profile_path,
                    bytes: &self.profile,
                },
                contract: ProvidedFile {
                    path: &self.contract_path,
                    bytes: &self.contract,
                },
                schema_paths: &self.schemas,
                required_steps: &steps,
            },
            &[GenerationStepObservation {
                step_id: self.recipe.id.clone(),
                request: ProvidedFile {
                    path: self.recipe.request.path.as_str(),
                    bytes: &self.invocation.process.stdin,
                },
                installed_tool: ProvidedFile {
                    path: self.recipe.installed_tool.path.as_str(),
                    bytes: &self.image,
                },
                outputs,
            }],
        )?;
        Ok(())
    }
}
impl ArtifactRecipe for BoundCompositionGeneration {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }
    fn validate_result(&self, stdout: &[u8]) -> madb_app::Result<ArtifactDeclaration> {
        let registry = SchemaRegistry::new().map_err(app_error)?;
        let output = registry
            .decode(DocumentKind::OutputSet, stdout)
            .map_err(app_error)?;
        validate_port_pair(&self.request, &output).map_err(app_error)?;
        validate_sources(&output, &self.symbols)?;
        let v = output.value();
        if v["manifest_path"] != self.invocation.response_path.as_str() {
            return Err(app_error(invalid(
                "OutputInvalid",
                "manifest_path",
                "response location differs from fixed recipe",
            )));
        }
        let files = v["files"]
            .as_array()
            .expect("checked files")
            .iter()
            .map(|v| {
                Ok(FileReference {
                    path: LogicalPath::new(v["path"].as_str().expect("path"))
                        .map_err(|e| app_error(invalid("OutputInvalid", "path", e.to_string())))?,
                    digest: Digest::new(v["sha256"].as_str().expect("digest")).map_err(|e| {
                        app_error(invalid("OutputInvalid", "digest", e.to_string()))
                    })?,
                    size: v["size"].as_u64().expect("size"),
                })
            })
            .collect::<madb_app::Result<Vec<_>>>()?;
        let actual_paths = files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<BTreeSet<_>>();
        if files.len() != self.expected_paths.len()
            || actual_paths != self.expected_paths.iter().map(String::as_str).collect()
        {
            return Err(app_error(invalid(
                "OutputInvalid",
                "files",
                "generation must contain exactly every fixed output slot",
            )));
        }
        let digest = |key: &str| {
            Digest::new(v[key].as_str().expect("checked digest"))
                .map_err(|e| app_error(invalid("OutputInvalid", key, e.to_string())))
        };
        Ok(ArtifactDeclaration {
            kind: ArtifactKind::Generation,
            identity: digest("generation_id")?,
            input_digest: digest("input_digest")?,
            task_digest: digest("task_digest")?,
            profile_digest: digest("profile_digest")?,
            generation_id: Some(digest("generation_id")?),
            files,
        })
    }
}
fn query_error(e: QueryError) -> FormatError {
    invalid("InputInvalid", "composition", e.to_string())
}
fn app_error(e: FormatError) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, e.code, e.path, e.message)
}
