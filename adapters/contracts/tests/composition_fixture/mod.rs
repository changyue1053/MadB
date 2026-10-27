//! Profile/2 build adapter vectors with frozen in-memory files. These do not
//! claim an actual compiler run, publication, or SDK acceptance.
use madb_app::{
    inputs::capture_inputs,
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    ContractError, Document, DocumentKind, SchemaRegistry,
    artifacts::ProvidedFile,
    composition::{
        ComponentRequirement, CompositionInputContext, CompositionSchemaPaths,
        DependencyRequirement, DependencyScope, GeneratedSlot, RecipeRequirements, RecipeSelection,
        composition_digest, profile_v2_digest,
    },
    native, project_digest, source_digest,
};
use madb_model::{Digest, Id, LogicalPath, contract::Direction};
use madb_protocol::{Value, byte_digest};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
pub(crate) fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
pub(crate) fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
pub(crate) fn file(name: &str, data: &[u8]) -> Value {
    json!({"path":name,"size":data.len(),"sha256":byte_digest(data)})
}
pub(crate) fn reference(name: &str, schema: &str, data: &[u8]) -> Value {
    let mut v = file(name, data);
    v["schema"] = json!(schema);
    v
}
pub(crate) fn document(kind: DocumentKind, v: Value) -> Document {
    SchemaRegistry::new().unwrap().validate(kind, v).unwrap()
}
pub(crate) fn fixture(name: &str) -> Value {
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
pub(crate) struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, p: &LogicalPath, _: usize) -> madb_app::Result<Option<Vec<u8>>> {
        Ok(self.0.get(p).cloned())
    }
}
pub(crate) struct Fixture {
    pub(crate) project: Value,
    pub(crate) profile: Value,
    pub(crate) lock: Value,
    pub(crate) seed: Value,
    pub(crate) memory: Memory,
    pub(crate) schemas: CompositionSchemaPaths,
    pub(crate) recipe: RecipeRequirements,
}
impl Fixture {
    pub(crate) fn new() -> Self {
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
            dependencies: vec![DependencyRequirement {
                from: id("facade"),
                to: id("runtime"),
                scope: DependencyScope::TargetRuntime,
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
    pub(crate) fn repin_profile(&mut self) {
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
    pub(crate) fn repin_project(&mut self) {
        self.lock["project_digest"] =
            json!(project_digest(&document(DocumentKind::Project, self.project.clone())).unwrap());
    }
    pub(crate) fn finish(&self) -> Result<Document, ContractError> {
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

use madb_app::{
    dag::ExecutionGraph,
    process::ProcessRequest,
    task::{
        BuildRecipe, TaskFileReferences, TaskInvocation, ValidatedTaskResult, capture_task_result,
    },
};
use madb_contracts::{
    composition::{build::bind_build_graph, *},
    task::TaskBinding,
    task_digest,
};

impl Fixture {
    pub(crate) fn bound(&self) -> BoundCompositionInputs {
        let references = self
            .memory
            .0
            .iter()
            .map(|(p, b)| FileReference {
                path: p.clone(),
                digest: Digest::of_bytes(b),
                size: b.len() as u64,
            })
            .collect();
        let frozen = capture_inputs(&self.memory, references).unwrap();
        bind_composition_inputs(
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
        )
        .unwrap()
    }
    pub(crate) fn required(&self) -> Vec<ResolvedResponsibility> {
        let prefix = "packages/test.metadata/1.0.0/";
        let package = self
            .memory
            .0
            .iter()
            .filter_map(|(p, b)| {
                p.as_str()
                    .strip_prefix(prefix)
                    .map(|relative| file(relative, b))
            })
            .collect::<Vec<_>>();
        let subject =
            ResolvedSubject::PublicSymbol(madb_model::PublicId::new("fixture.create.v1").unwrap());
        vec![ResolvedResponsibility {
            key: ResponsibilityKey {
                boundary: id(self.project["boundaries"][0]["id"].as_str().unwrap()),
                subject: subject.clone(),
                stage: id("projection"),
                scope: ResolvedScope {
                    kind: id("call"),
                    anchor: subject,
                    layer: id("facade"),
                },
            },
            owner: ResolvedOwner::SdkAsset(AssetOrigin::Generated {
                package: Digest::new(source_digest(&json!(package)).unwrap()).unwrap(),
                producer: id("test.metadata/generator"),
                slot: id("facade"),
                paths: vec![path("generated/Facade.h")],
            }),
        }]
    }
    pub(crate) fn image(&self) -> FileReference {
        FileReference {
            path: path("bin/compiler"),
            digest: Digest::new(self.profile["tools"][0]["sha256"].as_str().unwrap()).unwrap(),
            size: 42,
        }
    }
    pub(crate) fn tasks(&self, contract: &Document, phases: &[&str]) -> Vec<Vec<u8>> {
        phases.iter().enumerate().map(|(index,phase)|{
            let mut task=fixture("build-task");task["id"]=json!(format!("task-{index}"));task["selector"]=contract.value()["selector"].clone();
            task["input_digest"]=contract.value()["source_digest"].clone();task["working_directory"]=json!("work");task["phase"]=json!(phase);
            task["dependencies"]=if index==0{json!([])}else{json!([format!("task-{}",index-1)])};task["inputs"]=json!([file("native/fixture.rs", b"fn fixture() {}")]);task["outputs"]=json!([format!("out/{phase}")]);
            task["tool"]=json!({"id":self.profile["tools"][0]["id"],"version":self.profile["tools"][0]["version"],"sha256":self.profile["tools"][0]["sha256"]});
            seal(&mut task);bytes(&task)
        }).collect()
    }
    pub(crate) fn graph(
        &self,
        contract: &Document,
        tasks: &[Vec<u8>],
        target: Option<&Id>,
    ) -> madb_app::Result<ExecutionGraph<madb_contracts::task::BoundBuildTask>> {
        bind_build_graph(
            &self.bound(),
            &document(DocumentKind::Project, self.project.clone()),
            contract,
            &self.required(),
            tasks
                .iter()
                .map(|task| TaskBinding {
                    task,
                    installed_tool: self.image(),
                })
                .collect(),
            target,
            &[],
        )
    }
}
pub(crate) fn seal(task: &mut Value) {
    task["task_digest"] =
        json!(task_digest(&document(DocumentKind::BuildTask, task.clone())).unwrap());
}
pub(crate) fn code<T>(value: madb_app::Result<T>, expected: &str) {
    let error = match value {
        Ok(_) => panic!("invalid composition build accepted"),
        Err(e) => e,
    };
    assert!(error.findings.iter().any(|f| f.code == expected), "{error}");
}
struct Recipe {
    invocation: TaskInvocation,
    files: TaskFileReferences,
}
impl BuildRecipe for Recipe {
    fn invocation(&self) -> &TaskInvocation {
        &self.invocation
    }
    fn validate_result(&self, _: &[u8]) -> madb_app::Result<TaskFileReferences> {
        Ok(self.files.clone())
    }
}
pub(crate) fn results(f: &Fixture, tasks: &[Vec<u8>]) -> BTreeMap<Id, ValidatedTaskResult> {
    tasks
        .iter()
        .map(|data| {
            let task: Value = serde_json::from_slice(data).unwrap();
            let name = task["id"].as_str().unwrap();
            let output = task["outputs"][0].as_str().unwrap();
            let log = format!("logs/{name}.txt");
            let refs = TaskFileReferences {
                outputs: vec![FileReference {
                    path: path(output),
                    digest: Digest::of_bytes(b"compiled bytes"),
                    size: 14,
                }],
                logs: vec![FileReference {
                    path: path(&log),
                    digest: Digest::of_bytes(b"compiler log"),
                    size: 12,
                }],
            };
            let recipe = Recipe {
                invocation: TaskInvocation {
                    id: id(name),
                    process: ProcessRequest {
                        tool: f.image(),
                        working_directory: path("work"),
                        arguments: vec![],
                        environment: BTreeMap::new(),
                        stdin: data.clone(),
                    },
                    inputs: vec![FileReference {
                        path: path("native/fixture.rs"),
                        digest: Digest::of_bytes(b"fn fixture() {}"),
                        size: 15,
                    }],
                    outputs: vec![path(output)],
                },
                files: refs,
            };
            let mut evidence = fixture("build-evidence");
            evidence["task_id"] = task["id"].clone();
            for field in [
                "task_digest",
                "input_digest",
                "generation_id",
                "tool",
                "phase",
            ] {
                evidence[field] = task[field].clone();
            }
            evidence["profile_digest"] = task["selector"]["profile_digest"].clone();
            evidence["target"] = task["selector"]["target"].clone();
            evidence["outputs"] = json!([file(output, b"compiled bytes")]);
            evidence["logs"] = json!([file(&log, b"compiler log")]);
            let memory = Memory(BTreeMap::from([
                (path(output), b"compiled bytes".to_vec()),
                (path(&log), b"compiler log".to_vec()),
            ]));
            (
                id(name),
                capture_task_result(&memory, &recipe, &bytes(&evidence)).unwrap(),
            )
        })
        .collect()
}
