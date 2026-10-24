mod common;
use common::*;
use madb_contracts::*;
use madb_protocol::Value;

#[test]
fn independent_fixture_digests_match_every_projection() {
    let ir = document(DocumentKind::ContractIr);
    assert_eq!(
        structure_digest(&ir).unwrap(),
        ir.value()["structure_digest"]
    );
    assert_eq!(policy_digest(&ir).unwrap(), ir.value()["policy_digest"]);
    for s in ir.value()["symbols"].as_array().unwrap() {
        assert_eq!(
            shape_digest(&ir, s["public_id"].as_str().unwrap()).unwrap(),
            s["shape_digest"]
        );
    }
    let lock = document(DocumentKind::Lock);
    assert_eq!(
        project_digest(&document(DocumentKind::Project)).unwrap(),
        lock.value()["project_digest"]
    );
    assert_eq!(
        profile_digest(&document(DocumentKind::Profile)).unwrap(),
        ir.value()["selector"]["profile_digest"]
    );
    assert_eq!(
        task_digest(&document(DocumentKind::BuildTask)).unwrap(),
        document(DocumentKind::BuildTask).value()["task_digest"]
    );
    assert_eq!(
        generation_id(&document(DocumentKind::OutputSet)).unwrap(),
        document(DocumentKind::OutputSet).value()["generation_id"]
    );
    assert_eq!(
        candidate_digest(&document(DocumentKind::Candidate)).unwrap(),
        document(DocumentKind::Candidate).value()["candidate_digest"]
    );
    assert_eq!(
        case_digest(&document(DocumentKind::Case)).unwrap(),
        document(DocumentKind::Report).value()["results"][0]["case_digest"]
    );
}
#[test]
fn positions_and_set_order_do_not_change_semantics() {
    let ir = document(DocumentKind::ContractIr);
    let moved = changed(DocumentKind::ContractIr, |v| {
        v["symbols"][0]["location"]["line"] = Value::from(999);
        v["symbols"].as_array_mut().unwrap().reverse();
        v["types"].as_array_mut().unwrap().reverse();
        v["policies"].as_array_mut().unwrap().reverse();
    });
    assert_eq!(
        structure_digest(&ir).unwrap(),
        structure_digest(&moved).unwrap()
    );
    assert_eq!(policy_digest(&ir).unwrap(), policy_digest(&moved).unwrap());
    let id = ir.value()["symbols"][0]["public_id"].as_str().unwrap();
    assert_eq!(
        shape_digest(&ir, id).unwrap(),
        shape_digest(&moved, id).unwrap()
    );
    let p = changed(DocumentKind::Project, |v| {
        v["policies"].as_array_mut().unwrap().reverse();
        v["policies"][0]["origin"]["location"]["line"] = Value::from(300);
    });
    assert_eq!(
        project_digest(&p).unwrap(),
        project_digest(&document(DocumentKind::Project)).unwrap()
    );
}
#[test]
fn parameter_order_policy_and_tools_invalidate_their_identities() {
    let original = document(DocumentKind::ContractIr);
    let reordered = changed(DocumentKind::ContractIr, |v| {
        v["symbols"][1]["shape"]["parameters"]
            .as_array_mut()
            .unwrap()
            .reverse()
    });
    assert_ne!(
        structure_digest(&original).unwrap(),
        structure_digest(&reordered).unwrap()
    );
    let changed_policy = changed(DocumentKind::ContractIr, |v| {
        v["policies"][0]["values"]["blocking"] = Value::String("unbounded".into())
    });
    assert_ne!(
        policy_digest(&original).unwrap(),
        policy_digest(&changed_policy).unwrap()
    );
    assert_eq!(
        structure_digest(&original).unwrap(),
        structure_digest(&changed_policy).unwrap()
    );
    let p = changed(DocumentKind::Profile, |v| {
        v["tools"][0]["version"] = Value::String("fixture-2".into())
    });
    assert_ne!(
        profile_digest(&p).unwrap(),
        profile_digest(&document(DocumentKind::Profile)).unwrap()
    );
    let t = changed(DocumentKind::BuildTask, |v| {
        v["arguments"].as_array_mut().unwrap().reverse()
    });
    assert_ne!(
        task_digest(&t).unwrap(),
        task_digest(&document(DocumentKind::BuildTask)).unwrap()
    );
}
#[test]
fn frozen_input_relationships_reject_stale_or_unresolved_inputs() {
    let ir = document(DocumentKind::ContractIr);
    let p = document(DocumentKind::Project);
    let profile = document(DocumentKind::Profile);
    let lock = document(DocumentKind::Lock);
    check_frozen_inputs(&ir, &p, &profile, &lock).unwrap();
    let stale = changed(DocumentKind::Lock, |v| {
        v["tools"][0]["version"] = Value::String("other".into())
    });
    assert_eq!(
        check_frozen_inputs(&ir, &p, &profile, &stale)
            .unwrap_err()
            .code,
        "LOCK004"
    );
    let stale = changed(DocumentKind::Lock, |v| {
        v["inputs"][0]["sha256"] = Value::String(digest("changed bytes").as_str().into())
    });
    assert_eq!(
        check_frozen_inputs(&ir, &p, &profile, &stale)
            .unwrap_err()
            .code,
        "LOCK005"
    );
    let unresolved = changed(DocumentKind::Project, |v| {
        v["boundaries"][0]["target"] = Value::Null
    });
    assert!(check_frozen_inputs(&ir, &unresolved, &profile, &lock).is_err());
}
#[test]
fn layout_observations_require_matching_input_and_compiler() {
    let profile = document(DocumentKind::Profile);
    let mut v = value(DocumentKind::ContractIr);
    let record=madb_protocol::strict_json(br#"{"id":"pair","definition":{"kind":"record","layout":"c","fields":[{"name":"count","type_id":"u64"}]}}"#).unwrap();
    v["types"].as_array_mut().unwrap().push(record);
    let missing = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    match check_contract(&missing, &profile) {
        Err(ContractError::Findings(items)) => assert_code(&items, "ABI005"),
        other => panic!("unexpected: {other:?}"),
    };
    let mut observation=madb_protocol::strict_json(br#"{"subject":"pair","kind":"layout","precision":"compiler-derived","input_digest":"","selector":{},"producer":{},"result":{"size":8,"alignment":8,"fields":[{"name":"count","offset":0,"size":8}]}}"#).unwrap();
    observation["input_digest"] = v["source_digest"].clone();
    observation["selector"] = v["selector"].clone();
    let tool = &profile.value()["tools"][0];
    observation["producer"] = madb_protocol::strict_json(
        format!(
            "{{\"id\":{},\"version\":{},\"sha256\":{}}}",
            tool["id"], tool["version"], tool["sha256"]
        )
        .as_bytes(),
    )
    .unwrap();
    v["observations"] = Value::Array(vec![observation]);
    let doc = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    v["structure_digest"] = Value::String(structure_digest(&doc).unwrap());
    let doc = registry()
        .validate(DocumentKind::ContractIr, v.clone())
        .unwrap();
    check_contract(&doc, &profile).unwrap();
    let mut model = contract(&doc).unwrap();
    let capability = capability(&profile).unwrap();
    model.observations[0].alignment = 0;
    assert_code(
        &madb_rules::validate_contract(&model, &capability),
        "ABI004",
    );
    model.observations[0].alignment = 8;
    model.observations[0].producer.version = "wrong-compiler".into();
    assert_code(
        &madb_rules::validate_contract(&model, &capability),
        "ABI003",
    );
    v["observations"][0]["input_digest"] = Value::String(digest("different input").as_str().into());
    let stale = registry().validate(DocumentKind::ContractIr, v).unwrap();
    match check_contract(&stale, &profile) {
        Err(ContractError::Findings(items)) => assert_code(&items, "ABI002"),
        other => panic!("unexpected: {other:?}"),
    };
}
