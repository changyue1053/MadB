//! The single installed values recipe. Helpers prepare C04 documents; only the
//! existing in-process C04 coordinator commits generated/build/candidate state.
use super::{control::BuildControl, evidence, invalid};
use crate::bootstrap::execution::{self, ExecutionJournal, ExecutionOptions, RetainedExecution};
use madb_app::ports::ReadWorkspace;
use madb_app::transaction::Layer;
use madb_app::use_cases::project_build::*;
use madb_app::use_cases::project_build::{
    BuildFailure as AttemptError, BuildResult as AttemptResult,
};
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    process::{ProcessPolicy, ResultAdmission},
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
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct NativeExecution<'a> {
    pub control: &'a BuildControl,
    pub store: &'a mut LocalInitEvidence,
    pub retained: &'a mut Vec<RetainedExecution>,
    tool: Option<FrozenInitializationTool>,
    recipe: Option<ReadyRecipe>,
}
struct ReadyRecipe {
    tools: InstalledValuesTools,
    revision: &'static str,
}
impl<'a> NativeExecution<'a> {
    pub fn new(
        control: &'a BuildControl,
        store: &'a mut LocalInitEvidence,
        retained: &'a mut Vec<RetainedExecution>,
    ) -> Self {
        Self {
            control,
            store,
            retained,
            tool: None,
            recipe: None,
        }
    }
}
impl ProjectBuildExecution for NativeExecution<'_> {
    fn preflight(&mut self, operation: Operation, descriptor: &Descriptor) -> Result<()> {
        super::bootstrap_check()?;
        let tool = FrozenInitializationTool::capture_project(env!("CARGO_PKG_VERSION"))?;
        let revision = super::source_revision()?;
        let expected = &descriptor.tool;
        if expected.id != tool.identity().id
            || expected.version != tool.identity().version
            || expected.sha256 != tool.identity().sha256
            || expected.size != tool.image_size()
            || expected.source_revision != revision
        {
            return Err(QueryError::new(
                ErrorKind::Conflict,
                "STORE003",
                "tool",
                "actual running image/version/source differs from the original project descriptor",
            ));
        }
        self.tool = Some(tool);
        if operation == Operation::Build {
            self.recipe = Some(ReadyRecipe {
                tools: InstalledValuesTools::capture()?,
                revision,
            });
        }
        Ok(())
    }
    fn execute(
        &mut self,
        descriptor: &Descriptor,
        attempt: &BuildAttempt,
        snapshot: &Digest,
        report: &mut ExecutionProgress<'_>,
    ) -> AttemptResult<ProjectOutput> {
        let recipe = self
            .recipe
            .as_ref()
            .ok_or_else(|| invalid("recipe", "build preflight was not completed"))?;
        let tool = self
            .tool
            .as_ref()
            .ok_or_else(|| invalid("tool", "actual tool is absent"))?;
        let initial = attempt
            .inputs
            .files
            .iter()
            .map(|f| madb_app::ports::FileReference {
                path: f.file.path.clone(),
                digest: f.file.sha256.clone(),
                size: f.file.size,
            })
            .collect();
        let filesystem = FinalProjectExecution::open_frozen(&attempt.root, initial)?;
        filesystem.materialize_project_runtime()?;
        filesystem.write_native_new(
            &path("installation.json"),
            recipe.tools.native_observation(),
        )?;
        filesystem.write_native_new(
            &path("execution-resources.json"),
            recipe.tools.execution_resources(),
        )?;
        let mut state = RunState {
            recipe,
            filesystem,
            tool,
            control: self.control,
            report,
            store: self.store,
            namespace: &descriptor.namespace,
            sequence: 0,
            retained: self.retained,
            cleanup: None,
            batches: Vec::new(),
            consumer: None,
        };
        state.log(
            "strategy",
            "rebuild.txt",
            b"reason=conservative-full-build\ncache_hit=false\n",
        )?;
        state.helper("prepare")?;
        state.report.stage_passed(StageId::Prepare);
        let metadata = state
            .c04("metadata")?
            .ok_or_else(|| invalid("metadata", "C04 returned no verified contract bytes"))?;
        state
            .filesystem
            .write_native_new(&path("contract.json"), &metadata)?;
        state.report.stage_passed(StageId::Metadata);
        state.helper("generate-plan")?;
        state.c04("generate")?;
        state.save_layer(Layer::Source, "source")?;
        state.report.stage_passed(StageId::Generate);
        state.helper("build-plan")?;
        state.c04("build")?;
        state.save_layer(Layer::Build, "build")?;
        state.report.stage_passed(StageId::Build);
        state.helper("package-plan")?;
        state.c04("package")?;
        state.save_layer(Layer::Candidate, "candidate")?;
        state.candidate()?;
        state.report.stage_passed(StageId::Package);
        state.consumer_link()?;
        state.report.stage_passed(StageId::ConsumerLink);
        let candidate = state.candidate()?;
        let relative = candidate
            .path()
            .strip_prefix(state.filesystem.root())
            .map_err(|_| invalid("candidate", "candidate escaped its owned attempt"))?;
        let root = path(&text(relative)?);
        let mut outputs = candidate
            .files()
            .references()
            .map(|f| FileIdentity {
                path: path(&format!("{}/{}", root.as_str(), f.path.as_str())),
                sha256: f.digest.clone(),
                size: f.size,
            })
            .collect::<Vec<_>>();
        let manifest = outputs
            .iter()
            .find(|f| f.path.as_str() == format!("{}/candidate.json", root.as_str()))
            .cloned()
            .ok_or_else(|| invalid("candidate", "manifest absent"))?;
        let consumer = state
            .consumer
            .as_ref()
            .ok_or_else(|| invalid("consumer", "consumer absent"))?;
        let consumer = FileIdentity {
            path: path("native/consumer-build/madb_demo"),
            sha256: consumer.digest.clone(),
            size: consumer.size,
        };
        outputs.push(consumer.clone());
        outputs.sort_by(|a, b| {
            a.path
                .as_str()
                .encode_utf16()
                .cmp(b.path.as_str().encode_utf16())
        });
        let projection = OutputProjection {
            snapshot_digest: snapshot.clone(),
            candidate: CandidateOutput { root, manifest },
            consumer,
            outputs,
        };
        let identity = madb_contracts::project_build::ProjectBuildAdapter::new()?
            .output_digest(&descriptor.tool, &projection)?;
        state.verify_batches()?;
        state.filesystem.verify_initial()?;
        recipe.tools.revalidate()?;
        tool.revalidate()?;
        state.control.checkpoint()?;
        Ok(ProjectOutput {
            identity,
            projection,
        })
    }
}
struct RunState<'a, 'report> {
    recipe: &'a ReadyRecipe,
    filesystem: FinalProjectExecution,
    tool: &'a FrozenInitializationTool,
    control: &'a BuildControl,
    report: &'a mut ExecutionProgress<'report>,
    store: &'a mut LocalInitEvidence,
    namespace: &'a str,
    sequence: usize,
    retained: &'a mut Vec<RetainedExecution>,
    cleanup: Option<bool>,
    batches: Vec<(BatchLease, Digest)>,
    consumer: Option<LockedInitializationFile>,
}
impl RunState<'_, '_> {
    fn consumer_link(&mut self) -> AttemptResult<()> {
        let candidate = self.candidate()?;
        self.save_layer(Layer::Candidate, "candidate")?;
        let root = self.filesystem.root().to_path_buf();
        let build = self.filesystem.native_root().join("consumer-build");
        self.filesystem
            .write_native_new(&path("consumer/CMakeLists.txt"), CONSUMER_CMAKE.as_bytes())?;
        let library = candidate
            .path()
            .join(format!("lib/lib{}.dylib", self.namespace));
        let arguments = vec![
            "-S".into(),
            text(&self.filesystem.native_root().join("consumer"))?,
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
            format!(
                "-DMADB_CONSUMER_SOURCE:FILEPATH={}",
                text(&root.join("inputs/consumer/main.c"))?
            ),
            format!(
                "-DMADB_CONSUMER_INCLUDE:PATH={}",
                text(&root.join("inputs/consumer/include"))?
            ),
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
            let (_, bytes) = self.filesystem.freeze_generated(
                &path(&format!("native/consumer-build/{name}")),
                4 * 1024 * 1024,
            )?;
            self.log("cmake-build", &format!("configuration/{name}"), &bytes)?;
        }
        let (consumer, _) = self
            .filesystem
            .freeze_generated(&path("native/consumer-build/madb_demo"), 256 * 1024 * 1024)?;
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
    fn helper(&mut self, stage: &str) -> AttemptResult<()> {
        let native = self.filesystem.native_root();
        let root = self.filesystem.root();
        let mut args = vec![
            "-I".into(),
            "-S".into(),
            "-B".into(),
            "-X".into(),
            "pycache_prefix=/dev/null".into(),
            text(&native.join("adapters/toolchains/rust_c/project_build_values.py"))?,
            stage.into(),
            "--workspace".into(),
            text(&native)?,
            "--profile".into(),
            text(&root.join("inputs/.madb/template-input/profile.json"))?,
            "--sdk".into(),
            text(&root.join("inputs/.madb/template-input/toolchain.json"))?,
            "--author".into(),
            text(&root.join("inputs/rust/src/lib.rs"))?,
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
        self.report.record_log(file)?;
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

const CONSUMER_CMAKE: &str = r#"cmake_minimum_required(VERSION 4.4.4)
project(madb_project_consumer LANGUAGES C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
if(NOT DEFINED MADB_CANDIDATE_ROOT OR NOT DEFINED MADB_LIBRARY_FILE)
  message(FATAL_ERROR "Run the checked MadB recipe before configuring")
endif()
if(NOT IS_ABSOLUTE "${MADB_CANDIDATE_ROOT}" OR NOT IS_ABSOLUTE "${MADB_LIBRARY_FILE}")
  message(FATAL_ERROR "The checked recipe must provide absolute candidate paths")
endif()
if(NOT IS_DIRECTORY "${MADB_CANDIDATE_ROOT}/include" OR NOT EXISTS "${MADB_LIBRARY_FILE}")
  message(FATAL_ERROR "The checked candidate headers and library must exist")
endif()
get_filename_component(MADB_LIBRARY_DIRECTORY "${MADB_LIBRARY_FILE}" DIRECTORY)
add_library(madb_sdk SHARED IMPORTED GLOBAL)
set_target_properties(madb_sdk PROPERTIES IMPORTED_LOCATION "${MADB_LIBRARY_FILE}" INTERFACE_INCLUDE_DIRECTORIES "${MADB_CANDIDATE_ROOT}/include")
add_executable(madb_demo "${MADB_CONSUMER_SOURCE}")
if(IS_DIRECTORY "${MADB_CONSUMER_INCLUDE}")
  target_include_directories(madb_demo PRIVATE "${MADB_CONSUMER_INCLUDE}")
endif()
target_link_libraries(madb_demo PRIVATE madb_sdk)
set_target_properties(madb_demo PROPERTIES BUILD_RPATH "${MADB_LIBRARY_DIRECTORY}")
"#;
