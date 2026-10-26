use madb_app::{
    inputs::capture_inputs,
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    Document, DocumentKind, FormatError, SchemaRegistry, artifacts::ProvidedFile, composition::*,
    project_digest, source_digest,
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
}

use madb_app::inputs::FrozenInputs;
use madb_contracts::{
    execution::{ExecutionRecipe, ExecutionTool, generation_task_digest},
    generation_id, policy_digest, shape_digest, structure_digest,
};
fn ref_value(f: &FileReference, schema: &str) -> Value {
    json!({"path":f.path.as_str(),"sha256":f.digest.as_str(),"size":f.size,"schema":schema})
}
fn file_ref(p: &str, b: &[u8]) -> FileReference {
    FileReference {
        path: path(p),
        digest: Digest::of_bytes(b),
        size: b.len() as u64,
    }
}
fn frozen(m: &Memory) -> FrozenInputs {
    capture_inputs(
        m,
        m.0.iter().map(|(p, b)| file_ref(p.as_str(), b)).collect(),
    )
    .unwrap()
}
fn projection(r: &ExecutionRecipe) -> Value {
    json!({"id":r.id.as_str(),"tool":{"id":r.tool.id.as_str(),"version":r.tool.version,"sha256":r.tool.digest.as_str()},"arguments":r.arguments,"environment":r.environment.iter().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>(),"working_directory":r.working_directory.as_str(),"response_path":r.response_path.as_str(),"inputs":r.inputs.iter().map(|f|ref_value(f,r.input_schemas[&f.path].as_str())).collect::<Vec<_>>()})
}
struct Outputs {
    base: Fixture,
    bound: BoundCompositionInputs,
    ir: Value,
    requests: Vec<Value>,
    recipes: Vec<ExecutionRecipe>,
    results: Vec<Value>,
    memories: Vec<Memory>,
    required: Vec<(Id, Vec<Id>, LogicalPath)>,
    installed: Vec<u8>,
    schemas: GenerationSchemaPaths,
}
impl Outputs {
    fn new(two_producers: bool) -> Self {
        Self::with_profile_input(two_producers, false)
    }
    fn with_profile_input(two_producers: bool, profile_is_input: bool) -> Self {
        let mut base = Fixture::new();
        let image = b"independent installed generator image".to_vec();
        base.profile["tools"][0]["sha256"] = json!(byte_digest(&image));
        for n in ["generate-request", "output-set"] {
            let name = format!("schemas/{n}.schema.json");
            let b =
                std::fs::read(format!("{}/../../docs/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            base.lock["schemas"]
                .as_array_mut()
                .unwrap()
                .push(file(&name, &b));
            base.memory.0.insert(path(&name), b);
        }
        let second = if two_producers {
            "test.composed/generator2"
        } else {
            "test.composed/generator"
        };
        if two_producers {
            let mut m = base.manifest["components"][0].clone();
            m["id"] = json!("generator2");
            base.manifest["components"].as_array_mut().unwrap().push(m);
            let mut c = base.composition["components"][0].clone();
            c["id"] = json!(second);
            base.composition["components"]
                .as_array_mut()
                .unwrap()
                .push(c);
            base.recipe.components.push(ComponentRequirement {
                id: id(second),
                port: id("generator"),
                interprets_composition: true,
            });
        }
        base.composition["assets"].as_array_mut().unwrap().push(json!({"id":"extra","package":"test.composed","source":{"kind":"generated","producer":second,"output_slot":"extra-slot","paths":["generated/Extra.ets"]}}));
        base.recipe.generated_slots.push(GeneratedSlot {
            producer: id(second),
            slot: id("extra-slot"),
            paths: vec![path("generated/Extra.ets")],
        });
        base.repin_package();
        let source = base.composition["packages"][0]["source_digest"].clone();
        for c in base.composition["components"].as_array_mut().unwrap() {
            c["sha256"] = source.clone();
        }
        base.repin_composition();
        if profile_is_input {
            let bytes = encode(&base.profile);
            base.lock["inputs"]
                .as_array_mut()
                .unwrap()
                .push(file("profiles/composed.json", &bytes));
            base.memory.0.insert(path("profiles/composed.json"), bytes);
        }
        let bound = base.check().unwrap();
        let mut ir = fixture("contract-ir");
        ir["boundary_id"] = json!("fixture-boundary");
        ir["selector"] = json!({"profile_id":base.profile["id"],"profile_digest":bound.profile_digest(),"route":base.profile["route"],"target":base.profile["target"],"direction":base.profile["direction"]});
        ir["source_digest"] = json!(bound.source_digest());
        ir["symbols"][0]["public_id"] = json!("fixture.call.v1");
        for i in 0..ir["symbols"].as_array().unwrap().len() {
            let public = ir["symbols"][i]["public_id"].as_str().unwrap().to_owned();
            ir["symbols"][i]["shape_digest"] =
                json!(shape_digest(&doc(DocumentKind::ContractIr, ir.clone()), &public).unwrap());
        }
        ir["structure_digest"] =
            json!(structure_digest(&doc(DocumentKind::ContractIr, ir.clone())).unwrap());
        ir["policy_digest"] =
            json!(policy_digest(&doc(DocumentKind::ContractIr, ir.clone())).unwrap());
        let required = if two_producers {
            vec![
                (
                    id("test.composed/generator"),
                    vec![id("ordinary-facade")],
                    path("out/first"),
                ),
                (id(second), vec![id("extra-slot")], path("out/second")),
            ]
        } else {
            vec![(
                id("test.composed/generator"),
                vec![id("ordinary-facade"), id("extra-slot")],
                path("out/first"),
            )]
        };
        let mut f = Self {
            base,
            bound,
            ir,
            requests: vec![],
            recipes: vec![],
            results: vec![],
            memories: vec![],
            required,
            installed: image,
            schemas: GenerationSchemaPaths {
                generate_request: path("schemas/generate-request.schema.json"),
                output_set: path("schemas/output-set.schema.json"),
            },
        };
        for i in 0..f.required.len() {
            f.add_step(i);
        }
        f
    }
    fn add_step(&mut self, i: usize) {
        let lock = encode(&self.base.lock);
        let profile = encode(&self.base.profile);
        let ir = encode(&self.ir);
        let mut all = self
            .base
            .memory
            .0
            .iter()
            .map(|(p, b)| (p.clone(), file_ref(p.as_str(), b)))
            .collect::<BTreeMap<_, _>>();
        for (p, b) in [
            ("madb.lock", &lock),
            ("profiles/composed.json", &profile),
            ("contract.json", &ir),
        ] {
            all.insert(path(p), file_ref(p, b));
        }
        let input_schemas = all
            .keys()
            .map(|p| {
                (
                    p.clone(),
                    id(if p.as_str().starts_with("schemas/") {
                        "madb.schema/1"
                    } else {
                        match p.as_str() {
                            "madb.lock" => "madb.lock/1",
                            "profiles/composed.json" => "madb.profile/2",
                            "contract.json" => "madb.contract-ir/1",
                            "composition.json" => "madb.composition/1",
                            "sdk.json" => "madb.fixture-sdk/1",
                            _ => "madb.source-input/1",
                        }
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let tool = &self.base.profile["tools"][0];
        let mut r = ExecutionRecipe {
            id: id(&format!("generate-{i}")),
            request: file_ref(&format!("request-{i}.json"), b"pending"),
            tool: ExecutionTool {
                id: id(tool["id"].as_str().unwrap()),
                version: tool["version"].as_str().unwrap().into(),
                digest: Digest::of_bytes(&self.installed),
            },
            installed_tool: file_ref("installed/generator", &self.installed),
            arguments: vec!["generate".into()],
            environment: BTreeMap::new(),
            working_directory: path("work"),
            response_path: path("output-set.json"),
            inputs: all.into_values().collect(),
            input_schemas,
        };
        let mut q = fixture("generate-request");
        q["boundary_id"] = self.ir["boundary_id"].clone();
        q["selector"] = self.ir["selector"].clone();
        q["inputs"] = json!(
            self.base.lock["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| {
                    let mut f = f.clone();
                    f["schema"] =
                        json!(r.input_schemas[&path(f["path"].as_str().unwrap())].as_str());
                    f
                })
                .collect::<Vec<_>>()
        );
        q["component"] = self.base.composition["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == self.required[i].0.as_str())
            .unwrap()
            .clone();
        q["lock"] = reference("madb.lock", "madb.lock/1", &lock);
        q["profile"] = reference("profiles/composed.json", "madb.profile/2", &profile);
        q["contract"] = reference("contract.json", "madb.contract-ir/1", &ir);
        q["output_root"] = json!(self.required[i].2.as_str());
        q["input_digest"] = json!(self.bound.source_digest());
        q["contract_digest"] = self.ir["structure_digest"].clone();
        q["policy_digest"] = self.ir["policy_digest"].clone();
        q["task_digest"] = json!(
            generation_task_digest(
                &doc(DocumentKind::GenerateRequest, q.clone()),
                &projection(&r)
            )
            .unwrap()
        );
        r.request = file_ref(&format!("request-{i}.json"), &encode(&q));
        let mut memory = Memory::default();
        let mut files = vec![];
        for asset in self.base.composition["assets"].as_array().unwrap() {
            let s = &asset["source"];
            if s["kind"] == "generated" && s["producer"] == self.required[i].0.as_str() {
                for p in s["paths"].as_array().unwrap() {
                    let name = p.as_str().unwrap();
                    let b = format!("real fixture file {name}").into_bytes();
                    let mut entry = file(name, &b);
                    entry["sources"] = json!(["fixture.call.v1"]);
                    files.push(entry);
                    memory.0.insert(path(name), b);
                }
            }
        }
        let mut out = json!({"schema":"madb.output-set/1","generation_id":byte_digest(b"placeholder"),"input_digest":q["input_digest"],"task_digest":q["task_digest"],"profile_digest":q["selector"]["profile_digest"],"manifest_path":"output-set.json","files":files});
        out["generation_id"] =
            json!(generation_id(&doc(DocumentKind::OutputSet, out.clone())).unwrap());
        memory.0.insert(path("output-set.json"), encode(&out));
        self.requests.push(q);
        self.recipes.push(r);
        self.results.push(out);
        self.memories.push(memory);
    }
    fn repin_request(&mut self, i: usize) {
        let task = generation_task_digest(
            &doc(DocumentKind::GenerateRequest, self.requests[i].clone()),
            &projection(&self.recipes[i]),
        )
        .unwrap();
        self.requests[i]["task_digest"] = json!(task);
        let p = self.recipes[i].request.path.as_str().to_owned();
        self.recipes[i].request = file_ref(&p, &encode(&self.requests[i]));
        self.results[i]["task_digest"] = self.requests[i]["task_digest"].clone();
        self.repin_output(i);
    }
    fn repin_output(&mut self, i: usize) {
        self.results[i]["generation_id"] =
            json!(generation_id(&doc(DocumentKind::OutputSet, self.results[i].clone())).unwrap());
        self.memories[i].0.insert(
            path(self.results[i]["manifest_path"].as_str().unwrap()),
            encode(&self.results[i]),
        );
    }
    fn check(&self) -> Result<BoundGeneratedOutputs, FormatError> {
        let outputs = self.memories.iter().map(frozen).collect::<Vec<_>>();
        let requests = self.requests.iter().map(encode).collect::<Vec<_>>();
        let req = self
            .required
            .iter()
            .zip(&self.recipes)
            .map(
                |((producer, slots, root), recipe)| GenerationStepRequirement {
                    recipe,
                    producer: producer.clone(),
                    output_slots: slots.clone(),
                    output_root: root.clone(),
                },
            )
            .collect::<Vec<_>>();
        let observations = self
            .recipes
            .iter()
            .enumerate()
            .map(|(i, r)| GenerationStepObservation {
                step_id: r.id.clone(),
                request: ProvidedFile {
                    path: r.request.path.as_str(),
                    bytes: &requests[i],
                },
                installed_tool: ProvidedFile {
                    path: r.installed_tool.path.as_str(),
                    bytes: &self.installed,
                },
                outputs: &outputs[i],
            })
            .collect::<Vec<_>>();
        bind_generated_outputs(
            &self.bound,
            GenerationBindingContext {
                lock: ProvidedFile {
                    path: "madb.lock",
                    bytes: &encode(&self.base.lock),
                },
                profile: ProvidedFile {
                    path: "profiles/composed.json",
                    bytes: &encode(&self.base.profile),
                },
                contract: ProvidedFile {
                    path: "contract.json",
                    bytes: &encode(&self.ir),
                },
                schema_paths: &self.schemas,
                required_steps: &req,
            },
            &observations,
        )
    }
}
fn reject_output(f: &Outputs, code: &str) {
    match f.check() {
        Err(e) => assert_eq!(e.code, code, "{e}"),
        Ok(_) => panic!("unexpected generated binding"),
    }
}
#[test]
fn one_producer_multiple_slots_and_two_producers_bind_actual_bytes() {
    for multiple in [false, true] {
        let f = Outputs::new(multiple);
        let before = encode(&f.base.composition);
        let result = f.check().unwrap();
        assert_eq!(result.slots().len(), 2);
        assert_eq!(result.output_sets().count(), if multiple { 2 } else { 1 });
        for (_, files) in result.output_sets() {
            madb_contracts::execution::source_manifest(files).unwrap();
        }
        for slot in result.slots() {
            assert_eq!(slot.files().references().count(), 1);
            for (r, b) in slot.files().files() {
                assert_eq!(r.digest, Digest::of_bytes(b));
                assert_eq!(
                    b,
                    format!("real fixture file {}", r.path.as_str()).as_bytes()
                );
            }
            assert_eq!(slot.profile_digest().as_str(), f.bound.profile_digest());
        }
        assert_eq!(before, encode(&f.base.composition));
    }
}
#[test]
fn repinned_request_cannot_change_producer_or_bound_context() {
    for (field, value, code) in [
        (
            "component",
            json!({"id":"other/generator","version":"1.2.3","sha256":byte_digest(b"other")}),
            "LOCK004",
        ),
        ("input_digest", json!(byte_digest(b"other")), "LOCK005"),
        ("contract_digest", json!(byte_digest(b"other")), "LOCK005"),
        ("policy_digest", json!(byte_digest(b"other")), "LOCK005"),
        ("output_root", json!("other/domain"), "LOCK001"),
    ] {
        let mut f = Outputs::new(false);
        f.requests[0][field] = value;
        f.repin_request(0);
        reject_output(&f, code);
    }
    let mut f = Outputs::new(false);
    f.requests[0]["selector"]["target"] = json!("other-target");
    f.repin_request(0);
    reject_output(&f, "LOCK001");
}
#[test]
fn repinned_result_cannot_change_lineage_or_claim_absent_sources() {
    for field in ["input_digest", "task_digest", "profile_digest"] {
        let mut f = Outputs::new(false);
        f.results[0][field] = json!(byte_digest(b"different"));
        f.repin_output(0);
        reject_output(&f, "PORT002");
    }
    let mut f = Outputs::new(false);
    f.results[0]["files"][0]["sources"] = json!(["missing.symbol"]);
    f.repin_output(0);
    reject_output(&f, "EXECUTION002");
}
#[test]
fn exact_output_inventory_and_bytes_are_required() {
    let mut f = Outputs::new(false);
    f.memories[0]
        .0
        .insert(path("unexpected.txt"), b"extra".to_vec());
    reject_output(&f, "EXECUTION002");
    let mut f = Outputs::new(false);
    f.memories[0].0.remove(&path("generated/Extra.ets"));
    reject_output(&f, "FILE003");
    let mut f = Outputs::new(false);
    f.memories[0]
        .0
        .insert(path("generated/Extra.ets"), b"changed".to_vec());
    reject_output(&f, "FILE004");
    let mut f = Outputs::new(false);
    f.results[0]["files"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"] != "generated/Extra.ets");
    f.memories[0].0.remove(&path("generated/Extra.ets"));
    f.repin_output(0);
    reject_output(&f, "InputInvalid");
}
#[test]
fn complete_fixed_slots_and_steps_cannot_be_inferred_from_results() {
    let mut f = Outputs::new(false);
    f.required[0].1.pop();
    reject_output(&f, "InputInvalid");
    let mut f = Outputs::new(false);
    f.required[0].1.push(id("ordinary-facade"));
    reject_output(&f, "InputInvalid");
    let mut f = Outputs::new(true);
    f.required.pop();
    f.recipes.pop();
    f.requests.pop();
    f.results.pop();
    f.memories.pop();
    reject_output(&f, "InputInvalid");
    let mut f = Outputs::new(true);
    f.required[1].0 = id("test.composed/generator");
    reject_output(&f, "InputInvalid");
}
#[test]
fn installed_image_is_separate_from_component_source_identity() {
    let mut f = Outputs::new(false);
    f.installed.push(0);
    reject_output(&f, "FILE004");
    let mut f = Outputs::new(false);
    f.recipes[0].tool.digest = Digest::new(
        f.base.composition["components"][0]["sha256"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    f.recipes[0].installed_tool.digest = f.recipes[0].tool.digest.clone();
    f.repin_request(0);
    reject_output(&f, "LOCK004");
}
#[test]
fn fixed_recipe_task_and_input_closure_are_checked() {
    let mut f = Outputs::new(false);
    f.recipes[0].arguments.push("changed".into());
    reject_output(&f, "PLAN002");
    let mut f = Outputs::new(false);
    f.recipes[0].inputs.pop();
    f.repin_request(0);
    reject_output(&f, "LOCK005");
    let mut f = Outputs::new(false);
    f.recipes[0]
        .input_schemas
        .insert(path("unexpected"), id("madb.source-input/1"));
    f.repin_request(0);
    reject_output(&f, "LOCK005");
}

#[test]
fn generation_schemas_must_be_the_bundled_decoders_even_after_repinning() {
    let mut f = Outputs::new(false);
    f.base.lock["schemas"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["path"] != "schemas/output-set.schema.json");
    f.base
        .memory
        .0
        .remove(&path("schemas/output-set.schema.json"));
    f.bound = f.base.check().unwrap();
    reject_output(&f, "LOCK005");
    let mut f = Outputs::new(false);
    let name = "schemas/output-set.schema.json";
    f.base.memory.0.insert(path(name), b"{}".to_vec());
    for r in f.base.lock["schemas"].as_array_mut().unwrap() {
        if r["path"] == name {
            *r = file(name, b"{}");
        }
    }
    f.bound = f.base.check().unwrap();
    reject_output(&f, "FILE004");
    let mut f = Outputs::new(false);
    f.schemas.output_set = f.schemas.generate_request.clone();
    reject_output(&f, "LOCK006");
}
#[test]
fn context_bytes_and_ir_identities_are_not_replaced_by_mutually_agreeing_claims() {
    let mut f = Outputs::new(false);
    f.base.lock["tools"][0]["version"] = json!("changed");
    reject_output(&f, "LOCK003");
    let mut f = Outputs::new(false);
    f.base.profile["deployment_target"] = json!("changed");
    reject_output(&f, "FILE004");
    let mut f = Outputs::new(false);
    f.ir["structure_digest"] = json!(byte_digest(b"changed"));
    reject_output(&f, "HASH001");
    let mut f = Outputs::new(false);
    f.ir["policy_digest"] = json!(byte_digest(b"changed"));
    reject_output(&f, "HASH002");
    let mut f = Outputs::new(false);
    f.ir["symbols"][0]["shape_digest"] = json!(byte_digest(b"changed"));
    f.ir["structure_digest"] =
        json!(structure_digest(&doc(DocumentKind::ContractIr, f.ir.clone())).unwrap());
    f.ir["policy_digest"] =
        json!(policy_digest(&doc(DocumentKind::ContractIr, f.ir.clone())).unwrap());
    reject_output(&f, "HASH003");
}
#[test]
fn manifest_location_generation_and_extra_declared_outputs_are_checked() {
    let mut f = Outputs::new(false);
    f.results[0]["generation_id"] = json!(byte_digest(b"wrong"));
    f.memories[0]
        .0
        .insert(path("output-set.json"), encode(&f.results[0]));
    reject_output(&f, "FILE006");
    let mut f = Outputs::new(false);
    f.results[0]["manifest_path"] = json!("other-manifest.json");
    f.memories[0].0.remove(&path("output-set.json"));
    f.repin_output(0);
    reject_output(&f, "FILE003");
    let mut f = Outputs::new(false);
    let name = "generated/Unowned.ets";
    let bytes = b"real but unowned output";
    let mut r = file(name, bytes);
    r["sources"] = json!(["fixture.call.v1"]);
    f.results[0]["files"].as_array_mut().unwrap().push(r);
    f.memories[0].0.insert(path(name), bytes.to_vec());
    f.repin_output(0);
    reject_output(&f, "InputInvalid");
}
#[test]
fn original_workspace_mutation_cannot_replace_bound_slot_bytes() {
    let mut f = Outputs::new(false);
    let result = f.check().unwrap();
    let before = result
        .slots()
        .iter()
        .flat_map(|s| s.files().files().map(|(r, b)| (r.clone(), b.to_vec())))
        .collect::<Vec<_>>();
    for memory in &mut f.memories {
        for bytes in memory.0.values_mut() {
            *bytes = b"mutated active workspace".to_vec();
        }
    }
    let after = result
        .slots()
        .iter()
        .flat_map(|s| s.files().files().map(|(r, b)| (r.clone(), b.to_vec())))
        .collect::<Vec<_>>();
    assert_eq!(before, after);
}
#[test]
fn repinned_reference_and_component_fields_are_checked_individually() {
    for (field, value, code) in [
        ("id", json!("other/generator"), "LOCK004"),
        ("version", json!("9.0.0"), "LOCK004"),
        ("sha256", json!(byte_digest(b"changed")), "LOCK004"),
    ] {
        let mut f = Outputs::new(false);
        f.requests[0]["component"][field] = value;
        f.repin_request(0);
        reject_output(&f, code);
    }
    for reference_key in ["lock", "profile", "contract"] {
        for (field, value, code) in [
            ("path", json!("wrong.json"), "FILE003"),
            ("sha256", json!(byte_digest(b"wrong")), "FILE004"),
            ("size", json!(0), "FILE004"),
            ("schema", json!("madb.other/1"), "FMT003"),
        ] {
            let mut f = Outputs::new(false);
            f.requests[0][reference_key][field] = value;
            f.repin_request(0);
            reject_output(&f, code);
        }
    }
}

#[test]
fn selected_profile_may_also_be_a_frozen_source_input() {
    let f = Outputs::with_profile_input(false, true);
    let result = f.check().unwrap();
    assert_eq!(result.slots().len(), 2);
    assert_eq!(
        result.slots()[0].input_digest().as_str(),
        f.bound.source_digest()
    );
}
