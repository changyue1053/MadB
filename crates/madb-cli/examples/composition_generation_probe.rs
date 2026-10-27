//! Fixed local composition test root. This is not a CLI product command, an
//! extension registration service, an E02 session or a Source commit endpoint.
use madb_app::{
    Digest, Id, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::ProvidedFile,
    composition::{
        AssetOrigin, BoundCompositionInputs, ComponentRequirement, CompositionInputContext,
        CompositionSchemaPaths, Direction, GeneratedSlot, GenerationBindingContext,
        GenerationSchemaPaths, GenerationStepObservation, GenerationStepRequirement,
        RecipeRequirements, RecipeSelection, ResolvedOwner, ResolvedResponsibility, ResolvedScope,
        ResolvedSubject, ResponsibilityKey, bind_composition_inputs, bind_generated_outputs,
        compare_responsibility_sets, composition_digest, profile_v2_digest,
    },
    execution::{ExecutionRecipe, ExecutionTool, generation_task_digest},
    native,
};
use madb_host::LocalWorkspace;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) const COMPONENT: &str = "madb.local.c07-composition/generator";
pub(crate) const SLOT: &str = "ordinary-source";
pub(crate) const STEP: &str = "generate-c07-composition";
pub(crate) const SOURCE_PATHS: &[&str] = &[
    "identity.json",
    "declarations.json",
    "generated/contract/runtime-policy.json",
    "generated/contract/frame-layout.json",
    "generated/contract/inventory-format.json",
    "generated/notices/native-NOTICE.txt",
    "generated/notices/llvm-NOTICE.txt",
    "generated/facade/Boundary.ets",
    "generated/facade/RuntimeFrame.ets",
    "generated/native/exports.map",
    "generated/native/Index.d.ts",
    "generated/native/exports.generated.inc",
    "generated/keep-rules.txt",
    "generated/native/frame.generated.hpp",
    "generated/native/identity.generated.hpp",
];
const GENERATE_ARGS: &[&str] = &[
    "-I",
    "-S",
    "-B",
    "-X",
    "pycache_prefix=/dev/null",
    "../inputs/packages/madb.local.c07-composition/0.1.0/adapters/generators/arkts/composition_generate.py",
    "generate",
    "--config",
    "../inputs/backend.json",
];
pub(crate) const DOCUMENT_LIMIT: usize = native::MAX_ENVELOPE_BYTES;
pub(crate) const FILE_LIMIT: usize = 32 * 1024 * 1024;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3
        || !matches!(
            args[1].as_str(),
            "metadata" | "task" | "bind" | "composition-digest" | "profile-digest"
        )
        || args.len() != if args[1] == "bind" { 4 } else { 3 }
    {
        return Err("expected metadata|task INPUT_ROOT or bind INPUT_ROOT OUTPUT_ROOT".into());
    }
    let root = directory(Path::new(&args[2]))?;
    if matches!(args[1].as_str(), "composition-digest" | "profile-digest") {
        // Assembly needs these identities before the referring lock exists.
        // A hash is deliberately not input admission or recipe acceptance.
        let workspace = LocalWorkspace::open(&root)?;
        let registry = SchemaRegistry::new()?;
        let digest = if args[1] == "composition-digest" {
            let bytes = read(&workspace, "composition.json", DOCUMENT_LIMIT)?;
            composition_digest(&registry.decode(DocumentKind::Composition, &bytes)?)?
        } else {
            let bytes = read(&workspace, "profile.json", DOCUMENT_LIMIT)?;
            profile_v2_digest(&registry.decode(DocumentKind::ProfileV2, &bytes)?)?
        };
        println!("{digest}");
        return Ok(());
    }
    let input = Inputs::load(&root)?;
    let recipe = recipe();
    let bound = input.bind(&recipe)?;
    match args[1].as_str() {
        "metadata" => {
            let mut seed = Vec::new();
            std::io::stdin()
                .lock()
                .take(DOCUMENT_LIMIT as u64 + 1)
                .read_to_end(&mut seed)?;
            if seed.len() > DOCUMENT_LIMIT {
                return Err("metadata seed exceeds document budget".into());
            }
            check_roles(
                &input,
                &bound,
                &input.registry.decode(DocumentKind::ContractIr, &seed)?,
            )?;
            let completed = native::finalize_composition_metadata(&seed, input.context(), &recipe)?;
            input.frozen.verify(&input.workspace)?;
            input.verify_context()?;
            std::io::stdout().lock().write_all(&completed)?;
        }
        "task" => {
            let request_bytes = input.read("generate-request.json", DOCUMENT_LIMIT)?;
            let request = input
                .registry
                .decode(DocumentKind::GenerateRequest, &request_bytes)?;
            let contract = input.read("contract.json", DOCUMENT_LIMIT)?;
            let (execution, _) = execution(&input, &request, &request_bytes, &contract)?;
            let digest = generation_task_digest(&request, &projection(&execution)?)?;
            input.frozen.verify(&input.workspace)?;
            input.verify_context()?;
            println!("{digest}");
        }
        "bind" => {
            let output = directory(Path::new(&args[3]))?;
            separate(&root, &output)?;
            bind_outputs(&input, &bound, &output)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
pub(crate) fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).expect("fixed logical path")
}
pub(crate) fn id(value: &str) -> Id {
    Id::new(value).expect("fixed identity")
}
pub(crate) fn recipe() -> RecipeRequirements {
    RecipeRequirements {
        id: id("madb.local.c07-c-arkts/ordinary@1"),
        selection: RecipeSelection {
            profile_id: id("c07-c-arkts-composition-aarch64-ohos23"),
            route: id("ARKTS-C-NAPI"),
            direction: Direction::ConsumerToProvider,
            target: id("aarch64-linux-ohos"),
        },
        components: vec![ComponentRequirement {
            id: id(COMPONENT),
            port: id("generator"),
            interprets_composition: true,
        }],
        generated_slots: vec![GeneratedSlot {
            producer: id(COMPONENT),
            slot: id(SLOT),
            paths: SOURCE_PATHS.iter().map(|v| path(v)).collect(),
        }],
    }
}
pub(crate) fn schemas() -> CompositionSchemaPaths {
    CompositionSchemaPaths {
        common: path("schemas/common.schema.json"),
        project: path("schemas/project.schema.json"),
        lock: path("schemas/lock.schema.json"),
        profile: path("schemas/profile-v2.schema.json"),
        composition: path("schemas/composition.schema.json"),
        extension: path("schemas/extension-manifest.schema.json"),
        contract_ir: path("schemas/contract-ir.schema.json"),
    }
}
pub(crate) struct Inputs {
    pub(crate) root: PathBuf,
    pub(crate) root_identity: (u64, u64),
    pub(crate) workspace: LocalWorkspace,
    pub(crate) registry: SchemaRegistry,
    pub(crate) project: Document,
    pub(crate) project_bytes: Vec<u8>,
    pub(crate) lock: Document,
    pub(crate) lock_bytes: Vec<u8>,
    pub(crate) profile_bytes: Vec<u8>,
    pub(crate) frozen: FrozenInputs,
    pub(crate) schemas: CompositionSchemaPaths,
}
impl Inputs {
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let root_identity = directory_identity(root)?;
        let workspace = LocalWorkspace::open(root)?;
        let registry = SchemaRegistry::new()?;
        let project_bytes = read(&workspace, "project.json", DOCUMENT_LIMIT)?;
        let lock_bytes = read(&workspace, "lock.json", DOCUMENT_LIMIT)?;
        let profile_bytes = read(&workspace, "profile.json", DOCUMENT_LIMIT)?;
        let project = registry.decode(DocumentKind::Project, &project_bytes)?;
        let lock = registry.decode(DocumentKind::Lock, &lock_bytes)?;
        let refs = ["inputs", "schemas"]
            .into_iter()
            .flat_map(|k| lock.value()[k].as_array().expect("schema checked"))
            .map(reference)
            .collect::<Result<Vec<_>>>()?;
        let frozen = capture_inputs(&workspace, refs)?;
        Ok(Self {
            root: root.to_owned(),
            root_identity,
            workspace,
            registry,
            project,
            project_bytes,
            lock,
            lock_bytes,
            profile_bytes,
            frozen,
            schemas: schemas(),
        })
    }
    pub(crate) fn context(&self) -> CompositionInputContext<'_> {
        CompositionInputContext {
            project: &self.project,
            lock: &self.lock,
            profile: ProvidedFile {
                path: "profile.json",
                bytes: &self.profile_bytes,
            },
            boundary_id: "c-arkts",
            inputs: &self.frozen,
            schema_paths: &self.schemas,
        }
    }
    pub(crate) fn bind(&self, recipe: &RecipeRequirements) -> Result<BoundCompositionInputs> {
        let bound = bind_composition_inputs(self.context(), recipe)?;
        let value = bound.composition().value();
        let packages = value["packages"].as_array().expect("checked packages");
        if packages.len() != 1
            || packages[0]["id"] != "madb.local.c07-composition"
            || packages[0]["version"] != "0.1.0"
            || value["components"][0]["version"] != "0.1.0"
        {
            return Err("package is outside the fixed local composition recipe".into());
        }
        Ok(bound)
    }
    pub(crate) fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        read(&self.workspace, name, limit)
    }
    pub(crate) fn verify_context(&self) -> Result<()> {
        if directory(&self.root)? != self.root
            || directory_identity(&self.root)? != self.root_identity
        {
            return Err("input root changed".into());
        }
        for (name, expected) in [
            ("project.json", &self.project_bytes),
            ("lock.json", &self.lock_bytes),
            ("profile.json", &self.profile_bytes),
        ] {
            if self.read(name, DOCUMENT_LIMIT)? != *expected {
                return Err(format!("context changed: {name}").into());
            }
        }
        Ok(())
    }
}
fn read(workspace: &LocalWorkspace, name: &str, limit: usize) -> Result<Vec<u8>> {
    workspace
        .read(&LogicalPath::new(name)?, limit)?
        .ok_or_else(|| format!("missing input: {name}").into())
}
pub(crate) fn reference(v: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: LogicalPath::new(v["path"].as_str().ok_or("missing path")?)?,
        digest: Digest::new(v["sha256"].as_str().ok_or("missing digest")?)?,
        size: v["size"].as_u64().ok_or("missing size")?,
    })
}
pub(crate) fn actual_reference(name: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: path(name),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
fn execution(
    input: &Inputs,
    request: &Document,
    request_bytes: &[u8],
    contract: &[u8],
) -> Result<(ExecutionRecipe, Vec<u8>)> {
    let image = input.read("tools/python", FILE_LIMIT)?;
    let installed_tool = actual_reference("tools/python", &image);
    let tool = input.lock.value()["tools"]
        .as_array()
        .expect("checked tools")
        .iter()
        .find(|v| v["id"] == "c07-python")
        .ok_or("fixed c07-python tool absent")?;
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
        let reference = actual_reference(name, bytes);
        if let Some(old) = refs.insert(reference.path.clone(), reference.clone())
            && old != reference
        {
            return Err(format!("conflicting fixed input: {name}").into());
        }
    }
    let mut input_schemas: BTreeMap<_, _> = refs
        .keys()
        .map(|p| (p.clone(), id("madb.schema/1")))
        .collect();
    for row in request.value()["inputs"]
        .as_array()
        .expect("checked inputs")
    {
        input_schemas.insert(
            LogicalPath::new(row["path"].as_str().ok_or("input path")?)?,
            Id::new(row["schema"].as_str().ok_or("input schema")?)?,
        );
    }
    for (name, schema) in [
        ("lock.json", "madb.lock/1"),
        ("profile.json", "madb.profile/2"),
        ("contract.json", "madb.contract-ir/1"),
    ] {
        input_schemas.insert(path(name), id(schema));
    }
    Ok((
        ExecutionRecipe {
            id: id(STEP),
            request: actual_reference("generate-request.json", request_bytes),
            tool: ExecutionTool {
                id: id("c07-python"),
                version: tool["version"].as_str().ok_or("tool version")?.into(),
                digest: Digest::new(tool["sha256"].as_str().ok_or("tool digest")?)?,
            },
            installed_tool,
            arguments: GENERATE_ARGS.iter().map(|v| (*v).into()).collect(),
            environment: BTreeMap::new(),
            working_directory: path("work"),
            response_path: path("outputs.json"),
            inputs: refs.into_values().collect(),
            input_schemas,
        },
        image,
    ))
}
fn projection(recipe: &ExecutionRecipe) -> Result<Value> {
    let inputs = recipe
        .inputs
        .iter()
        .map(|f| {
            Ok(
                json!({"path":f.path.as_str(),"sha256":f.digest.as_str(),"size":f.size,
            "schema":recipe.input_schemas.get(&f.path).ok_or("missing recipe schema")?.as_str()}),
            )
        })
        .collect::<Result<Vec<Value>>>()?;
    Ok(
        json!({"id":recipe.id.as_str(),"tool":{"id":recipe.tool.id.as_str(),"version":recipe.tool.version,"sha256":recipe.tool.digest.as_str()},
        "arguments":recipe.arguments,"environment":[],"working_directory":recipe.working_directory.as_str(),
        "response_path":recipe.response_path.as_str(),"inputs":inputs}),
    )
}
fn bind_outputs(input: &Inputs, bound: &BoundCompositionInputs, output: &Path) -> Result<()> {
    let request_bytes = input.read("generate-request.json", DOCUMENT_LIMIT)?;
    let request = input
        .registry
        .decode(DocumentKind::GenerateRequest, &request_bytes)?;
    let contract = input.read("contract.json", DOCUMENT_LIMIT)?;
    check_roles(
        input,
        bound,
        &input.registry.decode(DocumentKind::ContractIr, &contract)?,
    )?;
    let (execution, image) = execution(input, &request, &request_bytes, &contract)?;
    let outputs = capture_tree(output)?;
    let generation_schemas = GenerationSchemaPaths {
        generate_request: path("schemas/generate-request.schema.json"),
        output_set: path("schemas/output-set.schema.json"),
    };
    let steps = [GenerationStepRequirement {
        recipe: &execution,
        producer: id(COMPONENT),
        output_slots: vec![id(SLOT)],
        output_root: path("generated-step"),
    }];
    let observations = [GenerationStepObservation {
        step_id: id(STEP),
        request: ProvidedFile {
            path: "generate-request.json",
            bytes: &request_bytes,
        },
        installed_tool: ProvidedFile {
            path: "tools/python",
            bytes: &image,
        },
        outputs: &outputs,
    }];
    let bound_outputs = bind_generated_outputs(
        bound,
        GenerationBindingContext {
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
                bytes: &contract,
            },
            schema_paths: &generation_schemas,
            required_steps: &steps,
        },
        &observations,
    )?;
    input.frozen.verify(&input.workspace)?;
    input.verify_context()?;
    for (name, bytes) in [
        ("contract.json", &contract),
        ("generate-request.json", &request_bytes),
        ("tools/python", &image),
    ] {
        if input.read(name, FILE_LIMIT)? != *bytes {
            return Err(format!("late input change: {name}").into());
        }
    }
    let after = capture_tree(output)?;
    if outputs.references().ne(after.references()) {
        return Err("late output change".into());
    }
    let slots: Vec<_> = bound_outputs.slots().iter().map(|slot|json!({"producer":slot.producer().as_str(),"slot":slot.slot().as_str(),
        "task_digest":slot.task_digest().as_str(),"generation_id":slot.generation_id().as_str(),"profile_digest":slot.profile_digest().as_str(),
        "input_digest":slot.input_digest().as_str(),"files":slot.files().references().map(|f|json!({"path":f.path.as_str(),"sha256":f.digest.as_str(),"size":f.size})).collect::<Vec<_>>()})).collect();
    println!(
        "{}",
        json!({"scope":"fixed local profile/2 generated-byte binding","slots":slots,
        "not_claimed":["E01 registration","E02 session admission","Source transaction","SDK compilation or target execution","certification or publication"]})
    );
    Ok(())
}
pub(crate) fn directory(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("absolute non-traversing directory required".into());
    }
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("aliased or non-directory root".into());
        }
    }
    Ok(std::fs::canonicalize(path)?)
}
fn separate(input: &Path, output: &Path) -> Result<()> {
    if input.starts_with(output) || output.starts_with(input) {
        return Err("input and output domains overlap".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let in_id = std::fs::metadata(input)?;
        let out_id = std::fs::metadata(output)?;
        for (root, other) in [(input, &out_id), (output, &in_id)] {
            for ancestor in root.ancestors() {
                let actual = std::fs::metadata(ancestor)?;
                if actual.dev() == other.dev() && actual.ino() == other.ino() {
                    return Err("input and output domains alias".into());
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn capture_tree(root: &Path) -> Result<FrozenInputs> {
    let original_root = directory_identity(root)?;
    let workspace = LocalWorkspace::open(root)?;
    let mut names = BTreeSet::new();
    let mut directories = BTreeSet::new();
    enumerate(root, root, &mut names, &mut directories, 0)?;
    let refs = names
        .iter()
        .map(|name| Ok(actual_reference(name, &read(&workspace, name, FILE_LIMIT)?)))
        .collect::<Result<Vec<_>>>()?;
    let frozen = capture_inputs(&workspace, refs)?;
    let expected: BTreeSet<_> = frozen
        .directories()
        .map(|v| v.as_str().to_owned())
        .collect();
    if directories != expected {
        return Err("undeclared or empty output directory".into());
    }
    frozen.verify(&workspace)?;
    let mut after = BTreeSet::new();
    let mut after_dirs = BTreeSet::new();
    enumerate(root, root, &mut after, &mut after_dirs, 0)?;
    if names != after || directories != after_dirs {
        return Err("output inventory changed during capture".into());
    }
    if directory(root)? != root || directory_identity(root)? != original_root {
        return Err("output root changed during capture".into());
    }
    Ok(frozen)
}

#[cfg(unix)]
fn directory_identity(root: &Path) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("expected stable directory".into());
    }
    Ok((metadata.dev(), metadata.ino()))
}
#[cfg(not(unix))]
fn directory_identity(_: &Path) -> Result<(u64, u64)> {
    Err("confined composition probe requires a Unix host".into())
}
fn enumerate(
    root: &Path,
    current: &Path,
    files: &mut BTreeSet<String>,
    directories: &mut BTreeSet<String>,
    depth: usize,
) -> Result<()> {
    if depth > 32 || files.len() > 4096 || directories.len() > 8192 {
        return Err("output inventory budget exceeded".into());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let full = entry.path();
        let name = full
            .strip_prefix(root)?
            .to_str()
            .ok_or("non-UTF8 output name")?
            .to_owned();
        LogicalPath::new(&name)?;
        if kind.is_dir() {
            directories.insert(name);
            enumerate(root, &full, files, directories, depth + 1)?;
        } else if kind.is_file() {
            files.insert(name);
        } else {
            return Err("nonregular output or symlink".into());
        }
    }
    Ok(())
}

/// These five facts are the previously reviewed ordinary source recipe, not
/// whole C07 lifecycle coverage. Fixed source pins and actual IR symbols are
/// prerequisites; submitted responsibility rows never define what is required.
fn check_roles(input: &Inputs, bound: &BoundCompositionInputs, ir: &Document) -> Result<()> {
    let required = required_roles(input, bound, ir)?;
    compare_responsibility_sets(&required, bound.responsibilities())?;
    Ok(())
}
pub(crate) fn required_roles(
    input: &Inputs,
    bound: &BoundCompositionInputs,
    ir: &Document,
) -> Result<Vec<ResolvedResponsibility>> {
    const NATIVE: &str = "sdk/arkts/native-runtime/src/protocol.cpp";
    for (name, digest) in [
        (
            "sdk/arkts/facade/Boundary.ets.in",
            "sha256:dd3775ec075333a79efaeb236b6dc8da46622fea22b43a6e021cf4cfe19faa15",
        ),
        (
            "adapters/generators/arkts/profiles/snapshot-v1.json",
            "sha256:88729ffd4878a7aa991ce3a94f0edd3b3ec156751fbe5b12bca7cf050b2ced75",
        ),
        (
            NATIVE,
            "sha256:eb3978d2dda0e2ac1c67c485a4b74d66bf9b4a3155934ab895245fb23a0bcc25",
        ),
    ] {
        let (_, bytes) = input
            .frozen
            .files()
            .find(|(f, _)| f.path.as_str() == name)
            .ok_or("fixed semantic source absent")?;
        if Digest::of_bytes(bytes).as_str() != digest {
            return Err(format!("unsupported semantic source: {name}").into());
        }
    }
    let model = madb_contracts::contract(ir)?;
    let mut subjects = BTreeMap::new();
    for (name, kind) in [
        ("Engine.roundtrip", "sync"),
        ("Engine.observeState", "stream"),
        ("Subscription.requestStop", "sync"),
        ("Subscription.close", "async"),
    ] {
        let public_id = format!("madb.c07.{name}.v1");
        let symbol = model
            .symbols
            .iter()
            .find(|s| s.public_id.as_str() == public_id)
            .ok_or("fixed semantic symbol absent")?;
        let wire = ir.value()["symbols"]
            .as_array()
            .expect("checked symbols")
            .iter()
            .find(|s| s["public_id"] == public_id)
            .ok_or("symbol absent")?;
        if wire["shape"]["kind"] != kind {
            return Err(format!("fixed semantic symbol kind differs: {name}").into());
        }
        subjects.insert(
            name,
            ResolvedSubject::PublicSymbol(symbol.public_id.clone()),
        );
    }
    let package = &bound.composition().value()["packages"][0];
    let digest = Digest::new(package["source_digest"].as_str().ok_or("package digest")?)?;
    let native_ref = package["source"]["files"]
        .as_array()
        .expect("bound files")
        .iter()
        .find(|f| f["path"] == NATIVE)
        .ok_or("native owner absent from package")?;
    if native_ref["sha256"]
        != "sha256:eb3978d2dda0e2ac1c67c485a4b74d66bf9b4a3155934ab895245fb23a0bcc25"
    {
        return Err("package native owner differs from reviewed source".into());
    }
    let native_owner = ResolvedOwner::SdkAsset(AssetOrigin::Source {
        package: digest.clone(),
        paths: vec![path(NATIVE)],
    });
    let mut generated_paths: Vec<_> = SOURCE_PATHS.iter().map(|p| path(p)).collect();
    generated_paths.sort();
    let facade_owner = ResolvedOwner::SdkAsset(AssetOrigin::Generated {
        package: digest,
        producer: id(COMPONENT),
        slot: id(SLOT),
        paths: generated_paths,
    });
    let required: Vec<_> = [
        (
            "Engine.roundtrip",
            "projection",
            "call",
            "Engine.roundtrip",
            "facade",
            &facade_owner,
        ),
        (
            "Engine.observeState",
            "subscription",
            "subscription",
            "Engine.observeState",
            "source",
            &native_owner,
        ),
        (
            "Subscription.requestStop",
            "stop",
            "subscription",
            "Engine.observeState",
            "source",
            &native_owner,
        ),
        (
            "Subscription.close",
            "drain",
            "subscription",
            "Engine.observeState",
            "source",
            &native_owner,
        ),
        (
            "Subscription.close",
            "release",
            "subscription",
            "Engine.observeState",
            "facade",
            &facade_owner,
        ),
    ]
    .into_iter()
    .map(
        |(symbol, stage, kind, anchor, layer, owner)| ResolvedResponsibility {
            key: ResponsibilityKey {
                boundary: id("c-arkts"),
                subject: subjects[symbol].clone(),
                stage: id(stage),
                scope: ResolvedScope {
                    kind: id(kind),
                    anchor: subjects[anchor].clone(),
                    layer: id(layer),
                },
            },
            owner: owner.clone(),
        },
    )
    .collect();
    Ok(required)
}
