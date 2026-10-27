//! Finite typed /2 build root. Each next task binds preceding actual bytes.
use super::{
    Result,
    inputs::{self, Inputs, actual_reference, path},
    recipe,
};
use madb_app::{
    Digest, LogicalPath,
    dag::{GraphSequence, GraphStatus},
    inputs::{FrozenInputs, capture_inputs},
    pipeline::{PipelineControl, PipelineParents},
    ports::{FileReference, ReadWorkspace},
    process::{ProcessExecution, ProcessPolicy, ProcessResult},
    transaction::Layer,
};
use madb_contracts::{
    DocumentKind,
    composition::build::{assemble_build_files, bind_build_graph},
    execution,
    task::TaskBinding,
};
use madb_host::{LocalWorkspace, dag::LocalGraphExecutor, transaction::LocalBatchStore};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};

#[derive(Default)]
pub(super) struct Memory(pub BTreeMap<LogicalPath, Vec<u8>>);
impl Memory {
    pub fn insert(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        let name = LogicalPath::new(name)?;
        if self.0.get(&name).is_some_and(|old| old != bytes) {
            return Err("conflicting frozen input path".into());
        }
        self.0.insert(name, bytes.to_vec());
        Ok(())
    }
    pub fn extend(&mut self, files: &FrozenInputs) -> Result<()> {
        for (r, b) in files.files() {
            self.insert(r.path.as_str(), b)?;
        }
        Ok(())
    }
    pub fn frozen(&self) -> Result<FrozenInputs> {
        Ok(capture_inputs(
            self,
            self.0
                .iter()
                .map(|(p, b)| actual_reference(p.as_str(), b))
                .collect(),
        )?)
    }
}
impl ReadWorkspace for Memory {
    fn read(&self, name: &LogicalPath, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .get(name)
            .map(|b| b[..b.len().min(limit.saturating_add(1))].to_vec()))
    }
}
pub(super) fn ref_json(file: &FileReference) -> Value {
    json!({"path":file.path.as_str(),"sha256":file.digest.as_str(),"size":file.size})
}
pub(super) fn output_paths(phase: &str) -> Vec<&'static str> {
    match phase {
        "compile" => vec!["build/bridge.o", "build/compile.commands.json"],
        "link" => vec![
            "build/native/bridge.dylib",
            "build/native/install.json",
            "build/lib/src/package_pins.dart",
            "build/provenance.json",
            "build/link.commands.json",
        ],
        "run" => vec!["build/run.observation.json", "build/run.commands.json"],
        _ => unreachable!(),
    }
}
pub(super) fn process(value: Option<&ProcessResult<ProcessExecution>>) -> Value {
    match value {
        None => Value::Null,
        Some(Err(e)) => json!({"error":e.to_string()}),
        Some(Ok(e)) => {
            json!({"admission":format!("{:?}",e.admission),"process":e.process.as_ref().map(|p|json!({"termination":format!("{:?}",p.report.termination),"cleanup":{"locally_complete":p.report.cleanup.locally_complete(),"leader_reaped":p.report.cleanup.leader_reaped,"stdout_eof":p.report.cleanup.stdout_eof,"stderr_eof":p.report.cleanup.stderr_eof,"signal_failed":p.report.cleanup.signal_failed},"pending_cleanup":p.pending_cleanup.is_some(),"diagnostic":p.report.diagnostic.as_ref().map(|d|d.to_string()),"stdout":String::from_utf8_lossy(&p.report.stdout.bytes),"stderr":String::from_utf8_lossy(&p.report.stderr.bytes)}))})
        }
    }
}
pub(super) fn policy() -> ProcessPolicy {
    ProcessPolicy {
        timeout: Duration::from_secs(240),
        termination_grace: Duration::from_secs(3),
        cleanup_timeout: Duration::from_secs(5),
        stderr_limit: 1024 * 1024,
        ..ProcessPolicy::default()
    }
}
pub fn run(args: &[String]) -> Result<()> {
    if args.len() != 5 && !(args.len() == 6 && args[5] == "external-tool-drift") {
        return Err("expected build INPUT_ROOT TOOL_ROOT SOURCE_RUN NEW_RUN".into());
    }
    let input = Inputs::load(&inputs::directory(Path::new(&args[1]))?)?;
    let tools = inputs::directory(Path::new(&args[2]))?;
    let source_run = inputs::directory(Path::new(&args[3]))?;
    let run = Path::new(&args[4]);
    fresh(run, &[&input.root, &tools, &source_run])?;
    let bound = input.bind(&recipe::requirements())?;
    let store = LocalBatchStore::open(&source_run.join("store"))?;
    let parents = PipelineParents::capture(&store, Layer::Build)?;
    let source = parents.source().ok_or("source absent")?;
    let source_files = source.files();
    let contract_bytes = source_files
        .files()
        .find(|(r, _)| r.path.as_str() == "declarations.json")
        .ok_or("source declarations absent")?
        .1;
    let contract = input
        .registry
        .decode(DocumentKind::ContractIr, contract_bytes)?;
    let roles = recipe::roles(&input, &bound, &contract)?;
    recipe::check(&input, &bound, &contract)?;
    let python = input.lock.value()["tools"]
        .as_array()
        .ok_or("tools")?
        .iter()
        .find(|t| t["id"] == "e04-python")
        .ok_or("python not locked")?;
    let image = fs::read(tools.join("python"))?;
    let installed = actual_reference("python", &image);
    if python["sha256"] != installed.digest.as_str() {
        return Err("installed Python differs".into());
    }
    let mut memory = Memory::default();
    memory.extend(&input.frozen)?;
    memory.extend(source_files)?;
    memory.insert("profile.json", &input.profile_bytes)?;
    memory.insert("lock.json", &input.lock_bytes)?;
    let before = store.snapshot()?;
    fs::create_dir(run)?;
    let control = PipelineControl::default();
    let _scope = control.scope();
    let mut sequence = GraphSequence::begin(control.graph())?;
    let mut documents = Vec::new();
    let mut results = BTreeMap::new();
    let mut runs = Vec::new();
    let mut reports = Vec::new();
    for phase in ["compile", "link", "run"] {
        let frozen = memory.frozen()?;
        let arguments = vec![
            "-I",
            "-S",
            "-B",
            "-X",
            "pycache_prefix=/dev/null",
            "../inputs/packages/org.madb.dart-ffi/0.1.0/adapters/generators/dart_ffi/build.py",
        ];
        let mut task = json!({"schema":"madb.build-task/1","id":format!("dart-ffi-{phase}"),"selector":contract.value()["selector"],"dependencies":[],"input_digest":contract.value()["source_digest"],"task_digest":Digest::of_bytes(b"pending").as_str(),"generation_id":source.info().spec.generation_id.as_str(),"tool":{"id":python["id"],"version":python["version"],"sha256":python["sha256"]},"arguments":arguments,"environment":[],"working_directory":"work","phase":phase,"inputs":frozen.references().map(ref_json).collect::<Vec<_>>(),"outputs":output_paths(phase)});
        let draft = input
            .registry
            .decode(DocumentKind::BuildTask, &serde_json::to_vec(&task)?)?;
        task["task_digest"] = json!(madb_contracts::task_digest(&draft)?);
        let bytes = serde_json::to_vec(&task)?;
        execution::validate_build_generation(
            std::slice::from_ref(&bytes),
            execution::source_manifest(source_files)?,
            source_files,
        )?;
        let graph = bind_build_graph(
            &bound,
            &input.project,
            &contract,
            &roles,
            vec![TaskBinding {
                task: &bytes,
                installed_tool: installed.clone(),
            }],
            None,
            &[],
        )?;
        fs::write(run.join(format!("{phase}.task.json")), &bytes)?;
        let mut executor =
            LocalGraphExecutor::new(run, &path(phase), &tools, policy()).map_err(|e| e.error)?;
        let step = sequence.run(&graph, &memory, &mut executor)?;
        reports.push(json!({"phase":phase,"status":format!("{:?}",step.status),"failure":step.failure.as_ref().map(|e|e.to_string()),"nodes":step.nodes().iter().map(|(id,node)|json!({"id":id.as_str(),"path":node.path(),"preparation_error":node.preparation_error().map(|e|e.to_string()),"execution":process(node.execution())})).collect::<Vec<_>>()}));
        fs::write(
            run.join("stages.json"),
            serde_json::to_vec_pretty(&reports)?,
        )?;
        if let Some(values) = step.results() {
            for (id, value) in values {
                for (r, b) in value.outputs() {
                    memory.insert(r.path.as_str(), b)?;
                }
                results.insert(id.clone(), value.clone());
            }
        }
        let okay = step.status == GraphStatus::Validated;
        runs.push(step);
        documents.push(bytes);
        if !okay {
            break;
        }
    }
    let status = sequence.finish();
    let mut committed = false;
    let mut failure = None;
    if status == GraphStatus::Validated {
        let artifacts = assemble_build_files(
            &bound,
            &input.project,
            &contract,
            &roles,
            &documents,
            &results,
        )?;
        let batch = parents.build_batch(artifacts.identity, artifacts.files)?;
        if args.len() == 6 {
            inputs::corrupt_test_tool(&input, run)?;
        }
        match parents.publish(&store, &batch, &control, || {
            input.verify().map_err(|e| {
                madb_app::QueryError::new(
                    madb_app::ErrorKind::InvalidInput,
                    "INPUT003",
                    "context",
                    e.to_string(),
                )
            })?;
            capture_inputs(&LocalWorkspace::open(&tools)?, vec![installed.clone()])?;
            Ok(())
        }) {
            Ok(_) => committed = true,
            Err(e) => failure = Some(e.to_string()),
        }
    }
    let after = store.snapshot()?;
    let value = json!({"scope":"typed profile/2 real compile/link/run and atomic Build","status":format!("{:?}",status),"committed":committed,"index_unchanged":before==after,"source_batch":after.source.as_ref().map(Digest::as_str),"build_batch":after.build.as_ref().map(Digest::as_str),"failure":failure,"stages":reports,"not_claimed":["Package","independent Lab","whole E04"]});
    fs::write(run.join("result.json"), serde_json::to_vec_pretty(&value)?)?;
    println!("{value}");
    drop(runs);
    Ok(())
}
pub(super) fn fresh(run: &Path, protected: &[&Path]) -> Result<()> {
    if !run.is_absolute()
        || run.exists()
        || run.parent().ok_or("parent")?.canonicalize()? != run.parent().ok_or("parent")?
        || protected
            .iter()
            .any(|p| run.starts_with(p) || p.starts_with(run))
    {
        return Err("new output overlaps input or is not canonical".into());
    }
    Ok(())
}
