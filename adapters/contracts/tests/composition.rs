use madb_app::{
    inputs::capture_inputs,
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    Document, DocumentKind, FormatError, SchemaRegistry, artifacts::ProvidedFile, composition::*,
    project_digest, source_digest,
};
use madb_model::{Digest, Id, LogicalPath, PublicId, contract::Direction};
use madb_protocol::{Value, byte_digest};
use serde_json::json;
use std::collections::BTreeMap;

fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn encode(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn file(name: &str, bytes: &[u8]) -> Value {
    json!({"path":name,"size":bytes.len(),"sha256":byte_digest(bytes)})
}
fn reference(name: &str, schema: &str, bytes: &[u8]) -> Value {
    let mut v = file(name, bytes);
    v["schema"] = json!(schema);
    v
}
fn doc(kind: DocumentKind, v: Value) -> Document {
    SchemaRegistry::new().unwrap().validate(kind, v).unwrap()
}
fn fixture(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../tests/fixtures/contracts/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
#[derive(Default)]
struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, p: &LogicalPath, _: usize) -> madb_app::Result<Option<Vec<u8>>> {
        Ok(self.0.get(p).cloned())
    }
}
struct Fixture {
    manifest: Value,
    composition: Value,
    profile: Value,
    project: Value,
    lock: Value,
    memory: Memory,
    schemas: CompositionSchemaPaths,
    recipe: RecipeRequirements,
}
impl Fixture {
    fn new() -> Self {
        let mut c = fixture("composition");
        c["packages"][0]["id"] = json!("test.composed");
        c["packages"][0]["version"] = json!("1.2.3");
        c["components"][0]["id"] = json!("test.composed/generator");
        c["components"][0]["version"] = json!("1.2.3");
        for a in c["assets"].as_array_mut().unwrap() {
            a["package"] = json!("test.composed");
        }
        c["assets"][0]["source"]["producer"] = json!("test.composed/generator");
        c["responsibilities"][0]["owner"]["package"] = json!("test.composed");
        let manifest = json!({"schema":"madb.extension/1","id":"test.composed","version":"1.2.3",
            "compatibility":{"protocol":["1.0"],"ir":["1.0"]},"languages":[],
            "components":[{"id":"generator","port":"generator","supported_features":["core.extension-composition@1"],"entrypoint":{"kind":"process","path":"bin/generator","args":[]}}],
            "dependencies":[],"routes":[],"assets":[{"kind":"sdk","path":"sdk"}],
            "execution":{"transport":"stdio-jsonl","network":"none","env":[]}});
        let mut profile = fixture("profile-v2");
        profile["tools"][0]["target"] = profile["target"].clone();
        profile["sdk"] = reference("sdk.json", "madb.fixture-sdk/1", b"SDK fixture bytes");
        let mut project = fixture("project");
        project["boundaries"][0]["id"] = json!("fixture-boundary");
        for k in ["route", "target", "direction"] {
            project["boundaries"][0][k] = profile[k].clone();
        }
        let schemas = CompositionSchemaPaths {
            common: path("schemas/common.schema.json"),
            project: path("schemas/project.schema.json"),
            lock: path("schemas/lock.schema.json"),
            profile: path("schemas/profile-v2.schema.json"),
            composition: path("schemas/composition.schema.json"),
            extension: path("schemas/extension-manifest.schema.json"),
            contract_ir: path("schemas/contract-ir.schema.json"),
        };
        let recipe = RecipeRequirements {
            id: id("fixture.arkts/ordinary@1"),
            selection: RecipeSelection {
                profile_id: id("fixture-composed-profile"),
                route: id("madb.arkts-c-napi"),
                direction: Direction::ConsumerToProvider,
                target: id("aarch64-linux-ohos"),
            },
            components: vec![ComponentRequirement {
                id: id("test.composed/generator"),
                port: id("generator"),
                interprets_composition: true,
            }],
            generated_slots: vec![GeneratedSlot {
                producer: id("test.composed/generator"),
                slot: id("ordinary-facade"),
                paths: vec![path("generated/Boundary.ets")],
            }],
        };
        let mut f = Self {
            manifest,
            composition: c,
            profile,
            project,
            lock: fixture("lock"),
            memory: Memory::default(),
            schemas,
            recipe,
        };
        f.memory
            .0
            .insert(path("sdk.json"), b"SDK fixture bytes".to_vec());
        f.memory
            .0
            .insert(path("native/fixture.rs"), b"fn fixture() {}".to_vec());
        let mut refs = vec![];
        for n in [
            "common",
            "project",
            "lock",
            "profile-v2",
            "composition",
            "extension-manifest",
            "contract-ir",
        ] {
            let name = format!("schemas/{n}.schema.json");
            let bytes =
                std::fs::read(format!("{}/../../docs/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            refs.push(file(&name, &bytes));
            f.memory.0.insert(path(&name), bytes);
        }
        f.lock["schemas"] = json!(refs);
        f.repin_package();
        f
    }
    fn repin_package(&mut self) {
        let bytes = encode(&self.manifest);
        let entries = [
            ("extension.json", bytes),
            ("bin/generator", b"fixture image".to_vec()),
            ("sdk/Runtime.ets", b"fixture Runtime source".to_vec()),
        ];
        let mut files = vec![];
        for (p, b) in entries {
            files.push(file(p, &b));
            self.memory
                .0
                .insert(path(&format!("packages/test.composed/1.2.3/{p}")), b);
        }
        let p = &mut self.composition["packages"][0];
        p["manifest"] = reference(
            "extension.json",
            "madb.extension/1",
            &encode(&self.manifest),
        );
        p["source"]["files"] = json!(files);
        p["source_digest"] = json!(source_digest(&p["source"]["files"]).unwrap());
        self.composition["components"][0]["sha256"] = p["source_digest"].clone();
        self.composition["assets"][1]["source"]["files"] = json!([files[2]]);
        self.repin_composition();
    }
    fn repin_composition(&mut self) {
        let bytes = encode(&self.composition);
        self.memory
            .0
            .insert(path("composition.json"), bytes.clone());
        self.profile["composition"]["file"] =
            reference("composition.json", "madb.composition/1", &bytes);
        self.profile["composition"]["semantic_digest"] = json!(
            composition_digest(&doc(DocumentKind::Composition, self.composition.clone())).unwrap()
        );
        self.lock["components"] = self.composition["components"].clone();
        self.lock["inputs"] = json!(
            self.memory
                .0
                .iter()
                .filter(|(p, _)| !p.as_str().starts_with("schemas/"))
                .map(|(p, b)| file(p.as_str(), b))
                .collect::<Vec<_>>()
        );
        self.repin_profile();
    }
    fn repin_profile(&mut self) {
        let r = reference(
            "profiles/composed.json",
            "madb.profile/2",
            &encode(&self.profile),
        );
        self.project["boundaries"][0]["profile"] = r.clone();
        self.lock["profiles"] = json!([{"id":self.profile["id"],"file":r,"semantic_digest":profile_v2_digest(&doc(DocumentKind::ProfileV2,self.profile.clone())).unwrap()}]);
        self.lock["project_digest"] =
            json!(project_digest(&doc(DocumentKind::Project, self.project.clone())).unwrap());
        self.lock["tools"] = self.profile["tools"].clone();
    }
    fn check(&self) -> Result<BoundCompositionInputs, FormatError> {
        let refs = self
            .memory
            .0
            .iter()
            .map(|(p, b)| FileReference {
                path: p.clone(),
                digest: Digest::of_bytes(b),
                size: b.len() as u64,
            })
            .collect();
        let frozen = capture_inputs(&self.memory, refs).unwrap();
        bind_composition_inputs(
            CompositionInputContext {
                project: &doc(DocumentKind::Project, self.project.clone()),
                lock: &doc(DocumentKind::Lock, self.lock.clone()),
                profile: ProvidedFile {
                    path: "profiles/composed.json",
                    bytes: &encode(&self.profile),
                },
                boundary_id: "fixture-boundary",
                inputs: &frozen,
                schema_paths: &self.schemas,
            },
            &self.recipe,
        )
    }
    fn required(&self) -> Vec<ResolvedResponsibility> {
        let symbol = ResolvedSubject::PublicSymbol(PublicId::new("fixture.call.v1").unwrap());
        vec![ResolvedResponsibility {
            key: ResponsibilityKey {
                boundary: id("fixture-boundary"),
                subject: symbol.clone(),
                stage: id("projection"),
                scope: ResolvedScope {
                    kind: id("call"),
                    anchor: symbol,
                    layer: id("facade"),
                },
            },
            owner: ResolvedOwner::SdkAsset(AssetOrigin::Generated {
                package: Digest::new(
                    self.composition["packages"][0]["source_digest"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap(),
                producer: id("test.composed/generator"),
                slot: id("ordinary-facade"),
                paths: vec![path("generated/Boundary.ets")],
            }),
        }]
    }
}
fn rejects(f: &Fixture, code: &str) {
    match f.check() {
        Err(e) => assert_eq!(e.code, code, "{e}"),
        Ok(_) => panic!("unexpected accepted input observation"),
    }
}

#[test]
fn independent_shared_vector_and_separate_profile_domains() {
    let c = doc(DocumentKind::Composition, fixture("composition"));
    assert_eq!(
        composition_digest(&c).unwrap(),
        fixture("profile-v2")["composition"]["semantic_digest"]
    );
    let p = doc(DocumentKind::ProfileV2, fixture("profile-v2"));
    assert_eq!(
        madb_contracts::profile_digest(&p).unwrap_err().code,
        "FMT003"
    );
    assert_eq!(
        profile_v2_digest(&doc(DocumentKind::Profile, fixture("profile")))
            .unwrap_err()
            .code,
        "FMT003"
    );
    let mut reordered = c.value().clone();
    for k in ["assets", "dependencies", "responsibilities"] {
        reordered[k].as_array_mut().unwrap().reverse();
    }
    reordered["packages"][0]["source"]["files"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(
        composition_digest(&c).unwrap(),
        composition_digest(&doc(DocumentKind::Composition, reordered)).unwrap()
    );
}
#[test]
fn real_frozen_byte_pins_and_independent_responsibility_facts_match() {
    let f = Fixture::new();
    let bound = f.check().unwrap();
    compare_responsibility_sets(&f.required(), bound.responsibilities()).unwrap();
    assert_eq!(
        bound.source_digest(),
        source_digest(&f.lock["inputs"]).unwrap()
    );
    assert_ne!(
        bound.source_digest(),
        f.composition["packages"][0]["source_digest"]
    );
}
#[test]
fn used_schema_is_required_and_exact_even_when_attacker_repins_bytes() {
    let mut f = Fixture::new();
    f.lock["schemas"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"] != "schemas/composition.schema.json");
    f.memory.0.remove(&path("schemas/composition.schema.json"));
    rejects(&f, "LOCK005");
    let mut f = Fixture::new();
    let name = "schemas/composition.schema.json";
    let bad = b"{}";
    f.memory.0.insert(path(name), bad.to_vec());
    for r in f.lock["schemas"].as_array_mut().unwrap() {
        if r["path"] == name {
            *r = file(name, bad);
        }
    }
    rejects(&f, "FILE004");
}
#[test]
fn sdk_and_authoritative_source_must_be_actual_locked_inputs() {
    let mut f = Fixture::new();
    f.lock["inputs"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"] != "sdk.json");
    f.memory.0.remove(&path("sdk.json"));
    rejects(&f, "LOCK005");
    let mut f = Fixture::new();
    f.lock["inputs"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"] != "native/fixture.rs");
    f.memory.0.remove(&path("native/fixture.rs"));
    rejects(&f, "LOCK005");
    let mut f = Fixture::new();
    f.profile["sdk"] = f.profile["composition"]["file"].clone();
    f.repin_profile();
    rejects(&f, "InputInvalid");
}
#[test]
fn repinned_wrong_port_missing_feature_and_example_package_are_not_eligible() {
    let mut f = Fixture::new();
    f.manifest["components"][0]["port"] = json!("contract-reader");
    f.repin_package();
    rejects(&f, "InputInvalid");
    let mut f = Fixture::new();
    f.manifest["components"][0]["supported_features"] = json!([]);
    f.repin_package();
    rejects(&f, "CapabilityUnsupported");
    let mut f = Fixture::new();
    f.manifest["example"] = json!(true);
    f.repin_package();
    assert!(f.check().is_err());
}
#[test]
fn future_output_aliases_do_not_hide_in_frozen_inputs() {
    for output in [
        "sdk.json",
        "SDK.json",
        "sdk.json/child",
        "packages/test.composed",
        "schemas/COMMON.schema.json",
        "profiles/composed.json",
    ] {
        let mut f = Fixture::new();
        f.recipe.generated_slots[0].paths = vec![path(output)];
        f.composition["assets"][0]["source"]["paths"] = json!([output]);
        f.repin_composition();
        rejects(&f, "InputInvalid");
    }
}
#[test]
fn source_alias_and_generated_slot_alias_are_rejected_after_repinning() {
    let mut f = Fixture::new();
    let mut alias = f.composition["assets"][1].clone();
    alias["id"] = json!("runtime-alias");
    f.composition["assets"].as_array_mut().unwrap().push(alias);
    f.repin_composition();
    rejects(&f, "InputInvalid");
    let mut f = Fixture::new();
    let mut alias = f.composition["assets"][0].clone();
    alias["id"] = json!("facade-alias");
    f.composition["assets"].as_array_mut().unwrap().push(alias);
    f.repin_composition();
    rejects(&f, "InputInvalid");
}
#[test]
fn missing_extra_and_conflicting_responsibilities_do_not_pass_by_self_reporting() {
    let f = Fixture::new();
    let bound = f.check().unwrap();
    let req = f.required();
    assert!(compare_responsibility_sets(&req, &[]).is_err());
    assert!(compare_responsibility_sets(&[], bound.responsibilities()).is_err());
    let mut extra = req.clone();
    let mut second = req[0].clone();
    second.key.stage = id("release");
    extra.push(second);
    assert!(compare_responsibility_sets(&req, &extra).is_err());
    let mut wrong = req.clone();
    wrong[0].owner = ResolvedOwner::Component {
        id: id("test.composed/generator"),
        package: Digest::of_bytes(b"other"),
    };
    assert!(compare_responsibility_sets(&req, &wrong).is_err());
    let duplicate = vec![req[0].clone(), req[0].clone()];
    assert!(compare_responsibility_sets(&duplicate, &duplicate).is_err());
}
#[test]
fn source_identity_and_snapshot_symbols_are_checked_without_claiming_ir_acceptance() {
    let f = Fixture::new();
    let bound = f.check().unwrap();
    let mut s = fixture("contract-ir");
    s["boundary_id"] = json!("fixture-boundary");
    s["selector"]["profile_id"] = f.profile["id"].clone();
    s["selector"]["profile_digest"] = json!(bound.profile_digest());
    for k in ["route", "direction", "target"] {
        s["selector"][k] = f.profile[k].clone();
    }
    s["source_digest"] = json!(bound.source_digest());
    // A schema-valid snapshot still lacks this recipe's symbol; the binding
    // checker cannot use the composition's own claims as proof of its presence.
    assert!(
        check_composition_snapshot_binding(&bound, &doc(DocumentKind::ContractIr, s.clone()))
            .is_err()
    );
    s["symbols"][0]["public_id"] = json!("fixture.call.v1");
    check_composition_snapshot_binding(&bound, &doc(DocumentKind::ContractIr, s.clone())).unwrap();
    s["source_digest"] = f.composition["packages"][0]["source_digest"].clone();
    assert_eq!(
        check_composition_snapshot_binding(&bound, &doc(DocumentKind::ContractIr, s))
            .unwrap_err()
            .code,
        "LOCK005"
    );
}
#[test]
fn coherent_selection_repin_cannot_expand_the_fixed_host_recipe() {
    for (key, value) in [
        ("target", "another-target"),
        ("direction", "host-callback"),
        ("route", "another-route"),
        ("profile_id", "another-profile"),
    ] {
        let mut f = Fixture::new();
        f.composition["selection"][key] = json!(value);
        if key == "profile_id" {
            f.profile["id"] = json!(value);
        } else {
            f.profile[key] = json!(value);
            f.project["boundaries"][0][key] = json!(value);
        }
        if key == "target" {
            f.profile["tools"][0]["target"] = json!(value);
        }
        f.repin_composition();
        rejects(&f, "LOCK001");
    }
}
#[test]
fn frozen_package_manifest_and_full_tool_tuple_cannot_drift() {
    let mut f = Fixture::new();
    f.memory.0.insert(
        path("packages/test.composed/1.2.3/bin/generator"),
        b"changed".to_vec(),
    );
    rejects(&f, "FILE004");
    let mut f = Fixture::new();
    f.composition["components"][0]["sha256"] = json!(byte_digest(b"fixture image"));
    f.repin_composition();
    rejects(&f, "LOCK004");
    for key in ["host", "target", "source", "version"] {
        let mut f = Fixture::new();
        f.lock["tools"][0][key] = json!("changed");
        rejects(&f, "LOCK004");
    }
    let mut f = Fixture::new();
    f.lock["tools"][0]["arguments"] = json!(["changed"]);
    rejects(&f, "LOCK004");
}
#[test]
fn dependencies_and_unknown_required_feature_are_not_silently_ignored() {
    let mut f = Fixture::new();
    f.composition["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(json!({"from":"runtime","to":"facade","scope":"target-runtime"}));
    f.repin_composition();
    rejects(&f, "InputInvalid");
    let mut f = Fixture::new();
    f.composition["dependencies"][0]["to"] = json!("missing");
    f.repin_composition();
    rejects(&f, "InputInvalid");
    let mut f = Fixture::new();
    f.profile["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("future.required@1"));
    f.profile["supported_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("future.required@1"));
    f.repin_profile();
    rejects(&f, "CapabilityUnsupported");
    let mut f = Fixture::new();
    f.profile["supported_features"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v != "core.extension-composition@1");
    f.repin_profile();
    rejects(&f, "CapabilityUnsupported");
}

fn freeze_profile_as_input(f: &mut Fixture, bytes: Vec<u8>) {
    f.lock["inputs"]
        .as_array_mut()
        .unwrap()
        .push(file("profiles/composed.json", &bytes));
    f.memory.0.insert(path("profiles/composed.json"), bytes);
}

#[test]
fn identical_profile_also_in_locked_inputs_is_a_legal_redundant_binding() {
    let mut f = Fixture::new();
    let bytes = encode(&f.profile);
    freeze_profile_as_input(&mut f, bytes);
    let bound = f.check().unwrap();
    assert_eq!(
        bound.source_digest(),
        source_digest(&f.lock["inputs"]).unwrap()
    );
}

#[test]
fn different_frozen_bytes_at_the_selected_profile_path_are_rejected() {
    let mut f = Fixture::new();
    let mut bytes = encode(&f.profile);
    bytes.push(b'\n');
    freeze_profile_as_input(&mut f, bytes);
    rejects(&f, "FILE004");
}

#[test]
fn redundant_profile_input_does_not_hide_generated_path_conflicts() {
    for output in [
        "profiles/composed.json",
        "profiles/COMPOSED.json",
        "profiles",
        "profiles/composed.json/child",
    ] {
        let mut f = Fixture::new();
        f.recipe.generated_slots[0].paths = vec![path(output)];
        f.composition["assets"][0]["source"]["paths"] = json!([output]);
        f.repin_composition();
        let bytes = encode(&f.profile);
        freeze_profile_as_input(&mut f, bytes);
        rejects(&f, "InputInvalid");
    }
}
