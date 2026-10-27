//! Fixed local two-port /2 integration root; not a registered product command.
use super::{inputs as baseline, recipe};
#[path = "session.rs"]
mod session;
use baseline::{Inputs, actual_reference, id, path};
use madb_app::{
    Digest,
    inputs::{FrozenInputs, capture_inputs},
    pipeline::{ArtifactSequence, PipelineControl, PipelineParents},
    ports::FileReference,
    process::ProcessPolicy,
    transaction::{Batch, BatchSpec, CommitControl, Layer},
};
use madb_contracts::{
    Document, DocumentKind,
    artifacts::ProvidedFile,
    composition::{
        GenerationSchemaPaths,
        admission::{CompositionImageRequest, check_composition_image},
        execution::{BoundCompositionGeneration, CompositionGenerationRequest},
        read::{BoundCompositionRead, CompositionReadRequest},
    },
    execution::{ExecutionRecipe, ExecutionTool, generation_task_digest},
    extension_files::BoundExtensionFiles,
};
use madb_host::{LocalWorkspace, transaction::LocalBatchStore};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ENTRY: &str = "crates/madb-cli/examples/dart_ffi_peer.rs";
const READER: &str = "org.madb.dart-ffi/reader";
pub fn run(args: &[String]) -> Result<()> {
    if args.len() != 5 {
        return Err("expected INPUT_ROOT TOOL_ROOT NEW_RUN MODE".into());
    }
    let input = Inputs::load(&baseline::directory(Path::new(&args[1]))?)?;
    let tools = baseline::directory(Path::new(&args[2]))?;
    let run = Path::new(&args[3]);
    let mode = args[4].as_str();
    if !matches!(
        mode,
        "ok" | "cancel-accept"
            | "cancel-between"
            | "cancel-commit"
            | "reader-cancel-before"
            | "reader-cancel-result"
            | "generator-cancel-before"
            | "generator-cancel-result"
            | "reader-output-drift"
            | "generator-output-drift"
            | "upstream-drift"
            | "external-tool-drift"
    ) {
        return Err("unknown chain mode".into());
    }
    check_run(&input.root, &tools, run)?;
    let fixed = recipe::requirements();
    // This root intentionally requires both fixed ports, not the older generator-only recipe.
    if fixed.components.len() != 2 {
        return Err("chain requires the fixed reader and generator".into());
    }
    verify_source(&input)?;
    let bound = input.bind(&fixed)?;
    let image = fs::read(tools.join("dart-peer"))?;
    let read_bytes = input.read("read-request.json", baseline::LIMIT)?;
    let read_request = input
        .registry
        .decode(DocumentKind::ReadRequest, &read_bytes)?;
    let read_recipe = execution(&input, &read_request, &read_bytes, None, &image, "read")?;
    let features = bound.profile().value()["required_features"]
        .as_array()
        .ok_or("features")?
        .iter()
        .map(|v| v.as_str().ok_or("feature"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let policy = policy();
    let pins = check_composition_image(
        &bound,
        CompositionImageRequest {
            entrypoint_source: ENTRY,
            component_id: READER,
            recipe: &read_recipe,
            installed_image: ProvidedFile {
                path: "dart-peer",
                bytes: &image,
            },
            observed_host: "aarch64-apple-darwin",
            observed_source: "fixed locally observed E04 tool image",
            required_features: &features,
            policy: &policy,
        },
    )?;
    let proof = BoundCompositionRead::new(
        bound,
        CompositionReadRequest {
            project: &input.project,
            recipe: read_recipe.clone(),
            pins,
            producer: id(READER),
            output_root: path("outputs/read-1"),
            read_request_schema: path("schemas/read-request.schema.json"),
            request: ProvidedFile {
                path: "read-request.json",
                bytes: &read_bytes,
            },
            lock: ProvidedFile {
                path: "lock.json",
                bytes: &input.lock_bytes,
            },
            profile: ProvidedFile {
                path: "profile.json",
                bytes: &input.profile_bytes,
            },
            installed_image: ProvidedFile {
                path: "dart-peer",
                bytes: &image,
            },
        },
    )?;
    fs::create_dir(run)?;
    let store = baseline_store(run)?;
    let before = store.snapshot()?;
    let parents = PipelineParents::capture(&store, Layer::Source)?;
    let pipeline = PipelineControl::default();
    let _scope = pipeline.scope();
    let mut sequence = ArtifactSequence::begin(pipeline.graph())?;
    let sequence_status;
    let snapshot = freeze(
        &input,
        &run.join("read-frozen"),
        &read_recipe,
        &read_bytes,
        None,
    )?;
    let files = BoundExtensionFiles::from_composition_read(
        &control_ref("read-request.json", "madb.read-request/1", &read_bytes),
        snapshot.clone(),
        proof,
    )?;
    let stage_request = session::Request {
        parent: run,
        tools: &tools,
        name: "read",
        files: &files,
        pipeline: &pipeline,
        fault: mode.strip_prefix("reader-").unwrap_or(""),
    };
    let read_run = session::run(
        &mut sequence,
        &stage_request,
        files.composition_read_recipe().ok_or("reader proof")?,
        &LocalWorkspace::open(&run.join("read-frozen"))?,
    )?;
    let mut reports = vec![session::report(&read_run, &stage_request)];
    let mut committed = false;
    let mut error = None;
    let mut transfer = None;
    if let Some(read_artifact) = read_run.result() {
        let read_stage = read_run.node().ok_or("missing admitted reader")?.stage()?;
        let (upstream_ref, contract_bytes) = read_artifact.manifest();
        let contract = input
            .registry
            .decode(DocumentKind::ContractIr, contract_bytes)?;
        transfer = Some(
            json!({"upstream":ref_value(upstream_ref),"downstream":ref_value(&actual_reference("contract.json",contract_bytes)),"exact_bytes":true,"size":contract_bytes.len()}),
        );
        if mode == "cancel-between" {
            pipeline.cancel();
        }
        {
            let bound = input.bind(&fixed)?;
            let roles = recipe::roles(&input, &bound, &contract)?;
            let mut generate = read_request.value().clone();
            generate["schema"] = json!("madb.generate-request/1");
            generate["component"] = bound.composition().value()["components"]
                .as_array()
                .ok_or("components")?
                .iter()
                .find(|c| c["id"] == recipe::GENERATOR)
                .ok_or("generator")?
                .clone();
            generate["output_root"] = json!("outputs/generate-1");
            generate["contract"] = json!({"path":"contract.json","schema":"madb.contract-ir/1","sha256":Digest::of_bytes(contract_bytes).as_str(),"size":contract_bytes.len()});
            for (output, input) in [
                ("input_digest", "source_digest"),
                ("contract_digest", "structure_digest"),
                ("policy_digest", "policy_digest"),
            ] {
                generate[output] = contract.value()[input].clone();
            }
            generate["task_digest"] = json!(Digest::of_bytes(b"pending task").as_str());
            let draft = serde_json::to_vec(&generate)?;
            let request = input
                .registry
                .decode(DocumentKind::GenerateRequest, &draft)?;
            let mut recipe = execution(
                &input,
                &request,
                &draft,
                Some(contract_bytes),
                &image,
                "generate",
            )?;
            generate["task_digest"] =
                json!(generation_task_digest(&request, &projection(&recipe))?);
            let request_bytes = serde_json::to_vec(&generate)?;
            recipe.request = actual_reference("generate-request.json", &request_bytes);
            let mut feature_names = contract.value()["required_features"]
                .as_array()
                .ok_or("features")?
                .iter()
                .map(|f| f.as_str().ok_or("feature").map(str::to_owned))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            feature_names.push("core.extension-composition@1".into());
            feature_names.sort();
            let refs = feature_names.iter().map(String::as_str).collect::<Vec<_>>();
            let pins = check_composition_image(
                &bound,
                CompositionImageRequest {
                    entrypoint_source: ENTRY,
                    component_id: recipe::GENERATOR,
                    recipe: &recipe,
                    installed_image: ProvidedFile {
                        path: "dart-peer",
                        bytes: &image,
                    },
                    observed_host: "aarch64-apple-darwin",
                    observed_source: "fixed locally observed E04 tool image",
                    required_features: &refs,
                    policy: &policy,
                },
            )?;
            let proof = BoundCompositionGeneration::new(
                bound,
                CompositionGenerationRequest {
                    project: &input.project,
                    recipe: recipe.clone(),
                    pins,
                    producer: id(recipe::GENERATOR),
                    slots: vec![id(recipe::SLOT)],
                    output_root: path("outputs/generate-1"),
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
                        bytes: contract_bytes,
                    },
                    installed_image: ProvidedFile {
                        path: "dart-peer",
                        bytes: &image,
                    },
                    required_responsibilities: &roles,
                },
            )?;
            let snapshot = freeze(
                &input,
                &run.join("generate-frozen"),
                &recipe,
                &request_bytes,
                Some(contract_bytes),
            )?;
            let files = BoundExtensionFiles::from_composition_recipe(
                &control_ref(
                    "generate-request.json",
                    "madb.generate-request/1",
                    &request_bytes,
                ),
                snapshot.clone(),
                proof,
            )?;
            let stage_request = session::Request {
                parent: run,
                tools: &tools,
                name: "generate",
                files: &files,
                pipeline: &pipeline,
                fault: mode.strip_prefix("generator-").unwrap_or(""),
            };
            let generate_run = session::run(
                &mut sequence,
                &stage_request,
                files.composition_recipe().ok_or("generator proof")?,
                &LocalWorkspace::open(&run.join("generate-frozen"))?,
            )?;
            reports.push(session::report(&generate_run, &stage_request));
            if mode == "cancel-accept" {
                pipeline.cancel();
            }
            sequence_status = sequence.finish();
            if let Some(artifact) = generate_run
                .result()
                .filter(|_| sequence_status == madb_app::dag::GraphStatus::Validated)
            {
                let generate_stage = generate_run
                    .node()
                    .ok_or("missing admitted generator")?
                    .stage()?;
                if mode == "upstream-drift" {
                    let mut changed = contract_bytes.to_vec();
                    changed.push(b' ');
                    fs::write(
                        read_stage
                            .workspace
                            .output_path()
                            .join(upstream_ref.path.as_str()),
                        changed,
                    )?;
                }
                if mode == "cancel-commit" {
                    pipeline.cancel();
                }
                if mode == "external-tool-drift" {
                    baseline::corrupt_test_tool(&input, run)?;
                }
                let batch = parents.batch(artifact)?;
                match parents.publish(&store, &batch, &pipeline, || {
                    read_stage.workspace.verify_inputs()?;
                    read_stage.workspace.verify_outputs(read_artifact.files())?;
                    generate_stage.workspace.verify_inputs()?;
                    generate_stage.workspace.verify_outputs(artifact.files())?;
                    verify_source(&input).map_err(|e| {
                        madb_app::QueryError::new(
                            madb_app::ErrorKind::InvalidInput,
                            "INPUT003",
                            "source",
                            e.to_string(),
                        )
                    })?;
                    capture_inputs(
                        &LocalWorkspace::open(&tools)?,
                        vec![actual_reference("dart-peer", &image)],
                    )?;
                    Ok(())
                }) {
                    Ok(_) => committed = true,
                    Err(e) => error = Some(e.to_string()),
                }
            }
        }
    } else {
        sequence_status = sequence.finish();
    }
    let after = store.snapshot()?;
    if committed {
        let published = store.pin(Layer::Source)?.ok_or("source absent")?;
        fs::create_dir(run.join("public-source"))?;
        for (r, b) in published.files().files() {
            let dest = run.join("public-source").join(r.path.as_str());
            fs::create_dir_all(dest.parent().ok_or("parent")?)?;
            fs::write(dest, b)?;
        }
    }
    println!(
        "{}",
        json!({"scope":"fixed profile/2 real reader then generator and atomic Source","mode":mode,"sequence_status":format!("{:?}",sequence_status),"committed":committed,"index_unchanged":before==after,"before_revision":before.revision,"after_revision":after.revision,"source_batch":after.source.map(|d|d.as_str().to_owned()),"error":error,"transfer":transfer,"stages":reports,"not_claimed":["Build","Package","Dart runtime consumption","E04 whole-package acceptance"]})
    );
    Ok(())
}
fn policy() -> ProcessPolicy {
    ProcessPolicy {
        timeout: Duration::from_secs(180),
        termination_grace: Duration::from_secs(3),
        cleanup_timeout: Duration::from_secs(5),
        stderr_limit: 1024 * 1024,
        ..ProcessPolicy::default()
    }
}
fn control_ref(name: &str, schema: &str, bytes: &[u8]) -> Value {
    json!({"path":format!("inputs/{name}"),"schema":schema,"sha256":Digest::of_bytes(bytes).as_str(),"size":bytes.len()})
}
fn execution(
    input: &Inputs,
    request: &Document,
    request_bytes: &[u8],
    contract: Option<&[u8]>,
    image: &[u8],
    operation: &str,
) -> Result<ExecutionRecipe> {
    let tool = format!(
        "madb-dart-ffi-{}",
        if operation == "read" {
            "reader"
        } else {
            "generator"
        }
    );
    let selected = input.lock.value()["tools"]
        .as_array()
        .ok_or("tools")?
        .iter()
        .find(|t| t["id"] == tool)
        .ok_or("tool absent")?;
    let mut references = input
        .frozen
        .references()
        .map(|r| (r.path.clone(), r.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut extra = vec![
        ("lock.json", input.lock_bytes.as_slice()),
        ("profile.json", input.profile_bytes.as_slice()),
    ];
    if let Some(bytes) = contract {
        extra.push(("contract.json", bytes));
    }
    for (name, bytes) in extra {
        let r = actual_reference(name, bytes);
        if references
            .insert(r.path.clone(), r.clone())
            .is_some_and(|old| old != r)
        {
            return Err("conflicting context bytes".into());
        }
    }
    let mut schemas = references
        .keys()
        .map(|p| (p.clone(), id("madb.schema/1")))
        .collect::<BTreeMap<_, _>>();
    for r in request.value()["inputs"].as_array().ok_or("inputs")? {
        schemas.insert(
            path(r["path"].as_str().ok_or("path")?),
            id(r["schema"].as_str().ok_or("schema")?),
        );
    }
    schemas.insert(path("lock.json"), id("madb.lock/1"));
    schemas.insert(path("profile.json"), id("madb.profile/2"));
    if contract.is_some() {
        schemas.insert(path("contract.json"), id("madb.contract-ir/1"));
    }
    Ok(ExecutionRecipe {
        id: id(&format!("{operation}-dart-ffi-chain")),
        request: actual_reference(&format!("{operation}-request.json"), request_bytes),
        tool: ExecutionTool {
            id: id(&tool),
            version: selected["version"].as_str().ok_or("version")?.into(),
            digest: Digest::new(selected["sha256"].as_str().ok_or("sha")?)?,
        },
        installed_tool: actual_reference("dart-peer", image),
        arguments: vec![operation.into()],
        environment: BTreeMap::new(),
        working_directory: path("work"),
        response_path: path(if operation == "read" {
            "contract.json"
        } else {
            "outputs.json"
        }),
        inputs: references.into_values().collect(),
        input_schemas: schemas,
    })
}
fn projection(recipe: &ExecutionRecipe) -> Value {
    json!({"id":recipe.id.as_str(),"tool":{"id":recipe.tool.id.as_str(),"version":recipe.tool.version,"sha256":recipe.tool.digest.as_str()},"arguments":recipe.arguments,"environment":[],"working_directory":recipe.working_directory.as_str(),"response_path":recipe.response_path.as_str(),"inputs":recipe.inputs.iter().map(|r|json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size,"schema":recipe.input_schemas[&r.path].as_str()})).collect::<Vec<_>>()})
}
fn freeze(
    input: &Inputs,
    dest: &Path,
    recipe: &ExecutionRecipe,
    request: &[u8],
    contract: Option<&[u8]>,
) -> Result<FrozenInputs> {
    fs::create_dir(dest)?;
    for (r, b) in input.frozen.files() {
        write(dest, r.path.as_str(), b)?;
    }
    write(dest, "lock.json", &input.lock_bytes)?;
    write(dest, "profile.json", &input.profile_bytes)?;
    if let Some(b) = contract {
        write(dest, "contract.json", b)?;
    }
    write(dest, recipe.request.path.as_str(), request)?;
    let mut refs = recipe.inputs.clone();
    refs.push(recipe.request.clone());
    Ok(capture_inputs(&LocalWorkspace::open(dest)?, refs)?)
}
fn write(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let dest = root.join(name);
    fs::create_dir_all(dest.parent().ok_or("parent")?)?;
    if dest.exists() && fs::read(&dest)? != bytes {
        return Err("input aliases different bytes".into());
    }
    fs::write(dest, bytes)?;
    Ok(())
}
fn baseline_store(run: &Path) -> Result<LocalBatchStore> {
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
    Ok(store)
}
fn verify_source(input: &Inputs) -> Result<()> {
    input.verify()?;
    input.frozen.verify(&input.workspace)?;
    let composition: Value =
        serde_json::from_slice(&input.read("composition.json", baseline::LIMIT)?)?;
    let actual = baseline::capture_tree(&input.root.join("packages/org.madb.dart-ffi/0.1.0"))?;
    let mut expected = composition["packages"][0]["source"]["files"]
        .as_array()
        .ok_or("files")?
        .iter()
        .map(baseline::reference)
        .collect::<Result<Vec<FileReference>>>()?;
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    if expected.iter().ne(actual.references()) {
        return Err("package source closure changed".into());
    }
    Ok(())
}

fn ref_value(r: &FileReference) -> Value {
    json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size})
}
fn check_run(input: &Path, tools: &Path, run: &Path) -> Result<()> {
    if !run.is_absolute()
        || run.exists()
        || run.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err("fresh absolute run directory required".into());
    }
    let parent = baseline::directory(run.parent().ok_or("run parent")?)?;
    let normalized = parent.join(run.file_name().ok_or("run name")?);
    for protected in [input, tools] {
        if normalized.starts_with(protected) || protected.starts_with(&normalized) {
            return Err("run overlaps immutable inputs".into());
        }
    }
    Ok(())
}
