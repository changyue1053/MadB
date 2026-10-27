//! Fixed ordinary /2 generation session and Source transaction development root.
//! Not a product CLI command, SDK consumer, or whole-composition certification.
#[allow(dead_code)]
#[path = "composition_generation_probe.rs"]
mod baseline;
use baseline::{Inputs, actual_reference, id, path};
use madb_app::{
    Digest,
    inputs::capture_inputs,
    pipeline::{PipelineControl, PipelineParents, capture_artifact_result},
    ports::FileReference,
    process::{ExecutionControl, ProcessError, ProcessPolicy, execute_process_validated},
    transaction::{Batch, BatchSpec, CommitControl, Layer},
};
use madb_contracts::{
    Document, DocumentKind,
    artifacts::ProvidedFile,
    composition::{
        GenerationSchemaPaths,
        admission::{CompositionImageRequest, check_composition_image},
        execution::{BoundCompositionGeneration, CompositionGenerationRequest},
    },
    execution::{ExecutionRecipe, ExecutionTool, generation_task_digest},
    extension_files::{BoundExtensionFiles, FrozenExtensionResult},
    extension_session::{HostSession, SessionOptions},
};
use madb_host::{
    LocalWorkspace,
    extension_files::LocalExtensionWorkspace,
    process::{LocalProcessRunner, session::InteractiveProcessRunner},
    transaction::LocalBatchStore,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, fs, path::Path, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ENTRY: &str = "crates/madb-cli/examples/arkts_composition_peer.rs";
const TOOL: &str = "madb-arkts-composition-peer";
const SOURCE: &str = "local-fixed-composition-source";
const OUTPUT: &str = "outputs/request-1";

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err(
            "expected task|admit INPUT_ROOT TOOL_ROOT or run INPUT_ROOT TOOL_ROOT NEW_RUN MODE"
                .into(),
        );
    }
    let input = Inputs::load(&baseline::directory(Path::new(&args[2]))?)?;
    let tools = baseline::directory(Path::new(&args[3]))?;
    let mut request_bytes = input.read("generate-request.json", baseline::DOCUMENT_LIMIT)?;
    let request = input
        .registry
        .decode(DocumentKind::GenerateRequest, &request_bytes)?;
    let contract_bytes = input.read("contract.json", baseline::DOCUMENT_LIMIT)?;
    let (mut recipe, image) = execution(&input, &request, &request_bytes, &contract_bytes, &tools)?;
    if args[1] == "task" {
        println!(
            "{}",
            generation_task_digest(&request, &projection(&recipe)?)?
        );
        return Ok(());
    }
    let bound = input.bind(&baseline::recipe())?;
    let contract = input
        .registry
        .decode(DocumentKind::ContractIr, &contract_bytes)?;
    let roles = baseline::required_roles(&input, &bound, &contract)?;
    let mut feature_names = contract.value()["required_features"]
        .as_array()
        .ok_or("features")?
        .iter()
        .map(|f| f.as_str().ok_or("feature").map(str::to_owned))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    feature_names.push("core.extension-composition@1".into());
    feature_names.sort();
    let feature_refs = feature_names.iter().map(String::as_str).collect::<Vec<_>>();
    let policy = ProcessPolicy {
        timeout: Duration::from_secs(180),
        termination_grace: Duration::from_secs(3),
        cleanup_timeout: Duration::from_secs(5),
        ..ProcessPolicy::default()
    };
    verify_source(&input)?;
    let pins = check_composition_image(
        &bound,
        CompositionImageRequest {
            entrypoint_source: ENTRY,
            component_id: baseline::COMPONENT,
            recipe: &recipe,
            installed_image: ProvidedFile {
                path: "arkts-peer",
                bytes: &image,
            },
            observed_host: "aarch64-apple-darwin",
            observed_source: SOURCE,
            required_features: &feature_refs,
            policy: &policy,
        },
    )?;
    // Coherently re-pin a different logical tool using the same image. The
    // already checked pins must reject this before any process starts.
    if args[1] == "reject-tool-alias" {
        recipe.tool.id = id("unobserved-peer-alias");
        recipe.tool.version = "9.9.9".into();
        let mut value = request.value().clone();
        value["task_digest"] = json!(generation_task_digest(&request, &projection(&recipe)?)?);
        request_bytes = serde_json::to_vec(&value)?;
        recipe.request = actual_reference("generate-request.json", &request_bytes);
    }
    let proof = BoundCompositionGeneration::new(
        bound,
        CompositionGenerationRequest {
            project: &input.project,
            recipe: recipe.clone(),
            pins,
            producer: id(baseline::COMPONENT),
            slots: vec![id(baseline::SLOT)],
            output_root: path(OUTPUT),
            schemas: GenerationSchemaPaths {
                generate_request: path("schemas/generate-request.schema.json"),
                output_set: path("schemas/output-set.schema.json"),
            },
            request: ProvidedFile {
                path: "generate-request.json",
                bytes: &request_bytes,
            },
            lock: ProvidedFile {
                path: "lock.json",
                bytes: &input.lock_bytes,
            },
            profile: ProvidedFile {
                path: "profile.json",
                bytes: &input.profile_bytes,
            },
            contract: ProvidedFile {
                path: "contract.json",
                bytes: &contract_bytes,
            },
            installed_image: ProvidedFile {
                path: "arkts-peer",
                bytes: &image,
            },
            required_responsibilities: &roles,
        },
    );
    if args[1] == "reject-tool-alias" {
        match proof {
            Err(error) if error.code == "ExtensionIdentityMismatch" && error.path == "recipe" => {
                println!(
                    "{}",
                    json!({"rejected":true,"code":error.code,"path":error.path})
                );
                return Ok(());
            }
            Err(error) => return Err(format!("wrong alias rejection: {error}").into()),
            Ok(_) => return Err("checked pins accepted an unobserved tool alias".into()),
        }
    }
    let proof = proof?;
    let mut references = recipe.inputs.clone();
    references.push(recipe.request.clone());
    let snapshot = capture_inputs(&input.workspace, references)?;
    let input_ref = json!({"path":"inputs/generate-request.json", "schema":"madb.generate-request/1",
        "sha256":Digest::of_bytes(&request_bytes).as_str(), "size":request_bytes.len()});
    let files = BoundExtensionFiles::from_composition_recipe(&input_ref, snapshot.clone(), proof)?;
    if args[1] == "admit" {
        let pins = files
            .composition_recipe()
            .ok_or("composition proof")?
            .pins();
        let legacy = HostSession::new(
            pins.manifest(),
            &files,
            SessionOptions {
                component_id: "generator",
                session_id: "legacy-rejection",
                request_id: "legacy-request",
                protocol: "1.0",
                ir: "1.0",
                required_features: &["core.values@1"],
                handshake_timeout: Duration::from_secs(10),
                task_timeout: Duration::from_secs(30),
            },
            |_| Ok(()),
        );
        if !legacy.is_err_and(|e| e.code == "CapabilityUnsupported") {
            return Err("legacy constructor accepted a composition proof".into());
        }
        println!(
            "{}",
            json!({"admitted":true,"profile_digest":request.value()["selector"]["profile_digest"],"source_digest":request.value()["component"]["sha256"],"image_digest":recipe.installed_tool.digest.as_str()})
        );
        return Ok(());
    }
    if args.len() != 6 || args[1] != "run" {
        return Err("invalid command arguments".into());
    }
    let run = Path::new(&args[4]);
    fs::create_dir(run)?;
    let mode = args[5].as_str();
    if !matches!(
        mode,
        "ok" | "cancel-before" | "cancel-result" | "cancel-commit" | "fail-output"
    ) {
        return Err("unknown test mode".into());
    }
    let store = LocalBatchStore::open(&run.join("store"))?;
    fs::create_dir(run.join("baseline"))?;
    fs::write(
        run.join("baseline/previous.txt"),
        b"previous complete source",
    )?;
    let previous = capture_inputs(
        &LocalWorkspace::open(&run.join("baseline"))?,
        vec![actual_reference(
            "previous.txt",
            b"previous complete source",
        )],
    )?;
    let batch = Batch::new(
        BatchSpec {
            layer: Layer::Source,
            parent: None,
            input_digest: Digest::of_bytes(b"baseline"),
            task_digest: Digest::of_bytes(b"baseline"),
            profile_digest: Digest::of_bytes(b"baseline"),
            generation_id: Digest::of_bytes(b"baseline"),
        },
        previous,
    )?;
    store.commit(
        store.stage(&batch)?,
        &store.snapshot()?,
        &CommitControl::default(),
        || Ok(()),
    )?;
    let before = store.snapshot()?;
    let parents = PipelineParents::capture(&store, Layer::Source)?;
    let pipeline = PipelineControl::default();
    let _scope = pipeline.scope();
    let workspace = LocalExtensionWorkspace::create(run, &path("session"), snapshot, &path(OUTPUT))
        .map_err(|e| e.error)?;
    let observed: RefCell<Option<FrozenExtensionResult>> = RefCell::new(None);
    let control = ExecutionControl::default();
    if mode == "cancel-before" {
        control.cancel();
        pipeline.cancel();
    }
    let session = HostSession::from_composition(
        &files,
        SessionOptions {
            component_id: "generator",
            session_id: "composition-session-1",
            request_id: "request-1",
            protocol: "1.0",
            ir: "1.0",
            required_features: &feature_refs,
            handshake_timeout: Duration::from_secs(10),
            task_timeout: Duration::from_secs(150),
        },
        |output| {
            let captured = files
                .capture_result(workspace.output_workspace(), output)
                .map_err(format_error)?;
            if mode == "fail-output" {
                let (r, bytes) = captured
                    .files()
                    .files()
                    .find(|(r, _)| r.path.as_str() != "outputs.json")
                    .ok_or_else(|| {
                        ProcessError::new("OutputInvalid", "fault", "no payload to mutate")
                    })?;
                let mut changed = bytes.to_vec();
                changed.push(b' ');
                fs::write(workspace.output_path().join(r.path.as_str()), changed)
                    .map_err(|e| ProcessError::new("OutputInvalid", "fault", e.to_string()))?;
            }
            observed.replace(Some(captured));
            if mode == "cancel-result" {
                pipeline.cancel();
                control.cancel();
            }
            Ok(())
        },
    )?;
    let local = LocalProcessRunner::new(&tools, workspace.path(), policy)?;
    let runner = InteractiveProcessRunner::new(&local, session);
    let recipe = files.composition_recipe().expect("checked recipe");
    let process = recipe.pins().process();
    let result = execute_process_validated(&runner, process, &control, |_, _| {
        runner.driver().result_reference()?;
        let captured = observed
            .borrow_mut()
            .take()
            .ok_or_else(|| ProcessError::new("OutputInvalid", "capture", "no terminal snapshot"))?;
        workspace
            .verify_inputs()
            .and_then(|_| workspace.verify_outputs(captured.files()))
            .map_err(query_process)?;
        captured.ensure_success().map_err(format_error)?;
        let bytes = captured
            .files()
            .files()
            .find(|(r, _)| r.path.as_str() == "outputs.json")
            .ok_or_else(|| ProcessError::new("OutputInvalid", "manifest", "missing manifest"))?
            .1;
        capture_artifact_result(
            workspace.output_workspace(),
            recipe,
            bytes,
            64 * 1024 * 1024,
        )
        .map_err(query_process)
    });
    let mut committed = false;
    let mut error = None;
    let mut execution_report = Value::Null;
    match result {
        Err(e) => {
            error = Some(e.to_string());
        }
        Ok(result) => {
            execution_report = json!({"admission":format!("{:?}",result.execution.admission),
                "process":result.execution.process.as_ref().map(|p| json!({
                    "termination":format!("{:?}",p.report.termination),"cleanup_complete":p.report.cleanup.locally_complete(),
                    "diagnostic":p.report.diagnostic.as_ref().map(|e| json!({"code":e.code,"stage":e.stage,"message":e.message})),
                    "stdout":String::from_utf8_lossy(&p.report.stdout.bytes),"stderr":String::from_utf8_lossy(&p.report.stderr.bytes),
                    "pending_cleanup":p.pending_cleanup.is_some()})), "peer_diagnostic":runner.driver().peer_diagnostic()});
            if let Some(artifact) = result.value {
                let batch = parents.batch(&artifact)?;
                if mode == "cancel-commit" {
                    pipeline.cancel();
                }
                match parents.publish(&store, &batch, &pipeline, || {
                    workspace.verify_inputs()?;
                    workspace.verify_outputs(artifact.files())?;
                    input.frozen.verify(&input.workspace)?;
                    verify_source(&input).map_err(|e| {
                        madb_app::QueryError::new(
                            madb_app::ErrorKind::InvalidInput,
                            "INPUT003",
                            "source",
                            e.to_string(),
                        )
                    })?;
                    let installed = LocalWorkspace::open(&tools)?;
                    capture_inputs(&installed, vec![process.tool.clone()])?;
                    Ok(())
                }) {
                    Ok(_) => committed = true,
                    Err(e) => error = Some(e.to_string()),
                }
            }
        }
    }
    let after = store.snapshot()?;
    if committed {
        let published = store
            .pin(Layer::Source)?
            .ok_or("missing source after commit")?;
        fs::create_dir(run.join("public-source"))?;
        for (r, b) in published.files().files() {
            let p = run.join("public-source").join(r.path.as_str());
            fs::create_dir_all(p.parent().ok_or("parent")?)?;
            fs::write(p, b)?;
        }
    }
    println!(
        "{}",
        json!({"scope":"ordinary profile/2 generator session and Source transaction", "committed":committed,
        "index_unchanged":before==after,"before_revision":before.revision,"after_revision":after.revision,
        "source_batch":after.source.map(|s|s.as_str().to_owned()),"error":error,"execution":execution_report,
        "not_claimed":["multiple component sessions","Build or Candidate batch","SDK/native compilation or target run","HAR consumer","E01/E02/E03 whole-package acceptance"]})
    );
    Ok(())
}
fn execution(
    input: &Inputs,
    request: &Document,
    request_bytes: &[u8],
    contract: &[u8],
    tools: &Path,
) -> Result<(ExecutionRecipe, Vec<u8>)> {
    let image = fs::read(tools.join("arkts-peer"))?;
    let selected = input.lock.value()["tools"]
        .as_array()
        .ok_or("tools")?
        .iter()
        .find(|t| t["id"] == TOOL)
        .ok_or("peer tool absent")?;
    let mut refs: BTreeMap<_, _> = input
        .frozen
        .references()
        .map(|r| (r.path.clone(), r.clone()))
        .collect();
    for (name, bytes) in [
        ("lock.json", input.lock_bytes.as_slice()),
        ("profile.json", input.profile_bytes.as_slice()),
        ("contract.json", contract),
    ] {
        let r = actual_reference(name, bytes);
        if refs
            .insert(r.path.clone(), r.clone())
            .is_some_and(|old| old != r)
        {
            return Err("conflicting input".into());
        }
    }
    let mut schemas: BTreeMap<_, _> = refs
        .keys()
        .map(|p| (p.clone(), id("madb.schema/1")))
        .collect();
    for r in request.value()["inputs"].as_array().ok_or("inputs")? {
        schemas.insert(
            path(r["path"].as_str().ok_or("path")?),
            id(r["schema"].as_str().ok_or("schema")?),
        );
    }
    for (name, schema) in [
        ("lock.json", "madb.lock/1"),
        ("profile.json", "madb.profile/2"),
        ("contract.json", "madb.contract-ir/1"),
    ] {
        schemas.insert(path(name), id(schema));
    }
    Ok((
        ExecutionRecipe {
            id: id("generate-c07-composition-session"),
            request: actual_reference("generate-request.json", request_bytes),
            tool: ExecutionTool {
                id: id(TOOL),
                version: selected["version"].as_str().ok_or("version")?.into(),
                digest: Digest::new(selected["sha256"].as_str().ok_or("sha")?)?,
            },
            installed_tool: actual_reference("arkts-peer", &image),
            arguments: vec![],
            environment: BTreeMap::new(),
            working_directory: path("work"),
            response_path: path("outputs.json"),
            inputs: refs.into_values().collect(),
            input_schemas: schemas,
        },
        image,
    ))
}
fn projection(recipe: &ExecutionRecipe) -> Result<Value> {
    Ok(
        json!({"id":recipe.id.as_str(),"tool":{"id":recipe.tool.id.as_str(),"version":recipe.tool.version,"sha256":recipe.tool.digest.as_str()},
        "arguments":recipe.arguments,"environment":[],"working_directory":"work","response_path":"outputs.json",
        "inputs":recipe.inputs.iter().map(|r|json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size,"schema":recipe.input_schemas[&r.path].as_str()})).collect::<Vec<_>>()}),
    )
}
fn verify_source(input: &Inputs) -> Result<()> {
    input.verify_context()?;
    input.frozen.verify(&input.workspace)?;
    let composition: Value =
        serde_json::from_slice(&input.read("composition.json", baseline::DOCUMENT_LIMIT)?)?;
    let package = &composition["packages"][0];
    let root = input.root.join("packages/madb.local.c07-composition/0.1.0");
    let actual = baseline::capture_tree(&root)?;
    let mut expected = package["source"]["files"]
        .as_array()
        .ok_or("source files")?
        .iter()
        .map(baseline::reference)
        .collect::<Result<Vec<FileReference>>>()?;
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    if expected.iter().ne(actual.references()) {
        return Err("package source inventory is not complete".into());
    }
    Ok(())
}
fn format_error(e: madb_contracts::FormatError) -> ProcessError {
    ProcessError::new(e.code, "composition", e.to_string())
}
fn query_process(e: madb_app::QueryError) -> ProcessError {
    ProcessError::new("OutputInvalid", "composition", e.to_string())
}
