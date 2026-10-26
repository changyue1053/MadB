use super::*;
use madb_app::{inputs::capture_inputs, ports::ReadWorkspace};

const AUTHOR: &str = "pub struct Reading { pub value: i32, pub label: String }\n#[repr(i32)] pub enum ReadingError { Overflow = 1 }\npub fn normalize(value: Reading) -> Reading { value }\npub fn offset(mut value: Reading, delta: i32) -> Result<Reading, ReadingError> { value.value = value.value.checked_add(delta).ok_or(ReadingError::Overflow)?; Ok(value) }\n";
fn fixture() -> (Value, Value, Owners) {
    let input = serde_json::to_vec(
        &json!({"source":AUTHOR,"source_path":"author.rs","namespace":"reading_demo"}),
    )
    .unwrap();
    let plan = decode(&crate::native_values::read_envelope(&input).unwrap()).unwrap();
    let types = json!([
        {"id":"madb.recipe.unit","definition":{"kind":"unit"}},
        {"id":"madb.recipe.u8","definition":{"kind":"integer","width":8,"signed":false}},
        {"id":"madb.recipe.u32","definition":{"kind":"integer","width":32,"signed":false}},
        {"id":"madb.recipe.i32","definition":{"kind":"integer","width":32,"signed":true}},
        {"id":"madb.recipe.String","definition":{"kind":"string","encoding":"utf-8","max_bytes":64}},
        {"id":"madb.recipe.identity_bytes","definition":{"kind":"buffer","element":"madb.recipe.u8","length_unit":"bytes","encoding":"none","mutable":false,"max_bytes":64}},
        {"id":"Reading","definition":{"kind":"record","layout":"logical","fields":[{"name":"value","type_id":"madb.recipe.i32"},{"name":"label","type_id":"madb.recipe.String"}]}},
        {"id":"ReadingError","definition":{"kind":"variant","tag_type":"madb.recipe.i32","unknown":"reject","cases":[{"name":"Overflow","tag":{"kind":"integer","type":"i32","decimal":"1"},"payload":null}]}}
    ]);
    let shapes = [
        (
            "$recipe.bootstrap.v1",
            json!({"kind":"sync","calling_convention":"c","link_name":"reading_demo_bootstrap_v1","parameters":[{"name":"version","type_id":"madb.recipe.u32"}],"result":"madb.recipe.identity_bytes","constants":[]}),
        ),
        (
            "normalize",
            json!({"kind":"sync","calling_convention":"c","link_name":"reading_demo_raw_normalize","parameters":[{"name":"value","type_id":"Reading"}],"result":"Reading","constants":[]}),
        ),
        (
            "offset",
            json!({"kind":"sync","calling_convention":"c","link_name":"reading_demo_raw_offset","parameters":[{"name":"value","type_id":"Reading"},{"name":"delta","type_id":"madb.recipe.i32"}],"result":"Reading","throws":"ReadingError","constants":[]}),
        ),
    ];
    let mut symbols = Vec::new();
    let mut policies = Vec::new();
    for (name, shape) in shapes {
        let (key, source_id, id) =
            source_identity("reading_demo", "fixture", "author.rs", name).unwrap();
        let location = if name == "$recipe.bootstrap.v1" {
            json!({"path":"author.rs","line":1,"column":1})
        } else {
            plan["functions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["name"] == name)
                .unwrap()["location"]
                .clone()
        };
        symbols.push(json!({"public_id":id,"shape":shape,"location":location,"precision":"artifact-derived","public_id_origin":"derived","source_key":key,"source_id":source_id}));
        policies.push(json!({"symbol_id":id,"slot":{"kind":"call"},"values":{"threading":"caller","blocking":"unbounded","delivery":"work-completed","cancellation":"none","cancellation_confirmation":"not-supported"},"provenance":[]}));
        policies.push(json!({"symbol_id":id,"slot":{"kind":"return"},"values":{"ownership":"copy"},"provenance":[]}));
        for p in shape["parameters"].as_array().unwrap() {
            policies.push(json!({"symbol_id":id,"slot":{"kind":"parameter","name":p["name"]},"values":{"ownership":"copy"},"provenance":[]}));
        }
    }
    let digest = Digest::of_bytes(AUTHOR.as_bytes());
    let owners = Owners {
        reader: (format!("{READER}@{VERSION}"), digest.clone()),
        native: (NATIVE.into(), digest.clone()),
        helper: (
            format!("include/reading_demo.h via {READER}@{VERSION}"),
            digest,
        ),
    };
    (
        json!({"sdk_namespace":"reading_demo","types":types,"symbols":symbols,"policies":policies}),
        plan,
        owners,
    )
}
#[test]
fn required_coverage_is_derived_from_values_and_errors_without_resources() {
    let (ir, plan, owners) = fixture();
    check_source_and_policy(&ir, &plan, "fixture", "author.rs").unwrap();
    let required = required_rows(&ir, &owners).unwrap();
    let provided = provided_rows(&plan, "fixture", "author.rs", &owners).unwrap();
    madb_rules::compare_unique_owners(&required, &provided).unwrap();
    assert_eq!(required.len(), 18);
    assert_eq!(
        required
            .iter()
            .filter(|(k, _)| k.2 == "domain-error")
            .count(),
        1
    );
    assert!(required.iter().all(|(k, _)| {
        !["subscription", "dispatch", "stop", "drain", "release"].contains(&k.1.as_str())
    }));
    assert!(required.iter().any(|(k, _)| {
        k.0 == source_identity(
            "reading_demo",
            "fixture",
            "author.rs",
            "$recipe.bootstrap.v1",
        )
        .unwrap()
        .2 && k.2 == "recipe-bootstrap"
    }));
}
#[test]
fn source_field_signature_error_and_origin_changes_are_rejected() {
    let (ir, plan, _) = fixture();
    for (pointer, value) in [
        ("/types/6/definition/fields/1/type_id", json!("u32")),
        ("/types/7/definition/cases/0/tag/decimal", json!("2")),
        ("/types/4/definition/max_bytes", json!(65)),
        (
            "/symbols/1/shape/link_name",
            json!("reading_demo_raw_other"),
        ),
        ("/symbols/2/shape/parameters/1/name", json!("other")),
        ("/symbols/2/shape/throws", json!("OtherError")),
        ("/symbols/1/location/line", json!(99)),
        ("/symbols/1/source_key/module", json!("elsewhere.rs")),
        ("/symbols/1/source_key/package", json!("other-project")),
        ("/symbols/1/source_key/kind", json!("method")),
        ("/symbols/1/source_key/overload", json!("another")),
        ("/symbols/1/source_id", json!("source:v1:wrong")),
        ("/symbols/1/public_id", json!("reading_demo.normalize.v1")),
        ("/symbols/0/public_id_origin", json!("explicit")),
    ] {
        let mut changed = ir.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            check_source_and_policy(&changed, &plan, "fixture", "author.rs").is_err(),
            "{pointer}"
        );
    }
    let mut changed = ir.clone();
    changed["symbols"].as_array_mut().unwrap().pop();
    assert!(check_source_and_policy(&changed, &plan, "fixture", "author.rs").is_err());
}
#[test]
fn effective_policies_cannot_claim_transfer_or_bounded_author_execution() {
    let (ir, plan, _) = fixture();
    for (pointer, value) in [
        ("/policies/0/values/blocking", json!("bounded")),
        ("/policies/1/values/ownership", json!("transfer")),
        ("/policies/2/symbol_id", json!("wrong.v1")),
        ("/policies/3/values/threading", json!("any")),
    ] {
        let mut changed = ir.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            check_source_and_policy(&changed, &plan, "fixture", "author.rs").is_err(),
            "{pointer}"
        );
    }
    let mut changed = ir.clone();
    let duplicate = changed["policies"][0].clone();
    changed["policies"].as_array_mut().unwrap().push(duplicate);
    assert!(check_source_and_policy(&changed, &plan, "fixture", "author.rs").is_err());
}
#[test]
fn missing_duplicate_extra_and_wrong_owner_are_rejected() {
    let (ir, plan, owners) = fixture();
    let required = required_rows(&ir, &owners).unwrap();
    let rows = provided_rows(&plan, "fixture", "author.rs", &owners).unwrap();
    let mut missing = rows.clone();
    missing.pop();
    let mut duplicate = rows.clone();
    duplicate.push(rows[0].clone());
    let mut wrong = rows.clone();
    wrong[0].1.0 = "unbound-provider".into();
    let mut extra = rows.clone();
    extra.push(row(
        "reading_demo.normalize.v1",
        "drain",
        "invented-resource",
        &owners.native,
    ));
    for bad in [missing, duplicate, wrong, extra] {
        assert!(madb_rules::compare_unique_owners(&required, &bad).is_err());
    }
}
struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, path: &LogicalPath, maximum: usize) -> Result<Option<Vec<u8>>> {
        let value = self.0.get(path).cloned();
        if let Some(bytes) = &value {
            assert!(bytes.len() <= maximum);
        }
        Ok(value)
    }
}
fn doc(kind: DocumentKind, value: Value) -> Document {
    crate::SchemaRegistry::new()
        .unwrap()
        .validate(kind, value)
        .unwrap()
}
fn format_fixture(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/contracts")
                .join(format!("{name}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn changed_reader_label_cannot_skip_frozen_value_recipe() {
    let data=serde_json::to_vec(&json!({"schema":"madb.rust-c-backend/1","provider":"rust","semantics":SEMANTICS,"source":"author.rs"})).unwrap();
    let reference = FileReference {
        path: LogicalPath::new("backend.json").unwrap(),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    let frozen = capture_inputs(
        &Memory(BTreeMap::from([(reference.path.clone(), data)])),
        vec![reference.clone()],
    )
    .unwrap();
    let mut lock = format_fixture("lock");
    lock["inputs"] =
        json!([{"path":"backend.json","sha256":reference.digest.as_str(),"size":reference.size}]);
    let result = values_ownership(
        &doc(DocumentKind::Project, format_fixture("project")),
        &doc(DocumentKind::Profile, format_fixture("profile")),
        &doc(DocumentKind::Lock, lock),
        &doc(
            DocumentKind::GenerateRequest,
            format_fixture("generate-request"),
        ),
        &doc(DocumentKind::ContractIr, format_fixture("contract-ir")),
        &frozen,
    );
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("recipe/reader identity differs")
    );
}

#[test]
fn unrelated_backend_file_does_not_acquire_finite_value_constraints() {
    let data = b"not a finite value backend".to_vec();
    let reference = FileReference {
        path: LogicalPath::new("backend.json").unwrap(),
        digest: Digest::of_bytes(&data),
        size: data.len() as u64,
    };
    let frozen = capture_inputs(
        &Memory(BTreeMap::from([(reference.path.clone(), data)])),
        vec![reference.clone()],
    )
    .unwrap();
    let mut lock = format_fixture("lock");
    lock["inputs"] =
        json!([{"path":"backend.json","sha256":reference.digest.as_str(),"size":reference.size}]);
    assert!(
        values_ownership(
            &doc(DocumentKind::Project, format_fixture("project")),
            &doc(DocumentKind::Profile, format_fixture("profile")),
            &doc(DocumentKind::Lock, lock),
            &doc(
                DocumentKind::GenerateRequest,
                format_fixture("generate-request")
            ),
            &doc(DocumentKind::ContractIr, format_fixture("contract-ir")),
            &frozen,
        )
        .unwrap()
        .is_none()
    );
}

// File bytes here exercise only frozen-input admission. These placeholders are
// deliberately not compiler/generator evidence or a successful source batch.
fn frozen_admission_error(omitted: Option<&str>, selected_source: &str) -> String {
    let closure = [
        "adapters/generators/rust_c/values_driver.py",
        "adapters/generators/rust_c/values_lowering.py",
        "adapters/generators/rust_c/values_codegen.py",
        "adapters/generators/rust_c/values_runtime.py",
        "adapters/contracts/src/native_values.rs",
        "adapters/contracts/src/native_values/names.rs",
        "adapters/contracts/src/native_values/witness.rs",
        "adapters/generators/rust_c/values_client.py",
        "adapters/generators/rust_c/values_ownership.py",
        "adapters/generators/rust_c/native_ast.py",
        "crates/madb-cli/src/bin/madb-native-values-read.rs",
        "Cargo.lock",
        "adapters/contracts/Cargo.toml",
    ];
    let mut contents: BTreeMap<_, _> = closure
        .into_iter()
        .filter(|path| Some(*path) != omitted)
        .map(|path| {
            (
                LogicalPath::new(path).unwrap(),
                b"frozen input fixture".to_vec(),
            )
        })
        .collect();
    contents.insert(
        LogicalPath::new("author.rs").unwrap(),
        AUTHOR.as_bytes().to_vec(),
    );
    contents.insert(
        LogicalPath::new("backend.json").unwrap(),
        serde_json::to_vec(&json!({"schema":"madb.rust-c-backend/1","provider":"rust","semantics":SEMANTICS,"source":"author.rs"})).unwrap(),
    );
    let files: Vec<_> = contents
        .iter()
        .map(|(path, bytes)| FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        })
        .collect();
    let inputs: Vec<_> = files
        .iter()
        .map(|r| json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size}))
        .collect();
    let components: Vec<_> = inputs
        .iter()
        .filter(|r| component_input(r["path"].as_str().unwrap()))
        .map(|r| {
            let mut r = r.clone();
            r["schema"] = json!("madb.source-input/1");
            r
        })
        .collect();
    let component = json!({"id":READER,"version":VERSION,"sha256":semantic("madb.native-component/1",&json!(components)).unwrap().as_str()});
    let mut project = format_fixture("project");
    project["boundaries"][0]["source"]["path"] = json!(selected_source);
    let mut lock = format_fixture("lock");
    lock["inputs"] = json!(inputs);
    lock["components"] = json!([component.clone()]);
    lock["project_digest"] =
        json!(crate::project_digest(&doc(DocumentKind::Project, project.clone())).unwrap());
    let mut request = format_fixture("generate-request");
    request["component"] = component;
    request["inputs"] = json!(
        inputs
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r["schema"] = json!("madb.source-input/1");
                r
            })
            .collect::<Vec<_>>()
    );
    request["input_digest"] = json!(crate::source_digest(&request["inputs"]).unwrap());
    let mut ir = format_fixture("contract-ir");
    ir["source_digest"] = request["input_digest"].clone();
    let frozen = capture_inputs(&Memory(contents), files).unwrap();
    values_ownership(
        &doc(DocumentKind::Project, project),
        &doc(DocumentKind::Profile, format_fixture("profile")),
        &doc(DocumentKind::Lock, lock),
        &doc(DocumentKind::GenerateRequest, request),
        &doc(DocumentKind::ContractIr, ir),
        &frozen,
    )
    .err()
    .expect("admission fixture is not a generated contract")
    .to_string()
}

#[test]
fn every_finite_reader_and_runtime_source_is_required() {
    for omitted in [
        "adapters/generators/rust_c/values_runtime.py",
        "adapters/contracts/src/native_values.rs",
        "adapters/contracts/src/native_values/names.rs",
        "adapters/contracts/src/native_values/witness.rs",
        "adapters/generators/rust_c/values_codegen.py",
    ] {
        let error = frozen_admission_error(Some(omitted), "author.rs");
        assert!(
            error.contains(&format!("value source closure is incomplete: {omitted}")),
            "{omitted}: {error}"
        );
    }
    // Complete presence advances beyond closure validation to the intentionally
    // unrelated contract. Do not let an earlier generic failure satisfy negatives.
    assert!(
        frozen_admission_error(None, "author.rs")
            .contains("logical types differ from actual author declarations")
    );
}

#[test]
fn frozen_backend_author_must_match_the_selected_project_boundary() {
    // This path is present in the same frozen inventory. Being tracked does not
    // make it interchangeable with the project's authoritative Rust module.
    let error = frozen_admission_error(None, "adapters/contracts/src/native_values.rs");
    assert!(
        error.contains("value author differs from the checked project boundary source"),
        "{error}"
    );
}
