//! The single installed values recipe. Helpers prepare C04 documents; only the
//! existing in-process C04 coordinator commits generated/build/candidate state.
use super::{AttemptError, AttemptResult, InitControl, evidence, invalid};
use crate::bootstrap::execution::{self, ExecutionJournal, ExecutionOptions, RetainedExecution};
use madb_app::ports::ReadWorkspace;
use madb_app::transaction::Layer;
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    process::{ProcessPolicy, ResultAdmission},
    use_cases::initialize::*,
};
use madb_contracts::{
    DocumentKind, SchemaRegistry,
    artifacts::{ProvidedFile, verify_files},
};
use madb_host::{
    LocalWorkspace,
    initialization::LocalInitEvidence,
    initialization_execution::{
        FinalProjectExecution, InitializationProcess, InstalledValuesTools,
        LockedInitializationFile, LockedInitializationTool,
    },
    initialization_source::FrozenInitializationTool,
    transaction::{BatchLease, LocalBatchStore},
};
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{CheckLevel, ParameterValue};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct ReadyRecipe {
    tools: InstalledValuesTools,
    revision: &'static str,
}
impl ReadyRecipe {
    pub fn preflight(request: &FrozenInitRequest) -> Result<Option<Self>> {
        if request.projection().dry_run || request.projection().verify == VerifyLevel::Structure {
            return Ok(None);
        }
        if request.selected().template().version != Version::parse("0.1.1").expect("fixed version")
        {
            return Err(invalid(
                "template.version",
                "the preserved 0.1.0 template supports only structure and dry-run",
            ));
        }
        let revision=option_env!("MADB_SOURCE_REVISION").filter(|v|v.len()==40 && v.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
            .ok_or_else(||QueryError::new(ErrorKind::Host,"HOST005","tool.source","this compiled tool lacks its exact source revision; install a source-bound build/run candidate"))?;
        let tools = InstalledValuesTools::capture()?;
        let expected = tools.observations();
        let requirements = &request.selected().profile().requirements;
        let mut groups = vec![
            requirements.structure.tools.as_slice(),
            requirements.build.tools.as_slice(),
        ];
        if request.projection().verify == VerifyLevel::Run {
            groups.push(requirements.run.tools.as_slice());
        }
        for requirement in madb_plan::project::merge_tool_requirements(&groups)
            .map_err(|e| invalid(&e.subject, e.message))?
        {
            if !expected.iter().any(|t| {
                t.id == requirement.id
                    && t.version == requirement.version
                    && t.sha256 == requirement.sha256
            }) {
                return Err(invalid(
                    requirement.id.as_str(),
                    "selected required tool differs from the compiled installation record",
                ));
            }
        }
        Ok(Some(Self { tools, revision }))
    }
    pub fn execute(
        &self,
        project: &PreparedProject,
        tool: &FrozenInitializationTool,
        control: &InitControl,
        observations: &mut SuccessObservations,
        store: &mut LocalInitEvidence,
        retained: &mut Vec<RetainedExecution>,
    ) -> AttemptResult<()> {
        let filesystem = FinalProjectExecution::open(project)?;
        filesystem.create_native_root()?;
        filesystem.materialize_runtime()?;
        filesystem.write_native_new(&path("installation.json"), self.tools.native_observation())?;
        filesystem.write_native_new(
            &path("execution-resources.json"),
            self.tools.execution_resources(),
        )?;
        let namespace = match project
            .request()
            .projection()
            .parameter_values
            .get(&Id::new("namespace").expect("fixed id"))
        {
            Some(ParameterValue::String(value)) => value.as_str(),
            _ => return Err(invalid("namespace", "resolved namespace is absent").into()),
        };
        let mut state = RunState {
            recipe: self,
            filesystem,
            tool,
            control,
            observations,
            store,
            namespace,
            request: project.request(),
            sequence: 0,
            retained,
            cleanup: None,
            extra_tools: Vec::new(),
            batches: Vec::new(),
            consumer: None,
        };
        state.log(
            "installation",
            "execution-resources.json",
            self.tools.execution_resources(),
        )?;
        let result = state.native_generate();
        state.record_check("native-generate", EvidenceLevel::Generated, &result)?;
        if let Err(error) = result {
            state.block_remaining("native-generate")?;
            return Err(error);
        }
        state.cleanup = None;
        let result = state.native_link();
        state.record_check("c-link", EvidenceLevel::Linked, &result)?;
        if let Err(error) = result {
            state.block_remaining("c-link")?;
            return Err(error);
        }
        if project.request().projection().verify == VerifyLevel::Run {
            state.cleanup = None;
            let result = state.ordinary_run();
            state.record_check("ordinary-run", EvidenceLevel::HostRun, &result)?;
            result?;
        }
        state.recipe.tools.revalidate()?;
        state.filesystem.verify_initial()?;
        tool.revalidate()?;
        control.checkpoint()
    }
}
struct RunState<'a> {
    recipe: &'a ReadyRecipe,
    filesystem: FinalProjectExecution,
    tool: &'a FrozenInitializationTool,
    control: &'a InitControl,
    observations: &'a mut SuccessObservations,
    store: &'a mut LocalInitEvidence,
    namespace: &'a str,
    sequence: usize,
    retained: &'a mut Vec<RetainedExecution>,
    cleanup: Option<bool>,
    request: &'a FrozenInitRequest,
    extra_tools: Vec<ToolObservation>,
    batches: Vec<(BatchLease, Digest)>,
    consumer: Option<LockedInitializationFile>,
}
impl RunState<'_> {
    fn native_generate(&mut self) -> AttemptResult<()> {
        self.helper("prepare")?;
        let metadata = self
            .c04("metadata")?
            .ok_or_else(|| invalid("metadata", "C04 returned no verified contract bytes"))?;
        self.filesystem
            .write_native_new(&path("contract.json"), &metadata)?;
        self.helper("generate-plan")?;
        self.c04("generate")?;
        self.save_layer(Layer::Source, "source")?;
        Ok(())
    }
    fn native_link(&mut self) -> AttemptResult<()> {
        self.helper("build-plan")?;
        self.c04("build")?;
        self.save_layer(Layer::Build, "build")?;
        self.helper("package-plan")?;
        self.c04("package")?;
        let candidate = self.candidate()?;
        self.save_layer(Layer::Candidate, "candidate")?;
        let root = self.filesystem.root().to_path_buf();
        let build = root.join(".madb/build/cmake");
        let library = candidate
            .path()
            .join(format!("lib/lib{}.dylib", self.namespace));
        let arguments = vec![
            "-S".into(),
            text(&root)?,
            "-B".into(),
            text(&build)?,
            "-G".into(),
            "Ninja".into(),
            format!(
                "-DCMAKE_MAKE_PROGRAM:FILEPATH={}",
                text(&self.recipe.tools.tool("ninja")?.file.path)?
            ),
            format!(
                "-DCMAKE_C_COMPILER:FILEPATH={}",
                text(&self.recipe.tools.tool("clang")?.file.path)?
            ),
            format!(
                "-DCMAKE_OSX_SYSROOT:PATH={}",
                text(self.recipe.tools.sdk_root())?
            ),
            "-DCMAKE_OSX_DEPLOYMENT_TARGET:STRING=14.0".into(),
            "-DCMAKE_BUILD_TYPE:STRING=Debug".into(),
            format!("-DMADB_CANDIDATE_ROOT:PATH={}", text(candidate.path())?),
            format!("-DMADB_LIBRARY_FILE:FILEPATH={}", text(&library)?),
        ];
        let cmake = self.recipe.tools.tool("cmake")?.clone();
        self.process("cmake-configure", &cmake, arguments, None)?;
        self.process(
            "cmake-build",
            &cmake,
            vec![
                "--build".into(),
                text(&build)?,
                "--target".into(),
                "madb_demo".into(),
                "--parallel".into(),
                "1".into(),
            ],
            None,
        )?;
        for name in ["CMakeCache.txt", "build.ninja", "CMakeFiles/rules.ninja"] {
            let (_, bytes) = self
                .filesystem
                .freeze_generated(&path(&format!(".madb/build/cmake/{name}")), 4 * 1024 * 1024)?;
            self.log("cmake-build", &format!("configuration/{name}"), &bytes)?;
        }
        let (consumer, _) = self
            .filesystem
            .freeze_generated(&path(".madb/build/cmake/madb_demo"), 256 * 1024 * 1024)?;
        self.log(
            "cmake-build",
            "consumer-identity.txt",
            format!(
                "path={}\nsha256={}\nsize={}\n",
                consumer.path.display(),
                consumer.digest.as_str(),
                consumer.size
            )
            .as_bytes(),
        )?;
        self.consumer = Some(consumer);
        candidate
            .files()
            .verify(&LocalWorkspace::open(candidate.path())?)?;
        Ok(())
    }
    fn ordinary_run(&mut self) -> AttemptResult<()> {
        let candidate = self.candidate()?;
        let file = self
            .consumer
            .clone()
            .ok_or_else(|| invalid("consumer", "no executable was frozen by this CMake build"))?;
        file.verify()?;
        let tool = LockedInitializationTool {
            observation: ToolObservation {
                id: Id::new("i01-ordinary-consumer").expect("fixed id"),
                version: "0.1.0".into(),
                sha256: file.digest.clone(),
                host: Id::new("aarch64-apple-darwin").unwrap(),
                target: Id::new("aarch64-apple-darwin").unwrap(),
                source: "generated://final-project/cmake/madb_demo".into(),
                arguments: Vec::new(),
            },
            file,
        };
        self.extra_tools.push(tool.observation.clone());
        self.process("ordinary-consumer", &tool, Vec::new(), Some(b"7:x\n"))?;
        candidate
            .files()
            .verify(&LocalWorkspace::open(candidate.path())?)?;
        Ok(())
    }
    fn helper(&mut self, stage: &str) -> AttemptResult<()> {
        let native = self.filesystem.native_root();
        let root = self.filesystem.root();
        let mut args = vec![
            "-I".into(),
            "-S".into(),
            "-B".into(),
            "-X".into(),
            "pycache_prefix=/dev/null".into(),
            text(&native.join("adapters/toolchains/rust_c/init_values.py"))?,
            stage.into(),
            "--workspace".into(),
            text(&native)?,
            "--profile".into(),
            text(&root.join(".madb/template-input/profile.json"))?,
            "--sdk".into(),
            text(&root.join(".madb/template-input/toolchain.json"))?,
            "--author".into(),
            text(&root.join("rust/src/lib.rs"))?,
            "--namespace".into(),
            self.namespace.into(),
            "--observation".into(),
            text(&native.join("installation.json"))?,
            "--host".into(),
            text(self.tool.image_path())?,
            "--host-sha256".into(),
            self.tool.identity().sha256.as_str().into(),
            "--host-size".into(),
            self.tool.image_size().to_string(),
            "--producer-revision".into(),
            self.recipe.revision.into(),
        ];
        self.verify_batches()?;
        for (lease, metadata) in &self.batches {
            let flag = match lease.info().spec.layer {
                Layer::Source => "--source-metadata-sha256",
                Layer::Build => "--build-metadata-sha256",
                Layer::Candidate => continue,
            };
            args.extend([flag.into(), metadata.as_str().into()]);
        }
        let expected = match stage {
            "prepare" => "metadata-execution.json\n",
            "generate-plan" => "generate-execution.json\n",
            "build-plan" => "build-execution.json\n",
            "package-plan" => "package-execution.json\n",
            _ => unreachable!("fixed helper stage"),
        };
        let python = self.recipe.tools.tool("python")?.clone();
        self.process(
            stage,
            &python,
            std::mem::take(&mut args),
            Some(expected.as_bytes()),
        )
    }
    fn c04(&mut self, operation: &str) -> AttemptResult<Option<Vec<u8>>> {
        self.control.checkpoint()?;
        self.verify_batches()?;
        self.filesystem.verify_initial()?;
        self.recipe.tools.revalidate()?;
        self.tool.revalidate()?;
        let stage = self.control.pipeline()?;
        let options = ExecutionOptions {
            command: if operation == "metadata" {
                "build"
            } else {
                operation
            }
            .into(),
            root: self.filesystem.native_root(),
            execution: format!("{operation}-execution.json"),
            tool_root: Some(PathBuf::from("/")),
            process_timeout: Duration::from_secs(300),
            process_termination_grace: Duration::from_secs(2),
            check: false,
            task: (operation == "metadata").then(|| "metadata".into()),
            active_tasks: Vec::new(),
        };
        let output = execution::execute_controlled(&options, stage.pipeline());
        drop(stage);
        match output {
            Ok(output) => {
                self.journal(operation, output.journal)?;
                self.log(operation, "summary.txt", output.summary.as_bytes())?;
                self.verify_batches()?;
                self.filesystem.verify_initial()?;
                self.recipe.tools.revalidate()?;
                self.tool.revalidate()?;
                Ok(output.metadata.map(|(_, bytes)| bytes))
            }
            Err(error) => {
                let cancelled = error.journal.cancelled;
                self.journal(operation, error.journal)?;
                if cancelled {
                    Err(AttemptError::Cancelled)
                } else {
                    Err(error.error.into())
                }
            }
        }
    }
    fn process(
        &mut self,
        label: &str,
        tool: &LockedInitializationTool,
        args: Vec<String>,
        expected: Option<&[u8]>,
    ) -> AttemptResult<()> {
        self.control.checkpoint()?;
        self.verify_batches()?;
        self.recipe.tools.revalidate()?;
        self.tool.revalidate()?;
        let stage = self.control.process()?;
        let env = if label == "ordinary-consumer" {
            BTreeMap::new()
        } else {
            self.recipe.tools.environment()
        };
        let policy = ProcessPolicy {
            timeout: Duration::from_secs(300),
            termination_grace: Duration::from_secs(2),
            allowed_environment: env.keys().cloned().collect(),
            ..ProcessPolicy::default()
        };
        let started = std::time::Instant::now();
        let output = self.filesystem.run(
            InitializationProcess {
                tool,
                arguments: args.clone(),
                environment: env.clone(),
                policy,
            },
            stage.process(),
            |run| {
                self.verify_batches()?;
                self.recipe.tools.revalidate()?;
                self.tool.revalidate()?;
                if expected.is_some_and(|bytes| bytes != run.report.stdout.bytes) {
                    return Err(QueryError::new(
                        ErrorKind::CheckFailed,
                        "EVD001",
                        label,
                        "actual process stdout differs from the fixed finite recipe expectation",
                    ));
                }
                Ok(())
            },
        );
        drop(stage);
        let output = output?;
        let admission = output.execution.admission;
        self.retained
            .push(RetainedExecution::Process(output.execution));
        let RetainedExecution::Process(execution) =
            self.retained.last().expect("process owner retained")
        else {
            unreachable!("inserted process")
        };
        let mut message = format!(
            "tool={} version={} sha256={}\nargv={:?}\nenvironment={:?}\nadmission={:?}\nwall_seconds={}\n",
            tool.observation.id,
            tool.observation.version,
            tool.file.digest.as_str(),
            args,
            env,
            admission,
            started.elapsed().as_secs_f64()
        );
        let mut streams = None;
        if let Some(process) = &execution.process {
            let clean =
                process.pending_cleanup.is_none() && process.report.cleanup.locally_complete();
            self.cleanup = Some(self.cleanup.unwrap_or(true) && clean);
            message.push_str(&format!("termination={:?}\ncleanup={:?}\npending_cleanup={}\nstdout_observed={} stdout_truncated={}\nstderr_observed={} stderr_truncated={}\ndiagnostic={:?}\n",process.report.termination,process.report.cleanup,process.pending_cleanup.is_some(),process.report.stdout.observed_bytes,process.report.stdout.truncated(),process.report.stderr.observed_bytes,process.report.stderr.truncated(),process.report.diagnostic));
            streams = Some((
                process.report.stdout.bytes.clone(),
                process.report.stderr.bytes.clone(),
            ));
        }
        if let Some((stdout, stderr)) = streams {
            self.log(label, "stdout", &stdout)?;
            self.log(label, "stderr", &stderr)?;
        }
        self.log(label, "process.txt", message.as_bytes())?;
        if let Some(error) = output.validation_error {
            return Err(error.into());
        }
        if admission == ResultAdmission::RejectedByCancellation {
            return Err(AttemptError::Cancelled);
        }
        if admission != ResultAdmission::EligibleForValidation {
            return Err(QueryError::new(ErrorKind::CheckFailed,"EXEC010",label,"process failed; inspect actual termination, bounded output and cleanup attachments").into());
        }
        Ok(())
    }
    fn journal(&mut self, label: &str, mut journal: ExecutionJournal) -> AttemptResult<()> {
        self.retained.append(&mut journal.retained);
        if let Some((instance, batch)) = journal.committed.as_deref() {
            let store =
                LocalBatchStore::open_readonly(&self.filesystem.native_root().join("domain"))?
                    .ok_or_else(|| invalid("commit", "committed domain is absent"))?;
            let lease = store
                .pin(batch.spec().layer)?
                .ok_or_else(|| invalid("commit", "committed layer is absent"))?;
            if lease.info().id != *instance
                || lease.info().content_id != *batch.id()
                || lease.info().spec != *batch.spec()
                || lease.info().files != batch.files().references().cloned().collect::<Vec<_>>()
            {
                return Err(QueryError::new(
                    ErrorKind::Conflict,
                    "STORE004",
                    "commit",
                    "stored batch differs from this C04 publish receipt",
                )
                .into());
            }
            let metadata = read_metadata(&lease)?;
            self.log(label, "committed-metadata.json", &metadata)?;
            self.batches.push((lease, Digest::of_bytes(&metadata)));
        }
        if let Some(clean) = journal.cleanup_complete {
            self.cleanup = Some(self.cleanup.unwrap_or(true) && clean);
        }
        for (name, bytes) in journal.files {
            self.log(label, &name, &bytes)?;
        }
        Ok(())
    }
    fn log(&mut self, label: &str, name: &str, bytes: &[u8]) -> Result<()> {
        let path = format!("execution/{:04}-{label}/{name}", self.sequence);
        self.sequence += 1;
        let file = evidence::save_file(self.store, &path, bytes)?;
        self.observations.logs.push(file);
        Ok(())
    }
    fn verify_batches(&self) -> Result<()> {
        if self.batches.is_empty() {
            return Ok(());
        }
        let store = LocalBatchStore::open_readonly(&self.filesystem.native_root().join("domain"))?
            .ok_or_else(|| invalid("commit", "committed domain is absent"))?;
        for (original, metadata) in &self.batches {
            let current = store
                .pin(original.info().spec.layer)?
                .ok_or_else(|| invalid("commit", "committed layer disappeared"))?;
            if current.info() != original.info()
                || Digest::of_bytes(&read_metadata(&current)?) != *metadata
            {
                return Err(QueryError::new(
                    ErrorKind::Conflict,
                    "STORE004",
                    "commit",
                    "current changed from this initializer's C04 receipt",
                ));
            }
            original
                .files()
                .verify(&LocalWorkspace::open(original.path())?)?;
        }
        Ok(())
    }
    fn save_layer(&mut self, layer: Layer, label: &str) -> AttemptResult<()> {
        let store = LocalBatchStore::open_readonly(&self.filesystem.native_root().join("domain"))?
            .ok_or_else(|| invalid("C04", "missing committed domain"))?;
        let lease = store
            .pin(layer)?
            .ok_or_else(|| invalid("C04", "expected committed layer is absent"))?;
        self.log(
            label,
            "identity.txt",
            format!(
                "batch={}\nroot={}\nspec={:?}\n",
                lease.info().id.as_str(),
                lease.path().display(),
                lease.info().spec
            )
            .as_bytes(),
        )?;
        for (file, bytes) in lease.files().files() {
            if file.path.as_str().ends_with(".json")
                || file.path.as_str().ends_with(".log")
                || file.path.as_str().ends_with(".txt")
                || file.path.as_str().ends_with(".stderr")
                || file.path.as_str().ends_with(".stdout")
            {
                self.log(label, &format!("files/{}", file.path.as_str()), bytes)?;
            }
        }
        Ok(())
    }
    fn candidate(&self) -> Result<BatchLease> {
        self.verify_batches()?;
        let store = LocalBatchStore::open_readonly(&self.filesystem.native_root().join("domain"))?
            .ok_or_else(|| invalid("candidate", "missing C04 domain"))?;
        let lease = store
            .pin(Layer::Candidate)?
            .ok_or_else(|| invalid("candidate", "missing committed SDK candidate"))?;
        let (_, bytes) = lease
            .files()
            .files()
            .find(|(file, _)| file.path.as_str() == "candidate.json")
            .ok_or_else(|| invalid("candidate", "missing candidate manifest"))?;
        let document = SchemaRegistry::new()
            .map_err(|e| invalid("candidate", e.to_string()))?
            .decode(DocumentKind::Candidate, bytes)
            .map_err(|e| invalid("candidate", e.to_string()))?;
        let provided = lease
            .files()
            .files()
            .filter(|(file, _)| file.path.as_str() != "candidate.json")
            .map(|(file, bytes)| ProvidedFile {
                path: file.path.as_str(),
                bytes,
            })
            .collect::<Vec<_>>();
        verify_files(&document, &provided).map_err(|e| invalid("candidate", e.to_string()))?;
        Ok(lease)
    }
    fn record_check(
        &mut self,
        id: &str,
        level: EvidenceLevel,
        result: &AttemptResult<()>,
    ) -> Result<()> {
        let requirement = self
            .observations_required()?
            .into_iter()
            .find(|r| r.id.as_str() == id)
            .ok_or_else(|| invalid(id, "fixed check is not independently required"))?;
        let diagnostics = match result {
            Ok(_) => Vec::new(),
            Err(AttemptError::Cancelled) => vec![Diagnostic {
                code: Id::new("EXEC001").unwrap(),
                message: "The active C04 stage rejected result admission by cancellation.".into(),
                location: None,
            }],
            Err(AttemptError::Stage(error)) => evidence::bounded_error(QueryError {
                kind: error.kind,
                findings: error.findings.clone(),
            })
            .findings
            .into_iter()
            .map(|f| Diagnostic {
                code: Id::new(f.code).unwrap_or_else(|_| Id::new("EXEC010").unwrap()),
                message: f.message,
                location: f.locations.into_iter().next(),
            })
            .collect(),
        };
        self.observations.checks.push(CheckObservation {
            id: requirement.id,
            level: requirement.level,
            target: requirement.target,
            status: if result.is_ok() {
                ExecutionStatus::Passed
            } else {
                ExecutionStatus::Failed
            },
            requirement: Applicability::Required,
            environment: self
                .recipe
                .tools
                .observations()
                .into_iter()
                .chain(self.extra_tools.clone())
                .collect(),
            command: None,
            evidence_level: result.is_ok().then_some(level),
            cleanup: match self.cleanup {
                Some(true) => Cleanup::Complete,
                Some(false) => Cleanup::Failed,
                None => Cleanup::NotRun,
            },
            diagnostics,
            attachments: self.observations.logs.clone(),
            reason: None,
        });
        Ok(())
    }
    fn observations_required(&self) -> Result<Vec<madb_plan::project::RequiredCheck>> {
        // The frozen request remains the only authority for selected requirements.
        self.request
            .selected()
            .required_checks(match self.request.projection().verify {
                VerifyLevel::Structure => CheckLevel::Structure,
                VerifyLevel::Build => CheckLevel::Build,
                VerifyLevel::Run => CheckLevel::Run,
            })
            .map_err(|e| invalid(&e.subject, e.message))
    }
    fn block_remaining(&mut self, failed: &str) -> Result<()> {
        for r in self.observations_required()? {
            if !self
                .observations
                .checks
                .iter()
                .any(|c| c.id == r.id && c.target == r.target)
            {
                self.observations.checks.push(CheckObservation {
                    id: r.id,
                    level: r.level,
                    target: r.target,
                    status: ExecutionStatus::Blocked,
                    requirement: Applicability::Required,
                    environment: Vec::new(),
                    command: None,
                    evidence_level: None,
                    cleanup: Cleanup::NotRun,
                    diagnostics: Vec::new(),
                    attachments: Vec::new(),
                    reason: Some(format!("required predecessor {failed} did not pass")),
                });
            }
        }
        Ok(())
    }
}
fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).expect("fixed recipe path")
}
fn text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("path", "finite recipe paths must be UTF-8"))
}

fn read_metadata(lease: &BatchLease) -> Result<Vec<u8>> {
    LocalWorkspace::open(lease.path().parent().expect("batch files parent"))?
        .read(&path("metadata.json"), 8 * 1024 * 1024)?
        .ok_or_else(|| invalid("metadata", "committed metadata is absent"))
}
