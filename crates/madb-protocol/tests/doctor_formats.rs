use madb_protocol::{DocumentKind, SchemaRegistry, Value, strict_json};
use std::path::Path;
use std::sync::OnceLock;

fn registry() -> &'static SchemaRegistry {
    static REGISTRY: OnceLock<SchemaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SchemaRegistry::new().unwrap())
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/contracts")
        .join(name);
    strict_json(&std::fs::read(path).unwrap()).unwrap()
}

fn object_paths(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            output.push(path.to_owned());
            for (key, item) in fields {
                let key = key.replace('~', "~0").replace('/', "~1");
                object_paths(item, &format!("{path}/{key}"), output);
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
fn doctor_objects_are_closed_and_require_explicit_nullable_fields() {
    for name in [
        "doctor-result.json",
        "doctor-result-complete-model.json",
        "doctor-result-errors-model.json",
        "doctor-result-truncated-model.json",
    ] {
        let original = fixture(name);
        registry()
            .validate(DocumentKind::DoctorResult, original.clone())
            .unwrap();
        let mut paths = Vec::new();
        object_paths(&original, "", &mut paths);
        for path in paths {
            let fields = original.pointer(&path).unwrap().as_object().unwrap();
            let mut extra = original.clone();
            extra
                .pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unapproved_field".into(), Value::Null);
            assert!(
                registry()
                    .validate(DocumentKind::DoctorResult, extra)
                    .is_err()
            );
            for key in fields.keys() {
                let mut missing = original.clone();
                missing
                    .pointer_mut(&path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    registry()
                        .validate(DocumentKind::DoctorResult, missing)
                        .is_err(),
                    "{name}: optional {path}/{key}"
                );
            }
        }
    }
}

#[test]
fn doctor_reuses_the_existing_tool_observation_shape() {
    let schemas = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
    let doctor =
        strict_json(&std::fs::read(schemas.join("doctor-result.schema.json")).unwrap()).unwrap();
    let report = strict_json(&std::fs::read(schemas.join("report.schema.json")).unwrap()).unwrap();
    assert_eq!(
        doctor["$defs"]["tool_observation"],
        report["properties"]["environment"]["items"]
    );
}

#[test]
fn doctor_result_never_changes_strict_json_rejections() {
    for (raw, code) in [
        (
            r#"{"schema":"madb.doctor-result/1","schema":"madb.doctor-result/1"}"#,
            "FMT005",
        ),
        (r#"{"exit_code":1.0}"#, "FMT006"),
        (r#"{"size":1.5}"#, "FMT006"),
        (r#"{"exit_code":-0}"#, "FMT006"),
    ] {
        assert_eq!(
            registry()
                .decode(DocumentKind::DoctorResult, raw.as_bytes())
                .unwrap_err()
                .code,
            code
        );
    }
}
