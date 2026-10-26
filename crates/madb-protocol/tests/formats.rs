use madb_protocol::*;
use std::path::PathBuf;
use std::sync::OnceLock;

fn registry() -> &'static SchemaRegistry {
    static REGISTRY: OnceLock<SchemaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SchemaRegistry::new().unwrap())
}
fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/contracts")
        .join(name);
    strict_json(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn shared_format_vectors_cover_every_payload() {
    let original = fixture("format-vectors.json");
    let composition = fixture("composition-format-vectors.json");
    let explain = fixture("explain-format-vectors.json");
    let mut covered = std::collections::BTreeSet::new();
    for vector in original["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .chain(composition["vectors"].as_array().unwrap())
        .chain(explain["vectors"].as_array().unwrap())
    {
        let kind = DocumentKind::ALL
            .into_iter()
            .find(|k| k.name() == vector["kind"].as_str().unwrap())
            .unwrap();
        covered.insert(kind);
        let mut value = fixture(vector["file"].as_str().unwrap());
        if let Some(patches) = vector["patch"].as_array() {
            for patch in patches {
                let path = patch["path"].as_str().unwrap();
                let (parent, key) = path.rsplit_once('/').unwrap();
                let container = value.pointer_mut(parent).unwrap();
                if patch["op"] == "remove" {
                    container.as_object_mut().unwrap().remove(key);
                } else if let Some(array) = container.as_array_mut() {
                    array[key.parse::<usize>().unwrap()] = patch["value"].clone();
                } else {
                    container
                        .as_object_mut()
                        .unwrap()
                        .insert(key.into(), patch["value"].clone());
                }
            }
        }
        let result = registry().validate(kind, value);
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
    assert_eq!(covered.len(), DocumentKind::ALL.len());
}

#[test]
fn canonical_bytes_and_domain_hash_match_independent_vectors() {
    for vector in fixture("canonical-vectors.json")["vectors"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            String::from_utf8(canonical_json(&vector["input"]).unwrap()).unwrap(),
            vector["canonical"].as_str().unwrap(),
            "{}",
            vector["id"]
        );
        assert_eq!(
            semantic_digest(DigestDomain::Contract, &vector["input"]).unwrap(),
            vector["sha256"].as_str().unwrap(),
            "{}",
            vector["id"]
        );
    }
    let value = fixture("canonical-vectors.json");
    assert_ne!(
        semantic_digest(DigestDomain::Contract, &value).unwrap(),
        semantic_digest(DigestDomain::Policy, &value).unwrap()
    );
}

#[test]
fn parser_rejects_lossy_and_ambiguous_documents() {
    for vector in fixture("parse-vectors.json")["vectors"].as_array().unwrap() {
        let error = strict_json(vector["text"].as_str().unwrap().as_bytes()).unwrap_err();
        assert_eq!(
            error.code,
            vector["code"].as_str().unwrap(),
            "{}: {error}",
            vector["id"]
        );
    }
    for raw in [
        "1.0",
        "-0",
        "1.0000000000000001",
        "9007199254740992",
        "-9007199254740992",
    ] {
        assert_eq!(strict_json(raw.as_bytes()).unwrap_err().code, "FMT006");
    }
}

#[test]
fn budgets_apply_to_parsing_and_already_decoded_values() {
    assert_eq!(
        strict_json(&vec![b' '; MAX_DOCUMENT_BYTES + 1])
            .unwrap_err()
            .code,
        "FMT002"
    );
    let raw = format!("{}0{}", "[".repeat(70), "]".repeat(70));
    assert_eq!(strict_json(raw.as_bytes()).unwrap_err().code, "FMT002");
    let mut value = Value::Null;
    for _ in 0..70 {
        value = Value::Array(vec![value]);
    }
    assert_eq!(canonical_json(&value).unwrap_err().code, "FMT002");
}

#[test]
fn toml_uses_the_same_schema_and_rejects_duplicate_keys() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/madb.example.toml");
    registry()
        .decode_project_toml(&std::fs::read_to_string(path).unwrap())
        .unwrap();
    assert!(
        registry()
            .decode_project_toml("schema='madb.project/1'\nschema='madb.project/1'")
            .is_err()
    );
    assert!(
        registry()
            .decode_project_toml("schema='madb.project/2'")
            .is_err()
    );
}

#[test]
fn profile_versions_and_domains_are_explicit() {
    assert_eq!(DocumentKind::Profile.schema_id(), "madb.profile/1");
    assert_eq!(DocumentKind::ProfileV2.schema_id(), "madb.profile/2");
    assert_eq!(DocumentKind::Composition.schema_id(), "madb.composition/1");
    let value = fixture("composition.json");
    assert_ne!(
        semantic_digest(DigestDomain::Profile, &value).unwrap(),
        semantic_digest(DigestDomain::ProfileV2, &value).unwrap()
    );
    assert_ne!(
        semantic_digest(DigestDomain::ProfileV2, &value).unwrap(),
        semantic_digest(DigestDomain::Composition, &value).unwrap()
    );
}
