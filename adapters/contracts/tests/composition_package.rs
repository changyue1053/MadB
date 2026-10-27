//! Package adapter vectors over actual frozen in-memory Source/Build bytes.
//! These checks do not claim helper execution, candidate publication or acceptance.
mod composition_fixture;
use composition_fixture::*;
use madb_app::{
    inputs::{FrozenInputs, capture_inputs},
    pipeline::ArtifactRecipe,
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    Document, DocumentKind,
    artifacts::ProvidedFile,
    candidate_digest,
    composition::{
        build::assemble_build_files,
        package::{BoundCompositionPackage, CompositionPackageRequest, PackageFileMapping},
    },
    execution::{ExecutionRecipe, ExecutionTool},
    generation_id,
};
use madb_model::{Digest, LogicalPath};
use serde_json::{Value, json};
use std::collections::BTreeMap;

struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, p: &LogicalPath, _: usize) -> madb_app::Result<Option<Vec<u8>>> {
        Ok(self.0.get(p).cloned())
    }
}
fn freeze(values: BTreeMap<LogicalPath, Vec<u8>>) -> FrozenInputs {
    let refs = values
        .iter()
        .map(|(p, b)| FileReference {
            path: p.clone(),
            digest: Digest::of_bytes(b),
            size: b.len() as u64,
        })
        .collect();
    capture_inputs(&Memory(values), refs).unwrap()
}
fn inventory(files: &FrozenInputs) -> BTreeMap<LogicalPath, Vec<u8>> {
    files
        .files()
        .map(|(f, b)| (f.path.clone(), b.to_vec()))
        .collect()
}
fn file_ref(name: &str, data: &[u8]) -> FileReference {
    FileReference {
        path: path(name),
        digest: Digest::of_bytes(data),
        size: data.len() as u64,
    }
}
fn wire(f: &FileReference) -> Value {
    json!({"path":f.path.as_str(),"sha256":f.digest.as_str(),"size":f.size})
}
struct PackageFixture {
    base: Fixture,
    contract: Document,
    source: FrozenInputs,
    build: FrozenInputs,
    provided: FrozenInputs,
    request: Value,
    recipe: ExecutionRecipe,
    mappings: Vec<PackageFileMapping>,
}
impl PackageFixture {
    fn new() -> Self {
        let base = Fixture::new();
        let contract = base.finish().unwrap();
        let mut outputs = fixture("output-set");
        outputs["input_digest"] = contract.value()["source_digest"].clone();
        outputs["profile_digest"] = contract.value()["selector"]["profile_digest"].clone();
        let symbols = contract.value()["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["public_id"].clone())
            .collect::<Vec<_>>();
        let mut header = file("generated/Facade.h", b"fixture generated header");
        header["sources"] = json!(symbols);
        outputs["files"] = json!([header]);
        outputs["generation_id"] =
            json!(generation_id(&document(DocumentKind::OutputSet, outputs.clone())).unwrap());
        let source = freeze(BTreeMap::from([
            (path("outputs.json"), bytes(&outputs)),
            (
                path("generated/Facade.h"),
                b"fixture generated header".to_vec(),
            ),
        ]));
        let mut tasks = base.tasks(&contract, &["compile", "link", "run"]);
        for task in &mut tasks {
            let mut value: Value = serde_json::from_slice(task).unwrap();
            value["generation_id"] = outputs["generation_id"].clone();
            seal(&mut value);
            *task = bytes(&value);
        }
        base.graph(&contract, &tasks, None).unwrap();
        let artifacts = assemble_build_files(
            &base.bound(),
            &document(DocumentKind::Project, base.project.clone()),
            &contract,
            &base.required(),
            &tasks,
            &results(&base, &tasks),
        )
        .unwrap();
        let build = artifacts.files;
        let lock_bytes = bytes(&base.lock);
        let mut input_files = inventory(&source);
        input_files.extend(inventory(&build));
        input_files.insert(path("madb.lock"), lock_bytes.clone());
        let provided = freeze(input_files);
        let mut request = fixture("package-request");
        request["selector"] = contract.value()["selector"].clone();
        request["contract_digest"] = contract.value()["structure_digest"].clone();
        request["policy_digest"] = contract.value()["policy_digest"].clone();
        request["generation_id"] = outputs["generation_id"].clone();
        request["public_symbols"] = json!(symbols);
        request["output_set"] = reference("outputs.json", "madb.output-set/1", &bytes(&outputs));
        request["lock"] = reference("madb.lock", "madb.lock/1", &lock_bytes);
        request["build_evidence"] = json!(
            artifacts
                .evidence
                .iter()
                .map(|f| {
                    let mut v = wire(f);
                    v["schema"] = json!("madb.build-evidence/1");
                    v
                })
                .collect::<Vec<_>>()
        );
        let inputs = provided.references().cloned().collect::<Vec<_>>();
        let schemas = inputs
            .iter()
            .map(|f| {
                (
                    f.path.clone(),
                    id(if f.path.as_str() == "madb.lock" {
                        "madb.lock/1"
                    } else if f.path.as_str() == "outputs.json" {
                        "madb.output-set/1"
                    } else if f.path.as_str().starts_with("evidence/") {
                        "madb.build-evidence/1"
                    } else {
                        "madb.bytes/1"
                    }),
                )
            })
            .collect();
        let recipe = ExecutionRecipe {
            id: id("package-composition"),
            request: file_ref("package.json", &bytes(&request)),
            tool: ExecutionTool {
                id: id(base.profile["tools"][0]["id"].as_str().unwrap()),
                version: base.profile["tools"][0]["version"].as_str().unwrap().into(),
                digest: base.image().digest.clone(),
            },
            installed_tool: base.image(),
            arguments: vec![],
            environment: BTreeMap::new(),
            working_directory: path("work"),
            response_path: path("candidate.json"),
            inputs,
            input_schemas: schemas,
        };
        let mut mappings = build
            .references()
            .map(|f| PackageFileMapping {
                source: f.clone(),
                destination: f.path.clone(),
            })
            .collect::<Vec<_>>();
        mappings.push(PackageFileMapping {
            source: source
                .references()
                .find(|f| f.path.as_str() == "generated/Facade.h")
                .unwrap()
                .clone(),
            destination: path("include/Facade.h"),
        });
        Self {
            base,
            contract,
            source,
            build,
            provided,
            request,
            recipe,
            mappings,
        }
    }
    fn repin_request(&mut self) {
        self.recipe.request = file_ref("package.json", &bytes(&self.request));
    }
    fn bind(&self) -> madb_app::Result<BoundCompositionPackage> {
        BoundCompositionPackage::new(
            &self.base.bound(),
            CompositionPackageRequest {
                project: &document(DocumentKind::Project, self.base.project.clone()),
                contract: &self.contract,
                required_responsibilities: &self.base.required(),
                recipe: &self.recipe,
                request: ProvidedFile {
                    path: "package.json",
                    bytes: &bytes(&self.request),
                },
                lock: ProvidedFile {
                    path: "madb.lock",
                    bytes: &bytes(&self.base.lock),
                },
                source: &self.source,
                build: &self.build,
                provided: &self.provided,
                files: &self.mappings,
            },
        )
    }
    fn candidate(&self) -> Value {
        let mut c = fixture("candidate");
        for key in [
            "selector",
            "contract_digest",
            "policy_digest",
            "generation_id",
            "public_symbols",
            "required_cases",
            "dependencies",
            "build_evidence",
        ] {
            c[key] = self.request[key].clone();
        }
        c["lock_digest"] = self.request["lock"]["sha256"].clone();
        c["manifest_path"] = json!(self.recipe.response_path.as_str());
        c["files"]=json!(self.mappings.iter().map(|m|json!({"path":m.destination.as_str(),"sha256":m.source.digest.as_str(),"size":m.source.size})).collect::<Vec<_>>());
        seal_candidate(&mut c);
        c
    }
}
fn seal_candidate(value: &mut Value) {
    value["candidate_digest"] =
        json!(candidate_digest(&document(DocumentKind::Candidate, value.clone())).unwrap());
}

#[test]
fn package_accepts_fixed_complete_source_and_build_bytes() {
    let f = PackageFixture::new();
    let bound = f.bind().unwrap();
    let output = bound.validate_result(&bytes(&f.candidate())).unwrap();
    assert_eq!(output.files.len(), 7);
    assert_eq!(
        output.profile_digest.as_str(),
        f.contract.value()["selector"]["profile_digest"]
            .as_str()
            .unwrap()
    );
}
#[test]
fn package_rejects_wrong_selector_source_parent_inventory_and_tool() {
    let mut f = PackageFixture::new();
    f.request["selector"]["profile_digest"] = json!(Digest::of_bytes(b"wrong selector").as_str());
    f.repin_request();
    code(f.bind(), "EXECUTION002");
    let mut f = PackageFixture::new();
    let mut source = inventory(&f.source);
    source.insert(path("undeclared"), b"extra".to_vec());
    f.source = freeze(source);
    code(f.bind(), "EXECUTION002");
    let mut f = PackageFixture::new();
    let mut build = inventory(&f.build);
    build.remove(&path("evidence/task-0001.json"));
    f.build = freeze(build);
    code(f.bind(), "EXECUTION002");
    let mut f = PackageFixture::new();
    f.recipe.tool.version = "9.9.9".into();
    code(f.bind(), "LOCK004");
    let mut f = PackageFixture::new();
    f.recipe.installed_tool.digest = Digest::of_bytes(b"different tool");
    code(f.bind(), "LOCK004");
}
#[test]
fn package_rejects_changed_materialized_bytes_and_parent_omission() {
    let mut f = PackageFixture::new();
    let mut provided = inventory(&f.provided);
    provided.insert(path("generated/Facade.h"), b"modified".to_vec());
    f.provided = freeze(provided);
    code(f.bind(), "FILE004");
    let mut f = PackageFixture::new();
    f.recipe.inputs.retain(|r| r.path.as_str() != "out/compile");
    code(f.bind(), "FILE003");
}
#[test]
fn candidate_cannot_drop_change_or_alias_fixed_mapped_assets() {
    let f = PackageFixture::new();
    let bound = f.bind().unwrap();
    let original = f.candidate();
    for mode in ["drop", "bytes", "alias"] {
        let mut candidate = original.clone();
        let files = candidate["files"].as_array_mut().unwrap();
        let index = files
            .iter()
            .position(|r| r["path"] == "include/Facade.h")
            .unwrap();
        match mode {
            "drop" => {
                files.remove(index);
            }
            "bytes" => files[index]["sha256"] = json!(Digest::of_bytes(b"changed asset").as_str()),
            "alias" => files[index]["path"] = json!("include/facade.h"),
            _ => unreachable!(),
        };
        seal_candidate(&mut candidate);
        code(bound.validate_result(&bytes(&candidate)), "FILE004");
    }
}
#[test]
fn mapped_paths_cannot_alias_or_collide_with_manifest_directory() {
    let mut f = PackageFixture::new();
    let source = f.mappings.last().unwrap().source.clone();
    f.mappings.push(PackageFileMapping {
        source,
        destination: path("include/facade.h"),
    });
    code(f.bind(), "INPUT001");
    let mut f = PackageFixture::new();
    f.mappings.last_mut().unwrap().destination = path("candidate.json");
    code(f.bind(), "FILE001");
    let mut f = PackageFixture::new();
    f.mappings.last_mut().unwrap().destination = path("candidate.json/header.h");
    code(f.bind(), "INPUT001");
}
