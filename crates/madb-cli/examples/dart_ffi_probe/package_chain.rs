//! Fixed file mapping over actual C04 Source/Build parents; no new wire.
use super::{
    Result,
    build_chain::{Memory, fresh, policy, process, ref_json},
    inputs::{self, Inputs, actual_reference, id, path},
    recipe,
};
use madb_app::{
    Digest,
    inputs::capture_inputs,
    pipeline::{PipelineControl, PipelineParents, run_artifact},
    transaction::Layer,
};
use madb_contracts::{
    DocumentKind,
    artifacts::ProvidedFile,
    composition::package::{
        BoundCompositionPackage, CompositionPackageRequest, PackageFileMapping,
    },
    execution::{ExecutionRecipe, ExecutionTool},
};
use madb_host::{LocalWorkspace, artifact::LocalArtifactExecutor, transaction::LocalBatchStore};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};

pub fn run(args: &[String]) -> Result<()> {
    if args.len() != 5 && !(args.len() == 6 && args[5] == "external-tool-drift") {
        return Err("expected package INPUT_ROOT TOOL_ROOT SOURCE_RUN NEW_RUN".into());
    }
    let input = Inputs::load(&inputs::directory(Path::new(&args[1]))?)?;
    let tools = inputs::directory(Path::new(&args[2]))?;
    let source_run = inputs::directory(Path::new(&args[3]))?;
    let run = Path::new(&args[4]);
    fresh(run, &[&input.root, &tools, &source_run])?;
    let bound = input.bind(&recipe::requirements())?;
    let store = LocalBatchStore::open(&source_run.join("store"))?;
    let parents = PipelineParents::capture(&store, Layer::Candidate)?;
    let source = parents.source().ok_or("source absent")?.files();
    let build = parents.build().ok_or("build absent")?.files();
    let bytes = source
        .files()
        .find(|(r, _)| r.path.as_str() == "declarations.json")
        .ok_or("declarations absent")?
        .1;
    let contract = input.registry.decode(DocumentKind::ContractIr, bytes)?;
    let roles = recipe::roles(&input, &bound, &contract)?;
    recipe::check(&input, &bound, &contract)?;
    let mut memory = Memory::default();
    memory.extend(&input.frozen)?;
    memory.extend(source)?;
    memory.extend(build)?;
    memory.insert("profile.json", &input.profile_bytes)?;
    memory.insert("lock.json", &input.lock_bytes)?;
    let frozen = memory.frozen()?;
    let find = |name: &str| -> Result<_> {
        frozen
            .references()
            .find(|r| r.path.as_str() == name)
            .cloned()
            .ok_or_else(|| format!("missing package input: {name}").into())
    };
    let mut mapping = BTreeMap::new();
    let mut map = |from: &str, to: &str| -> Result<()> {
        let r = find(from)?;
        if mapping.insert(to.to_owned(), r).is_some() {
            return Err("duplicate package destination".into());
        }
        Ok(())
    };
    for r in input.frozen.references() {
        map(r.path.as_str(), r.path.as_str())?;
    }
    map("profile.json", "profile.json")?;
    map("lock.json", "madb.lock")?;
    for r in source.references() {
        let name = r.path.as_str();
        map(name, &format!("source/{name}"))?;
        if name.starts_with("lib/")
            || [
                "LICENSE",
                "NOTICE",
                "README.md",
                "pubspec.yaml",
                "pubspec.lock",
                "native/native.dylib",
                "native/identity.json",
            ]
            .contains(&name)
        {
            map(name, name)?;
        }
    }
    map("declarations.json", "contract.json")?;
    for r in build.references() {
        map(r.path.as_str(), r.path.as_str())?;
    }
    for (from, to) in [
        ("build/native/bridge.dylib", "native/bridge.dylib"),
        ("build/native/install.json", "native/install.json"),
        (
            "build/lib/src/package_pins.dart",
            "lib/src/package_pins.dart",
        ),
        ("build/provenance.json", "provenance.json"),
    ] {
        map(from, to)?;
    }
    for phase in ["compile", "link", "run"] {
        map(
            &format!("build/{phase}.commands.json"),
            &format!("logs/{phase}.json"),
        )?;
    }
    let files = mapping
        .into_iter()
        .map(|(destination, source)| {
            Ok(PackageFileMapping {
                source,
                destination: madb_app::LogicalPath::new(destination)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let map_json = files
        .iter()
        .map(|m| json!({"source":ref_json(&m.source),"destination":m.destination.as_str()}))
        .collect::<Vec<_>>();
    memory.insert("package-map.json", &serde_json::to_vec(&map_json)?)?;
    let provided = memory.frozen()?;
    let mut output = ref_json(&find("outputs.json")?);
    output["schema"] = json!("madb.output-set/1");
    let evidence = build
        .references()
        .filter(|r| r.path.as_str().starts_with("evidence/"))
        .map(|r| {
            let mut f = ref_json(r);
            f["schema"] = json!("madb.build-evidence/1");
            f
        })
        .collect::<Vec<_>>();
    let dependencies = [
        "dependencies/ffi-2.2.0.tar.gz",
        "dependencies/crypto-3.0.8.tar.gz",
        "provider/candidate.json",
    ]
    .into_iter()
    .map(|name| {
        let mut f = ref_json(&find(name)?);
        f["schema"] = json!(if name.ends_with("candidate.json") {
            "madb.candidate/1"
        } else {
            "madb.source-input/1"
        });
        Ok(f)
    })
    .collect::<Result<Vec<_>>>()?;
    let mut lock_ref = ref_json(&actual_reference("lock.json", &input.lock_bytes));
    lock_ref["schema"] = json!("madb.lock/1");
    let request = json!({"schema":"madb.package-request/1","selector":contract.value()["selector"],"output_set":output,"build_evidence":evidence,"lock":lock_ref,"contract_digest":contract.value()["structure_digest"],"policy_digest":contract.value()["policy_digest"],"generation_id":parents.source().ok_or("source")?.info().spec.generation_id.as_str(),"dependencies":dependencies,"public_symbols":contract.value()["symbols"].as_array().ok_or("symbols")?.iter().map(|s|s["public_id"].clone()).collect::<Vec<_>>(),"required_cases":["e04.dart-sync.values","e04.dart-sync.ranges","e04.dart-sync.close","e04.dart-sync.isolate","e04.dart-sync.package"],"output_root":"candidate"});
    let request_bytes = serde_json::to_vec(&request)?;
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
    let execution = ExecutionRecipe {
        id: id("dart-ffi-package"),
        request: actual_reference("package-request.json", &request_bytes),
        tool: ExecutionTool {
            id: id("e04-python"),
            version: python["version"].as_str().ok_or("version")?.to_owned(),
            digest: installed.digest.clone(),
        },
        installed_tool: installed.clone(),
        arguments: [
            "-I",
            "-S",
            "-B",
            "-X",
            "pycache_prefix=/dev/null",
            "../inputs/packages/org.madb.dart-ffi/0.1.0/adapters/generators/dart_ffi/package.py",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        environment: BTreeMap::new(),
        working_directory: path("work"),
        response_path: path("candidate.json"),
        inputs: provided.references().cloned().collect(),
        input_schemas: provided
            .references()
            .map(|r| {
                let name = r.path.as_str();
                let schema = if name == "profile.json" {
                    "madb.profile/2"
                } else if name == "lock.json" {
                    "madb.lock/1"
                } else if name == "outputs.json" {
                    "madb.output-set/1"
                } else if name == "declarations.json" {
                    "madb.contract-ir/1"
                } else if name.starts_with("evidence/") {
                    "madb.build-evidence/1"
                } else if name.starts_with("schemas/") {
                    "madb.schema/1"
                } else {
                    "madb.source-input/1"
                };
                (r.path.clone(), id(schema))
            })
            .collect(),
    };
    let proof = BoundCompositionPackage::new(
        &bound,
        CompositionPackageRequest {
            project: &input.project,
            contract: &contract,
            required_responsibilities: &roles,
            recipe: &execution,
            request: ProvidedFile {
                path: "package-request.json",
                bytes: &request_bytes,
            },
            lock: ProvidedFile {
                path: "lock.json",
                bytes: &input.lock_bytes,
            },
            source,
            build,
            provided: &provided,
            files: &files,
        },
    )?;
    fs::create_dir(run)?;
    fs::write(run.join("package-request.json"), &request_bytes)?;
    fs::write(
        run.join("package-map.json"),
        serde_json::to_vec_pretty(&map_json)?,
    )?;
    let before = store.snapshot()?;
    let control = PipelineControl::default();
    let _scope = control.scope();
    let mut executor =
        LocalArtifactExecutor::new(run, &path("package"), &tools, policy()).map_err(|e| e.error)?;
    let result = run_artifact(&proof, &memory, &mut executor, control.graph())?;
    let mut committed = false;
    let mut failure = None;
    if let Some(artifact) = result.result() {
        let batch = parents.batch(artifact)?;
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
    if committed {
        let lease = store.pin(Layer::Candidate)?.ok_or("candidate absent")?;
        for (r, b) in lease.files().files() {
            let dest = run.join("candidate").join(r.path.as_str());
            fs::create_dir_all(dest.parent().ok_or("parent")?)?;
            fs::write(dest, b)?;
        }
    }
    let report = json!({"scope":"typed profile/2 exact Source+Build mapping and atomic Candidate","status":format!("{:?}",result.status),"committed":committed,"index_unchanged":before==after,"candidate_batch":after.candidate.as_ref().map(Digest::as_str),"failure":failure,"process":result.node().map(|n|process(n.execution())),"not_claimed":["independent Lab","whole E04"]});
    fs::write(run.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}
