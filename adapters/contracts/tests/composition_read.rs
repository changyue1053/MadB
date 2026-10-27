// The existing fixed package fixture retains its original regressions.
include!("composition_admission.rs");
use madb_contracts::composition::read::{BoundCompositionRead, CompositionReadRequest};

fn reader_fixture() -> (Fixture, ExecutionRecipe, Vec<u8>) {
    let (mut f, mut r) = image_fixture();
    let mut reader = f.manifest["components"][0].clone();
    reader["id"] = json!("reader");
    reader["port"] = json!("contract-reader");
    reader["supported_features"] = f.profile["required_features"].clone();
    f.manifest["components"]
        .as_array_mut()
        .unwrap()
        .push(reader);
    let mut component = f.composition["components"][0].clone();
    component["id"] = json!("test.composed/reader");
    f.composition["components"]
        .as_array_mut()
        .unwrap()
        .push(component);
    f.recipe.components.push(ComponentRequirement {
        id: id("test.composed/reader"),
        port: id("contract-reader"),
        interprets_composition: true,
    });
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/schemas/read-request.schema.json"
    ))
    .unwrap();
    f.lock["schemas"]
        .as_array_mut()
        .unwrap()
        .push(file("schemas/read-request.schema.json", &bytes));
    f.memory
        .0
        .insert(path("schemas/read-request.schema.json"), bytes);
    f.repin_package();
    f.composition["components"][1]["sha256"] =
        f.composition["packages"][0]["source_digest"].clone();
    f.repin_composition();
    let bound = f.check().unwrap();
    let lock = encode(&f.lock);
    let profile = encode(&f.profile);
    let mut all = f
        .memory
        .0
        .iter()
        .map(|(p, b)| {
            (
                p.clone(),
                FileReference {
                    path: p.clone(),
                    digest: Digest::of_bytes(b),
                    size: b.len() as u64,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (p, b) in [("madb.lock", &lock), ("profiles/composed.json", &profile)] {
        all.insert(
            path(p),
            FileReference {
                path: path(p),
                digest: Digest::of_bytes(b),
                size: b.len() as u64,
            },
        );
    }
    r.inputs = all.into_values().collect();
    r.input_schemas = r
        .inputs
        .iter()
        .map(|f| {
            (
                f.path.clone(),
                id(match f.path.as_str() {
                    "madb.lock" => "madb.lock/1",
                    "profiles/composed.json" => "madb.profile/2",
                    "composition.json" => "madb.composition/1",
                    _ => "madb.source-input/1",
                }),
            )
        })
        .collect();
    r.response_path = path("contract.json");
    let inputs = f.lock["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let mut f = f.clone();
            f["schema"] = json!(r.input_schemas[&path(f["path"].as_str().unwrap())].as_str());
            f
        })
        .collect::<Vec<_>>();
    let q = json!({"schema":"madb.read-request/1","boundary_id":"fixture-boundary","selector":{"profile_id":f.profile["id"],"profile_digest":bound.profile_digest(),"route":f.profile["route"],"direction":f.profile["direction"],"target":f.profile["target"]},"component":f.composition["components"][1],"inputs":inputs,"lock":reference("madb.lock","madb.lock/1",&lock),"profile":reference("profiles/composed.json","madb.profile/2",&profile),"output_root":"outputs/read"});
    let bytes = encode(&q);
    r.request = FileReference {
        path: path("read-request.json"),
        digest: Digest::of_bytes(&bytes),
        size: bytes.len() as u64,
    };
    (f, r, bytes)
}
fn bind_reader(
    f: &Fixture,
    r: ExecutionRecipe,
    bytes: &[u8],
) -> Result<BoundCompositionRead, FormatError> {
    let bound = f.check()?;
    let features = f.profile["required_features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect::<Vec<_>>();
    let image = b"separately installed executable";
    let pins = check_composition_image(
        &bound,
        CompositionImageRequest {
            entrypoint_source: "bin/generator",
            component_id: "test.composed/reader",
            recipe: &r,
            installed_image: ProvidedFile {
                path: "installed-peer",
                bytes: image,
            },
            observed_host: "aarch64-apple-darwin",
            observed_source: "explicit-local-root",
            required_features: &features,
            policy: &ProcessPolicy::default(),
        },
    )?;
    BoundCompositionRead::new(
        bound,
        CompositionReadRequest {
            project: &doc(DocumentKind::Project, f.project.clone()),
            recipe: r,
            pins,
            producer: id("test.composed/reader"),
            output_root: path("outputs/read"),
            read_request_schema: path("schemas/read-request.schema.json"),
            request: ProvidedFile {
                path: "read-request.json",
                bytes,
            },
            lock: ProvidedFile {
                path: "madb.lock",
                bytes: &encode(&f.lock),
            },
            profile: ProvidedFile {
                path: "profiles/composed.json",
                bytes: &encode(&f.profile),
            },
            installed_image: ProvidedFile {
                path: "installed-peer",
                bytes: image,
            },
        },
    )
}
#[test]
fn explicit_reader_checks_real_input_closure_and_schema() {
    let (f, r, b) = reader_fixture();
    bind_reader(&f, r.clone(), &b).unwrap();
    let mut missing = r.clone();
    missing.inputs.pop();
    assert_eq!(bind_reader(&f, missing, &b).err().unwrap().code, "LOCK005");
    let mut schema = r.clone();
    schema
        .input_schemas
        .insert(path("profiles/composed.json"), id("madb.profile/1"));
    assert_eq!(bind_reader(&f, schema, &b).err().unwrap().code, "FMT003");
    let mut changed = b.clone();
    changed.push(b' ');
    assert_eq!(bind_reader(&f, r, &changed).err().unwrap().code, "FILE004");
}
#[test]
fn coherent_reader_request_cannot_select_another_component_or_domain() {
    let (f, r, b) = reader_fixture();
    for (key, value, code) in [
        (
            "component",
            json!({"id":"test.composed/other","version":"1.2.3","sha256":f.composition["components"][0]["sha256"]}),
            "LOCK004",
        ),
        ("output_root", json!("outputs/other"), "LOCK001"),
    ] {
        let mut q: Value = serde_json::from_slice(&b).unwrap();
        q[key] = value;
        let bytes = encode(&q);
        let mut r = r.clone();
        r.request.digest = Digest::of_bytes(&bytes);
        r.request.size = bytes.len() as u64;
        assert_eq!(bind_reader(&f, r, &bytes).err().unwrap().code, code);
    }
}

#[test]
fn reader_session_requires_explicit_composition_entry_and_exact_request_bytes() {
    use madb_contracts::extension_files::BoundExtensionFiles;
    use madb_contracts::extension_session::{HostSession, SessionOptions};
    use std::time::Duration;
    let (f, recipe, bytes) = reader_fixture();
    let proof = bind_reader(&f, recipe.clone(), &bytes).unwrap();
    let mut memory = Memory(f.memory.0.clone());
    memory.0.insert(path("madb.lock"), encode(&f.lock));
    memory
        .0
        .insert(path("profiles/composed.json"), encode(&f.profile));
    memory.0.insert(path("read-request.json"), bytes.clone());
    let mut refs = recipe.inputs.clone();
    refs.push(recipe.request.clone());
    let frozen = capture_inputs(&memory, refs).unwrap();
    let control = reference("inputs/read-request.json", "madb.read-request/1", &bytes);
    let files = BoundExtensionFiles::from_composition_read(&control, frozen, proof).unwrap();
    let options = || SessionOptions {
        component_id: "reader",
        session_id: "reader-session",
        request_id: "reader-request",
        protocol: "1.0",
        ir: "1.0",
        required_features: &["core.extension-composition@1"],
        handshake_timeout: Duration::from_secs(5),
        task_timeout: Duration::from_secs(30),
    };
    let manifest = files.composition_pins().unwrap().manifest();
    assert_eq!(
        HostSession::new(manifest, &files, options(), |_| Ok(()))
            .err()
            .unwrap()
            .code,
        "CapabilityUnsupported"
    );
    let features = files
        .composition_pins()
        .unwrap()
        .required_features()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut explicit = options();
    explicit.required_features = &features;
    assert!(HostSession::from_composition(&files, explicit, |_| Ok(())).is_ok());
    let mut wrong = control.clone();
    wrong["sha256"] = json!(byte_digest(b"other bytes"));
    let proof = bind_reader(&f, recipe, &bytes).unwrap();
    assert!(
        BoundExtensionFiles::from_composition_read(&wrong, files.inputs().clone(), proof).is_err()
    );
}
