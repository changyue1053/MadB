//! Metadata semantics only. These fixtures are not observed declarations or a
//! registered executable package; the real generator probe supplies that proof.
use madb_app::{
    inputs::capture_inputs,
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    ContractError, Document, DocumentKind, SchemaRegistry,
    artifacts::ProvidedFile,
    composition::{
        ComponentRequirement, CompositionInputContext, CompositionSchemaPaths, GeneratedSlot,
        RecipeRequirements, RecipeSelection, composition_digest, profile_v2_digest,
    },
    native, policy_digest, project_digest, shape_digest, source_digest, structure_digest,
};
use madb_model::{Digest, Id, LogicalPath, contract::Direction};
use madb_protocol::{Value, byte_digest};
use serde_json::json;
use std::collections::BTreeMap;

fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn file(name: &str, data: &[u8]) -> Value {
    json!({"path":name,"size":data.len(),"sha256":byte_digest(data)})
}
fn reference(name: &str, schema: &str, data: &[u8]) -> Value {
    let mut v = file(name, data);
    v["schema"] = json!(schema);
    v
}
fn document(kind: DocumentKind, v: Value) -> Document {
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
    project: Value,
    profile: Value,
    lock: Value,
    seed: Value,
    memory: Memory,
    schemas: CompositionSchemaPaths,
    recipe: RecipeRequirements,
}
impl Fixture {
    fn new() -> Self {
        let mut profile = fixture("profile");
        profile["schema"] = json!("madb.profile/2");
        for key in ["required_features", "supported_features"] {
            profile[key]
                .as_array_mut()
                .unwrap()
                .push(json!("core.extension-composition@1"));
        }
        let project = fixture("project");
        let mut memory = Memory::default();
        memory
            .0
            .insert(path("native/fixture.rs"), b"fn fixture() {}".to_vec());
        memory
            .0
            .insert(path("sdk.json"), b"metadata test SDK".to_vec());
        profile["sdk"] = reference("sdk.json", "madb.fixture-sdk/1", b"metadata test SDK");
        let manifest = json!({"schema":"madb.extension/1","id":"test.metadata","version":"1.0.0",
            "compatibility":{"protocol":["1.0"],"ir":["1.0"]},"languages":[],
            "components":[{"id":"generator","port":"generator","supported_features":["core.extension-composition@1"],"entrypoint":{"kind":"process","path":"bin/generator","args":[]}}],
            "dependencies":[],"routes":[],"assets":[{"kind":"sdk","path":"sdk"}],
            "execution":{"transport":"stdio-jsonl","network":"none","env":[]}});
        let package_files = [
            ("extension.json", bytes(&manifest)),
            ("bin/generator", b"not an executed image".to_vec()),
            ("sdk/Runtime.ets", b"metadata fixture runtime".to_vec()),
        ];
        let refs: Vec<_> = package_files.iter().map(|(p, b)| file(p, b)).collect();
        for (p, b) in package_files {
            memory
                .0
                .insert(path(&format!("packages/test.metadata/1.0.0/{p}")), b);
        }
        let package_digest = source_digest(&json!(refs)).unwrap();
        let selection = json!({"profile_id":profile["id"],"route":profile["route"],
            "direction":profile["direction"],"target":profile["target"]});
        let composition = json!({"schema":"madb.composition/1","id":"metadata-composition",
            "recipe":"test.metadata/ordinary@1","selection":selection,
            "packages":[{"id":"test.metadata","version":"1.0.0",
                "manifest":reference("extension.json","madb.extension/1",&bytes(&manifest)),
                "source":{"kind":"file-set","files":refs},"source_digest":package_digest}],
            "components":[{"id":"test.metadata/generator","version":"1.0.0","sha256":package_digest}],
            "assets":[{"id":"facade","package":"test.metadata","source":{"kind":"generated","producer":"test.metadata/generator","output_slot":"facade","paths":["generated/Facade.h"]}},
                {"id":"runtime","package":"test.metadata","source":{"kind":"source","files":[refs[2]]}}],
            "dependencies":[{"from":"facade","to":"runtime","scope":"target-runtime"}],
            "responsibilities":[{"boundary":project["boundaries"][0]["id"],
                "subject":{"kind":"public-symbol","public_id":"fixture.create.v1"},"stage":"projection",
                "scope":{"kind":"call","anchor":{"kind":"public-symbol","public_id":"fixture.create.v1"},"layer":"facade"},
                "owner":{"kind":"sdk-asset","package":"test.metadata","asset":"facade"}}]});
        let data = bytes(&composition);
        profile["composition"] = json!({"id":"metadata-composition",
            "semantic_digest":composition_digest(&document(DocumentKind::Composition, composition.clone())).unwrap(),
            "file":reference("composition.json","madb.composition/1",&data)});
        memory.0.insert(path("composition.json"), data);
        let mut lock = fixture("lock");
        lock["components"] = composition["components"].clone();
        lock["inputs"] = json!(
            memory
                .0
                .iter()
                .map(|(p, b)| file(p.as_str(), b))
                .collect::<Vec<_>>()
        );
        let mut schema_refs = vec![];
        for name in [
            "common",
            "project",
            "lock",
            "profile-v2",
            "composition",
            "extension-manifest",
            "contract-ir",
        ] {
            let p = format!("schemas/{name}.schema.json");
            let b =
                std::fs::read(format!("{}/../../docs/{p}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            schema_refs.push(file(&p, &b));
            memory.0.insert(path(&p), b);
        }
        lock["schemas"] = json!(schema_refs);
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
            id: id("test.metadata/ordinary@1"),
            selection: RecipeSelection {
                profile_id: id(profile["id"].as_str().unwrap()),
                route: id(profile["route"].as_str().unwrap()),
                target: id(profile["target"].as_str().unwrap()),
                direction: Direction::ConsumerToProvider,
            },
            components: vec![ComponentRequirement {
                id: id("test.metadata/generator"),
                port: id("generator"),
                interprets_composition: true,
            }],
            generated_slots: vec![GeneratedSlot {
                producer: id("test.metadata/generator"),
                slot: id("facade"),
                paths: vec![path("generated/Facade.h")],
            }],
        };
        let mut seed = fixture("contract-ir");
        seed["policies"] = json!([]);
        let zero = json!(format!("sha256:{}", "0".repeat(64)));
        seed["structure_digest"] = zero.clone();
        seed["policy_digest"] = zero.clone();
        for symbol in seed["symbols"].as_array_mut().unwrap() {
            symbol["source_id"] = json!(format!("source:v1:{}", "0".repeat(64)));
            symbol["shape_digest"] = zero.clone();
        }
        seed["source_digest"] = json!(source_digest(&lock["inputs"]).unwrap());
        let mut value = Self {
            project,
            profile,
            lock,
            seed,
            memory,
            schemas,
            recipe,
        };
        value.repin_profile();
        value
    }
    fn repin_profile(&mut self) {
        let reference = reference("profile.json", "madb.profile/2", &bytes(&self.profile));
        self.project["boundaries"][0]["profile"] = reference.clone();
        let digest =
            profile_v2_digest(&document(DocumentKind::ProfileV2, self.profile.clone())).unwrap();
        self.lock["profiles"] =
            json!([{"id":self.profile["id"],"file":reference,"semantic_digest":digest}]);
        self.lock["tools"] = self.profile["tools"].clone();
        self.repin_project();
        self.seed["selector"]["profile_digest"] = json!(digest);
    }
    fn repin_project(&mut self) {
        self.lock["project_digest"] =
            json!(project_digest(&document(DocumentKind::Project, self.project.clone())).unwrap());
    }
    fn finish(&self) -> Result<Document, ContractError> {
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
        let result = native::finalize_composition_metadata(
            &bytes(&self.seed),
            CompositionInputContext {
                project: &document(DocumentKind::Project, self.project.clone()),
                lock: &document(DocumentKind::Lock, self.lock.clone()),
                profile: ProvidedFile {
                    path: "profile.json",
                    bytes: &bytes(&self.profile),
                },
                boundary_id: self.project["boundaries"][0]["id"].as_str().unwrap(),
                inputs: &frozen,
                schema_paths: &self.schemas,
            },
            &self.recipe,
        )?;
        Ok(SchemaRegistry::new()
            .unwrap()
            .decode(DocumentKind::ContractIr, &result)
            .unwrap())
    }
}
fn rejects(f: &Fixture, expected: &str) {
    match f.finish().unwrap_err() {
        ContractError::Format(error) => assert_eq!(error.code, expected, "{error}"),
        ContractError::Findings(findings) => {
            assert!(findings.iter().any(|f| f.code == expected), "{findings:?}")
        }
    }
}

#[test]
fn bound_profile_two_completes_metadata_without_relabelling_profile_one() {
    let f = Fixture::new();
    let complete = f.finish().unwrap();
    let v = complete.value();
    assert_eq!(v["selector"], f.seed["selector"]);
    assert_eq!(v["source_digest"], f.seed["source_digest"]);
    assert_eq!(v["required_features"], f.seed["required_features"]);
    assert_eq!(v["structure_digest"], structure_digest(&complete).unwrap());
    assert_eq!(v["policy_digest"], policy_digest(&complete).unwrap());
    for symbol in v["symbols"].as_array().unwrap() {
        assert_eq!(
            symbol["shape_digest"],
            shape_digest(&complete, symbol["public_id"].as_str().unwrap()).unwrap()
        );
        assert_ne!(symbol["source_id"], f.seed["symbols"][0]["source_id"]);
    }
    let owned = v["policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["symbol_id"] == "fixture.create.v1" && p["slot"]["kind"] == "return")
        .unwrap();
    assert_eq!(owned["values"]["ownership"], "transfer");
    assert_eq!(owned["values"]["releaser"], "fixture.release_handle.v1");
    assert_eq!(f.finish().unwrap().value(), v);
    assert!(
        native::finalize_metadata(&bytes(&f.seed), &bytes(&f.project), &bytes(&f.profile)).is_err()
    );
    assert!(
        native::finalize_envelope(&bytes(
            &json!({"contract":f.seed,"project":f.project,"profile":f.profile})
        ))
        .is_err()
    );
    assert!(
        madb_contracts::check_contract(&complete, &document(DocumentKind::ProfileV2, f.profile))
            .is_err()
    );
}

#[test]
fn input_or_selector_changes_do_not_silently_retarget_the_seed() {
    for field in [
        "profile_digest",
        "profile_id",
        "route",
        "target",
        "direction",
    ] {
        let mut f = Fixture::new();
        f.seed["selector"][field] = if field == "profile_digest" {
            json!(format!("sha256:{}", "9".repeat(64)))
        } else if field == "direction" {
            json!("host-callback")
        } else {
            json!("different")
        };
        rejects(&f, "LOCK001");
    }
    let mut f = Fixture::new();
    f.seed["source_digest"] = json!(format!("sha256:{}", "9".repeat(64)));
    rejects(&f, "LOCK005");
    let mut f = Fixture::new();
    f.project["policies"][0]["values"]["blocking"] = json!("unbounded");
    rejects(&f, "LOCK002");
    let mut f = Fixture::new();
    f.recipe.selection.target = id("other-target");
    rejects(&f, "LOCK001");
}

#[test]
fn pre_resolved_policy_and_unknown_ir_features_are_rejected() {
    let mut f = Fixture::new();
    f.seed["policies"] = fixture("contract-ir")["policies"].clone();
    rejects(&f, "NATIVE001");
    let mut f = Fixture::new();
    f.seed["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("core.extension-composition@1"));
    rejects(&f, "CAP002");
}

#[test]
fn bound_profile_defaults_are_effective_and_unsupported_semantics_reject() {
    let mut f = Fixture::new();
    f.profile["defaults"]["origin"]["id"] = json!("composed-default");
    f.repin_profile();
    let output = f.finish().unwrap();
    assert!(
        output.value()["policies"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["provenance"].as_array().unwrap())
            .any(|p| p["origin"]["id"] == "composed-default")
    );
    let mut f = Fixture::new();
    f.profile["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("unknown.required@1"));
    f.profile["supported_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("unknown.required@1"));
    f.repin_profile();
    rejects(&f, "CapabilityUnsupported");
    let mut f = Fixture::new();
    f.profile["restrictions"] =
        json!([{"feature":"core.resources@1","reason":"unsupported in this profile"}]);
    f.repin_profile();
    rejects(&f, "CAP002");
}

#[test]
fn schema_and_source_bytes_must_be_actually_bound() {
    let mut f = Fixture::new();
    f.memory
        .0
        .get_mut(&path("schemas/profile-v2.schema.json"))
        .unwrap()
        .push(b' ');
    rejects(&f, "FILE004");
    let mut f = Fixture::new();
    f.memory
        .0
        .get_mut(&path("native/fixture.rs"))
        .unwrap()
        .push(b' ');
    rejects(&f, "FILE004");
}

#[test]
fn invented_semantic_hashes_are_recomputed_but_invalid_structure_is_not_hidden() {
    let mut f = Fixture::new();
    let changed = json!(format!("sha256:{}", "a".repeat(64)));
    f.seed["structure_digest"] = changed.clone();
    f.seed["policy_digest"] = changed.clone();
    f.seed["symbols"][0]["shape_digest"] = changed.clone();
    let result = f.finish().unwrap();
    assert_ne!(result.value()["structure_digest"], changed);
    assert_ne!(result.value()["policy_digest"], changed);
    assert_ne!(result.value()["symbols"][0]["shape_digest"], changed);
    let mut f = Fixture::new();
    f.seed["symbols"][0]["shape"]["result"] = json!("undeclared-type");
    // Shape hashing resolves referenced types before the completed IR rule pass.
    rejects(&f, "HASH004");
}
