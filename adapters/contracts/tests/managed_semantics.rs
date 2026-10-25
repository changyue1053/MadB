mod common;
use common::*;
use madb_contracts::{DocumentKind, capability, contract, shape_digest, structure_digest};
use madb_model::contract::{ResourceRepresentation, TypeDefinition};
use madb_rules::validate_contract;
use serde_json::json;

fn typed() -> (madb_protocol::Value, madb_model::contract::Capability) {
    let mut v = value(DocumentKind::ContractIr);
    v["required_features"]
        .as_array_mut()
        .unwrap()
        .extend([json!("core.errors@1"), json!("core.variants@1")]);
    v["types"].as_array_mut().unwrap().push(json!({"id":"Failure","definition":{"kind":"variant","tag_type":"u32","cases":[{"name":"Closed","tag":{"kind":"integer","type":"u32","decimal":"1"},"payload":null}],"unknown":"reject"}}));
    v["symbols"][0]["shape"]["throws"] = json!("Failure");
    let mut c = capability(&document(DocumentKind::Profile)).unwrap();
    c.supported_features
        .extend([id("core.errors@1"), id("core.variants@1")]);
    (v, c)
}

#[test]
fn typed_errors_are_part_of_each_affected_shape_and_structure() {
    let (v, c) = typed();
    let first = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    assert!(validate_contract(&contract(&first).unwrap(), &c).is_empty());
    let mut changed = v;
    changed["types"].as_array_mut().unwrap().last_mut().unwrap()["definition"]["cases"][0]["name"] =
        json!("Cancelled");
    let second = registry()
        .validate(DocumentKind::ContractIr, changed)
        .unwrap();
    assert_ne!(
        shape_digest(&first, "fixture.create.v1").unwrap(),
        shape_digest(&second, "fixture.create.v1").unwrap()
    );
    assert_ne!(
        structure_digest(&first).unwrap(),
        structure_digest(&second).unwrap()
    );
    assert_eq!(
        shape_digest(&first, "fixture.checksum.v1").unwrap(),
        shape_digest(&second, "fixture.checksum.v1").unwrap()
    );
}

#[test]
fn unknown_or_non_value_errors_and_missing_capabilities_are_rejected() {
    let (v, c) = typed();
    for (target, code) in [
        ("absent", "TYPE002"),
        ("engine", "TYPE009"),
        ("u64", "TYPE009"),
    ] {
        let mut bad = v.clone();
        bad["symbols"][0]["shape"]["throws"] = json!(target);
        let d = registry().validate(DocumentKind::ContractIr, bad).unwrap();
        assert_code(&validate_contract(&contract(&d).unwrap(), &c), code);
    }
    let d = registry().validate(DocumentKind::ContractIr, v).unwrap();
    let mut old = c;
    old.supported_features
        .retain(|f| f.as_str() != "core.errors@1");
    assert_code(&validate_contract(&contract(&d).unwrap(), &old), "CAP002");
}

#[test]
fn backend_owned_handles_never_acquire_registry_semantics() {
    let mut v = value(DocumentKind::ContractIr);
    let resource = v["types"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["id"] == "engine")
        .unwrap();
    resource["definition"]["representation"] = json!("backend-handle");
    let d = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    let m = contract(&d).unwrap();
    let mut c = capability(&document(DocumentKind::Profile)).unwrap();
    assert_code(&validate_contract(&m, &c), "CAP004");
    assert!(m.types.iter().any(|t| matches!(
        t.definition,
        TypeDefinition::Resource {
            representation: ResourceRepresentation::BackendHandle,
            ..
        }
    )));
    v["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("core.backend-resources@1"));
    c.supported_features.push(id("core.backend-resources@1"));
    let d = registry().validate(DocumentKind::ContractIr, v).unwrap();
    assert!(validate_contract(&contract(&d).unwrap(), &c).is_empty());
}

#[test]
fn old_documents_keep_their_published_shape_and_structure_digests() {
    let d = document(DocumentKind::ContractIr);
    assert_eq!(
        structure_digest(&d).unwrap(),
        d.value()["structure_digest"].as_str().unwrap()
    );
    for s in d.value()["symbols"].as_array().unwrap() {
        assert_eq!(
            shape_digest(&d, s["public_id"].as_str().unwrap()).unwrap(),
            s["shape_digest"].as_str().unwrap()
        );
    }
}

#[test]
fn erased_platform_errors_require_backend_ownership_and_both_capabilities() {
    let (mut v, mut c) = typed();
    v["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("core.backend-resources@1"));
    c.supported_features.push(id("core.backend-resources@1"));
    v["types"].as_array_mut().unwrap().push(json!({"id":"Swift.Error","definition":{"kind":"resource","representation":"backend-handle","owner":"swift.error"}}));
    v["symbols"][0]["shape"]["throws"] = json!("Swift.Error");
    let d = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    assert!(validate_contract(&contract(&d).unwrap(), &c).is_empty());
    for representation in ["registry-u64", "opaque-pointer"] {
        let mut bad = v.clone();
        bad["types"].as_array_mut().unwrap().last_mut().unwrap()["definition"]["representation"] =
            json!(representation);
        let d = registry().validate(DocumentKind::ContractIr, bad).unwrap();
        assert_code(&validate_contract(&contract(&d).unwrap(), &c), "TYPE009");
    }
    c.supported_features
        .retain(|f| f.as_str() != "core.backend-resources@1");
    assert_code(&validate_contract(&contract(&d).unwrap(), &c), "CAP002");
}
