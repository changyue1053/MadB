mod common;
use common::*;
use madb_contracts::{
    ContractError, Document, DocumentKind, check_contract, check_policy_resolution, native,
};
use madb_protocol::{Value, strict_json};
use serde_json::json;

const CREATE_SOURCE: &str =
    "source:v1:3ad84265b5a66ee7e9527cafe483b29b55a404d37c42a115dc6e1e005053ba8b";

// Raw declaration data only: no effective policy, provenance or computed
// metadata survives from the structural fixture. Expectations below state the
// authoritative project/profile behavior independently of the resolver output.
fn raw_seed() -> Value {
    let mut seed = value(DocumentKind::ContractIr);
    seed["policies"] = json!([]);
    seed["structure_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    seed["policy_digest"] = seed["structure_digest"].clone();
    let placeholder = seed["structure_digest"].clone();
    for symbol in seed["symbols"].as_array_mut().unwrap() {
        symbol["source_id"] = json!(format!("source:v1:{}", "0".repeat(64)));
        symbol["shape_digest"] = placeholder.clone();
    }
    seed
}

fn finish(seed: &Value, project: &Value, profile: &Value) -> Result<Document, ContractError> {
    let bytes = native::finalize_metadata(
        &serde_json::to_vec(seed).unwrap(),
        &serde_json::to_vec(project).unwrap(),
        &serde_json::to_vec(profile).unwrap(),
    )?;
    Ok(registry().decode(DocumentKind::ContractIr, &bytes).unwrap())
}

fn standard(seed: &Value) -> Result<Document, ContractError> {
    finish(
        seed,
        &value(DocumentKind::Project),
        &value(DocumentKind::Profile),
    )
}

fn reject(seed: &Value, project: &Value, profile: &Value, code: &str) {
    match finish(seed, project, profile).unwrap_err() {
        ContractError::Format(error) => assert_eq!(error.code, code, "{error}"),
        ContractError::Findings(findings) => assert_code(&findings, code),
    }
}

fn policy<'a>(ir: &'a Value, id: &str, slot: Value) -> &'a Value {
    ir["policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["symbol_id"] == id && p["slot"] == slot)
        .unwrap()
}

fn provenance<'a>(policy: &'a Value, property: &str) -> &'a Value {
    policy["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["property"] == property)
        .unwrap()
}

#[test]
fn raw_seed_is_completed_from_authoritative_inputs() {
    let seed = raw_seed();
    let ir = standard(&seed).unwrap();
    let v = ir.value();
    assert_eq!(v["symbols"][0]["source_id"], CREATE_SOURCE);
    assert_eq!(v["source_digest"], seed["source_digest"]);
    for field in ["types", "observations", "selector", "required_features"] {
        assert_eq!(v[field], seed[field]);
    }
    for (before, after) in seed["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .zip(v["symbols"].as_array().unwrap())
    {
        assert_eq!(before["shape"], after["shape"]);
        assert_ne!(before["source_id"], after["source_id"]);
        assert_ne!(before["shape_digest"], after["shape_digest"]);
    }
    assert_ne!(v["structure_digest"], seed["structure_digest"]);
    assert_ne!(v["policy_digest"], seed["policy_digest"]);
    let owned = policy(v, "fixture.create.v1", json!({"kind":"return"}));
    assert_eq!(
        owned["values"],
        json!({"ownership":"transfer", "releaser":"fixture.release_handle.v1",
        "close":"request-await-release", "release_thread":"caller"})
    );
    let origin = provenance(owned, "ownership");
    assert_eq!(
        origin["origin"],
        json!({"authority":"project-policy", "id":"policy.create.return",
        "location":{"path":"madb.toml","line":101,"column":1}})
    );
    assert!(origin["replaced_default"].is_null());
    let copied = policy(
        v,
        "fixture.await_closed.v1",
        json!({"kind":"parameter", "name":"timeout_ms"}),
    );
    assert_eq!(copied["values"], json!({"ownership":"copy"}));
    assert_eq!(
        provenance(copied, "ownership")["origin"]["authority"],
        "profile-default"
    );
    let profile = document(DocumentKind::Profile);
    check_contract(&ir, &profile).unwrap();
    check_policy_resolution(&ir, &document(DocumentKind::Project), &profile, &[]).unwrap();
    assert_eq!(standard(&seed).unwrap().value(), v);
}

#[test]
fn source_selector_uses_recomputed_source_identity() {
    let mut project = value(DocumentKind::Project);
    project["policies"][0]["selector"] = json!({"kind":"source_id", "id":CREATE_SOURCE});
    let ir = finish(&raw_seed(), &project, &value(DocumentKind::Profile)).unwrap();
    assert_eq!(
        policy(ir.value(), "fixture.create.v1", json!({"kind":"call"}))["values"]["blocking"],
        "bounded"
    );
}

#[test]
fn family_override_and_profile_default_retain_their_origins() {
    let mut project = value(DocumentKind::Project);
    project["policy_families"] = json!([{"id":"call-family", "values":{"blocking":"unbounded"},
        "origin":{"authority":"policy-family", "id":"family-default",
            "location":{"path":"madb.toml","line":9,"column":2}}}]);
    project["policies"][0]["families"] = json!(["call-family"]);
    let ir = finish(&raw_seed(), &project, &value(DocumentKind::Profile)).unwrap();
    let call = policy(ir.value(), "fixture.create.v1", json!({"kind":"call"}));
    assert_eq!(call["values"]["blocking"], "bounded");
    assert_eq!(
        provenance(call, "blocking")["replaced_default"],
        project["policy_families"][0]["origin"]
    );
    let data = policy(
        ir.value(),
        "fixture.checksum.v1",
        json!({"kind":"parameter","name":"data"}),
    );
    assert_eq!(
        provenance(data, "ownership")["origin"]["authority"],
        "project-policy"
    );
    assert_eq!(
        provenance(data, "ownership")["replaced_default"]["authority"],
        "profile-default"
    );
}

#[test]
fn source_positions_do_not_change_semantics_but_shapes_and_policies_do() {
    let seed = raw_seed();
    let original = standard(&seed).unwrap();
    let mut moved = seed.clone();
    moved["symbols"][0]["location"]["line"] = json!(800);
    moved["source_digest"] = json!(format!("sha256:{}", "a".repeat(64)));
    let moved = standard(&moved).unwrap();
    for field in ["structure_digest", "policy_digest"] {
        assert_eq!(original.value()[field], moved.value()[field]);
    }
    assert_eq!(
        original.value()["symbols"][0]["source_id"],
        moved.value()["symbols"][0]["source_id"]
    );
    assert_eq!(
        original.value()["symbols"][0]["shape_digest"],
        moved.value()["symbols"][0]["shape_digest"]
    );
    let mut changed = seed.clone();
    changed["symbols"][1]["shape"]["parameters"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let changed = standard(&changed).unwrap();
    assert_eq!(
        original.value()["symbols"][1]["source_id"],
        changed.value()["symbols"][1]["source_id"]
    );
    assert_ne!(
        original.value()["symbols"][1]["shape_digest"],
        changed.value()["symbols"][1]["shape_digest"]
    );
    assert_ne!(
        original.value()["structure_digest"],
        changed.value()["structure_digest"]
    );
    assert_eq!(
        original.value()["policy_digest"],
        changed.value()["policy_digest"]
    );
    let mut project = value(DocumentKind::Project);
    project["policies"][0]["values"]["blocking"] = json!("unbounded");
    let changed = finish(&seed, &project, &value(DocumentKind::Profile)).unwrap();
    assert_eq!(
        original.value()["structure_digest"],
        changed.value()["structure_digest"]
    );
    assert_ne!(
        original.value()["policy_digest"],
        changed.value()["policy_digest"]
    );
    project["policies"][0]["origin"]["location"]["line"] = json!(900);
    let relocated = finish(&seed, &project, &value(DocumentKind::Profile)).unwrap();
    assert_eq!(
        changed.value()["policy_digest"],
        relocated.value()["policy_digest"]
    );
    assert_ne!(changed.value()["policies"], relocated.value()["policies"]);
}

#[test]
fn resolved_policy_seed_and_forged_annotation_authority_are_rejected() {
    let project = value(DocumentKind::Project);
    let profile = value(DocumentKind::Profile);
    reject(
        &value(DocumentKind::ContractIr),
        &project,
        &profile,
        "NATIVE001",
    );
    let mut forged = project.clone();
    forged["policies"][0]["origin"]["authority"] = json!("source-annotation");
    // The project/profile schemas constrain authority before domain conversion.
    reject(&raw_seed(), &forged, &profile, "FMT004");
    let mut forged_profile = profile.clone();
    forged_profile["defaults"]["origin"]["authority"] = json!("source-annotation");
    reject(&raw_seed(), &project, &forged_profile, "FMT004");
    let mut unsafe_defaults = profile.clone();
    unsafe_defaults["defaults"]["values"]["ownership"] = json!("transfer");
    reject(&raw_seed(), &project, &unsafe_defaults, "POL006");
}

#[test]
fn invalid_type_bad_releaser_and_unsupported_feature_keep_rule_diagnostics() {
    let project = value(DocumentKind::Project);
    let profile = value(DocumentKind::Profile);
    let mut invalid_type = raw_seed();
    invalid_type["types"][4]["definition"]["element"] = json!("engine");
    reject(&invalid_type, &project, &profile, "TYPE007");
    let mut bad_release = raw_seed();
    bad_release["symbols"][4]["shape"]["parameters"][0]["type_id"] = json!("u64");
    reject(&bad_release, &project, &profile, "SEM007");
    let mut unsupported = raw_seed();
    unsupported["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("vendor.magic@9"));
    reject(&unsupported, &project, &profile, "CAP002");
}

#[test]
fn absent_policy_and_foreign_project_are_not_silently_completed() {
    let mut project = value(DocumentKind::Project);
    project["policies"].as_array_mut().unwrap().remove(0);
    reject(
        &raw_seed(),
        &project,
        &value(DocumentKind::Profile),
        "SEM001",
    );
    let mut project = value(DocumentKind::Project);
    project["sdk_namespace"] = json!("foreign");
    reject(
        &raw_seed(),
        &project,
        &value(DocumentKind::Profile),
        "ID007",
    );
}

#[test]
fn envelope_is_exact_and_uses_the_existing_strict_decoder() {
    let mut envelope = json!({"contract":raw_seed(), "project":value(DocumentKind::Project),
        "profile":value(DocumentKind::Profile)});
    let expected = standard(&raw_seed()).unwrap();
    let output = native::finalize_envelope(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert_eq!(strict_json(&output).unwrap(), *expected.value());
    envelope["annotation"] = json!({});
    assert!(
        matches!(native::finalize_envelope(&serde_json::to_vec(&envelope).unwrap()),
        Err(ContractError::Format(error)) if error.code == "NATIVE001")
    );
    for (bytes, code) in [
        (
            br#"{"contract":null,"contract":null,"project":null,"profile":null}"#.as_slice(),
            "FMT005",
        ),
        (
            br#"{"contract":null,"project":null}"#.as_slice(),
            "NATIVE001",
        ),
        (b"[]".as_slice(), "NATIVE001"),
        (b"{} {}".as_slice(), "FMT001"),
    ] {
        assert!(matches!(native::finalize_envelope(bytes),
            Err(ContractError::Format(error)) if error.code == code));
    }
    assert!(
        matches!(native::finalize_envelope(&vec![b' '; native::MAX_ENVELOPE_BYTES + 1]),
        Err(ContractError::Format(error)) if error.code == "FMT002")
    );
}

#[test]
fn schema_errors_and_incomplete_source_keys_are_not_repaired() {
    let mut seed = raw_seed();
    seed["unknown"] = json!(false);
    reject(
        &seed,
        &value(DocumentKind::Project),
        &value(DocumentKind::Profile),
        "FMT004",
    );
    let mut seed = raw_seed();
    seed["symbols"][0]["source_key"]["module"] = json!("bad\u{0}key");
    reject(
        &seed,
        &value(DocumentKind::Project),
        &value(DocumentKind::Profile),
        "DTO002",
    );
}
