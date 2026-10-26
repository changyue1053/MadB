use madb_protocol::{
    DigestDomain, DocumentKind, SchemaRegistry, Value, byte_digest, canonical_json,
    semantic_digest, strict_json,
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn registry() -> &'static SchemaRegistry {
    static REGISTRY: OnceLock<SchemaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SchemaRegistry::new().unwrap())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/project-init")
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(root().join(name)).unwrap()
}

fn fixture(name: &str) -> Value {
    strict_json(&bytes(name)).unwrap()
}

fn apply(value: &mut Value, patch: &Value) {
    let (parent, key) = patch["path"].as_str().unwrap().rsplit_once('/').unwrap();
    let target = value.pointer_mut(parent).unwrap();
    if patch["op"] == "remove" {
        target.as_object_mut().unwrap().remove(key);
    } else if let Some(items) = target.as_array_mut() {
        items[key.parse::<usize>().unwrap()] = patch["value"].clone();
    } else {
        target
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), patch["value"].clone());
    }
}

#[test]
fn approved_init_wire_shapes_share_schema_vectors() {
    for vector in fixture("format-vectors.json")["vectors"]
        .as_array()
        .unwrap()
    {
        let kind = match vector["kind"].as_str().unwrap() {
            "project-template" => DocumentKind::ProjectTemplate,
            "init-result" => DocumentKind::InitResult,
            other => panic!("unexpected I01 kind {other}"),
        };
        let mut value = fixture(vector["file"].as_str().unwrap());
        for patch in vector["patch"].as_array().unwrap() {
            apply(&mut value, patch);
        }
        let result = registry().decode(kind, &canonical_json(&value).unwrap());
        assert_eq!(
            result.is_ok(),
            vector["expected"] == "valid",
            "{}: {result:?}",
            vector["id"]
        );
        if let Err(error) = result {
            assert_eq!(
                error.code,
                vector["code"].as_str().unwrap(),
                "{}",
                vector["id"]
            );
        }
    }
}

#[test]
fn lexical_rejections_happen_before_init_schema() {
    for vector in fixture("format-vectors.json")["parse_vectors"]
        .as_array()
        .unwrap()
    {
        for kind in [DocumentKind::ProjectTemplate, DocumentKind::InitResult] {
            let error = registry()
                .decode(kind, &bytes(vector["file"].as_str().unwrap()))
                .unwrap_err();
            assert_eq!(error.code, vector["code"].as_str().unwrap());
        }
    }
    for raw in ["{\"x\":1e0}", "{\"x\":NaN}", "{\"x\":Infinity}"] {
        assert!(strict_json(raw.as_bytes()).is_err(), "{raw}");
    }
}

fn object_paths(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            output.push(path.to_owned());
            for (key, item) in fields {
                let token = key.replace('~', "~0").replace('/', "~1");
                object_paths(item, &format!("{path}/{token}"), output);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                object_paths(item, &format!("{path}/{index}"), output);
            }
        }
        _ => {}
    }
}

#[test]
fn every_declared_object_is_closed_and_all_fields_are_required() {
    for (kind, name) in [
        (
            DocumentKind::ProjectTemplate,
            "package-input/meta/template.json",
        ),
        (
            DocumentKind::InitResult,
            "vectors/positive-created-model.json",
        ),
    ] {
        let original = fixture(name);
        let mut paths = Vec::new();
        object_paths(&original, "", &mut paths);
        for path in paths {
            let keys: Vec<_> = original
                .pointer(&path)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            let mut extra = original.clone();
            extra
                .pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unapproved_field".into(), Value::Null);
            assert!(
                registry().validate(kind, extra).is_err(),
                "{name} allows unknown field at {path}"
            );
            for key in keys {
                let mut missing = original.clone();
                missing
                    .pointer_mut(&path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
                assert!(
                    registry().validate(kind, missing).is_err(),
                    "{name} does not require {path}/{key}"
                );
            }
        }
    }
}

#[test]
fn init_request_digest_uses_approved_domain_and_exact_projection() {
    let projection = fixture("vectors/positive-request-projection.json");
    let expected = "sha256:6c4d1f2969bbcb244fd1b201fb32ac1341756be7e4ceb04040051f383a4ec1cc";
    assert_eq!(DigestDomain::InitRequest.as_str(), "madb.init-request/1");
    assert_eq!(
        semantic_digest(DigestDomain::InitRequest, &projection).unwrap(),
        expected
    );
    assert_eq!(
        fixture("vectors/positive-dry-run.json")["request_digest"],
        expected
    );
    assert_ne!(
        semantic_digest(DigestDomain::Project, &projection).unwrap(),
        expected
    );
    let mut changed = projection;
    changed["dry_run"] = Value::Bool(false);
    assert_eq!(
        semantic_digest(DigestDomain::InitRequest, &changed).unwrap(),
        "sha256:92d3cc22676df9bd3aca970018c02a736e94541adc1b3b8b5db320ae6f06b9d1"
    );
}

#[test]
fn environment_shape_is_identical_to_existing_report_items() {
    let schemas = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
    let result =
        strict_json(&std::fs::read(schemas.join("init-result.schema.json")).unwrap()).unwrap();
    let report = strict_json(&std::fs::read(schemas.join("report.schema.json")).unwrap()).unwrap();
    assert_eq!(
        result["$defs"]["environment_item"],
        report["properties"]["environment"]["items"]
    );
}

#[test]
fn imported_design_fixtures_keep_bytes_and_model_only_identity() {
    let provenance = fixture("fixture-provenance.json");
    for entry in provenance["files"].as_array().unwrap() {
        let content = bytes(entry["path"].as_str().unwrap());
        assert_eq!(content.len() as u64, entry["size"].as_u64().unwrap());
        assert_eq!(
            byte_digest(&content),
            format!("sha256:{}", entry["sha256"].as_str().unwrap())
        );
    }
    assert_eq!(fixture("index.json")["not_execution_evidence"], true);
    assert_eq!(
        fixture("vectors/positive-created-model-facts.json")["commit_new"]["actual"],
        false
    );
}
