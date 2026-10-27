mod common;
use common::*;
use madb_contracts::artifacts::validate_port_pair;
use madb_contracts::*;
use madb_model::evidence::{EvidenceLevel, Freshness};
use madb_protocol::extension_author::{AuthorEvent, AuthorSession};
use madb_protocol::{ExtensionFrame, Value, byte_digest};
use madb_rules::{evidence_freshness, validate_required_results};
use serde_json::json;

#[test]
fn current_decoders_reject_unknown_versions_and_model_variants_without_downgrading() {
    for version in ["madb.contract-ir/0", "madb.contract-ir/2"] {
        let mut ir = value(DocumentKind::ContractIr);
        ir["schema"] = json!(version);
        let error = registry()
            .validate(DocumentKind::ContractIr, ir)
            .unwrap_err();
        assert_eq!((error.code, error.path.as_str()), ("FMT003", "/schema"));
    }
    let mut ir = value(DocumentKind::ContractIr);
    ir["types"][0]["definition"]["kind"] = json!("future-resource");
    assert_eq!(
        registry()
            .validate(DocumentKind::ContractIr, ir)
            .unwrap_err()
            .code,
        "FMT004"
    );
    let mut ir = value(DocumentKind::ContractIr);
    ir["model_version"] = json!(2);
    assert_eq!(
        registry()
            .validate(DocumentKind::ContractIr, ir)
            .unwrap_err()
            .code,
        "FMT004"
    );
    let newer = document(DocumentKind::ProfileV2);
    assert_eq!(
        registry()
            .validate(DocumentKind::Profile, newer.value().clone())
            .unwrap_err()
            .code,
        "FMT003"
    );
    assert_eq!(capability(&newer).unwrap_err().code, "FMT003");
    assert_eq!(profile_digest(&newer).unwrap_err().code, "FMT003");
}

#[test]
fn reader_negotiates_protocol_and_ir_independently_without_inventing_compatibility() {
    let manifest = json!({
        "schema":"madb.extension/1", "id":"test.evolution", "version":"0.1.0",
        "compatibility":{"protocol":["1.0"],"ir":["1.0"]}, "languages":[],
        "components":[{"id":"reader","port":"contract-reader",
            "supported_features":["core.values@1"],
            "entrypoint":{"kind":"process","path":"bin/reader","args":[]}}],
        "dependencies":[],"routes":[],"assets":[],
        "execution":{"transport":"stdio-jsonl","network":"none","env":[]}
    });
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let hello = json!({"schema":"madb.extension-wire/1","kind":"hello","session_id":"evolution",
        "extension_id":"test.evolution","extension_version":"0.1.0","component_id":"reader",
        "manifest_digest":byte_digest(&bytes),"protocol_versions":["1.0"],"ir_versions":["1.0"],
        "required_features":["core.values@1"]});
    for (field, offered, expected) in [
        ("protocol_versions", json!(["1.0"]), None),
        (
            "protocol_versions",
            json!(["1.1"]),
            Some("ProtocolIncompatible"),
        ),
        (
            "protocol_versions",
            json!(["2.0"]),
            Some("ProtocolIncompatible"),
        ),
        ("ir_versions", json!(["1.1"]), Some("ProtocolIncompatible")),
        ("ir_versions", json!(["2.0"]), Some("ProtocolIncompatible")),
        (
            "required_features",
            json!(["vendor.future@1"]),
            Some("CapabilityUnsupported"),
        ),
    ] {
        let mut reader = AuthorSession::from_manifest(&bytes, "reader").unwrap();
        let mut offer = hello.clone();
        offer[field] = offered;
        let AuthorEvent::Reply(reply) = reader
            .receive(ExtensionFrame::from_value(offer).unwrap())
            .unwrap()
        else {
            panic!("handshake must reply")
        };
        if let Some(code) = expected {
            assert_eq!(reply.value()["kind"], "reject");
            assert_eq!(reply.value()["error"]["code"], code);
        } else {
            assert_eq!(reply.value()["kind"], "hello_ack");
            assert_eq!(reply.value()["protocol"], "1.0");
            assert_eq!(reply.value()["ir"], "1.0");
        }
    }
}

#[test]
fn metadata_does_not_grant_unknown_semantics_or_private_configuration() {
    let base_ir = document(DocumentKind::ContractIr);
    let base_profile = document(DocumentKind::Profile);
    let metadata_ir = changed(DocumentKind::ContractIr, |v| {
        v["extensions"] = json!({"test.display":"reader annotation"});
    });
    let metadata_profile = changed(DocumentKind::Profile, |v| {
        v["extensions"] = json!({"test.display":"profile annotation"});
    });
    assert_ne!(document_bytes(&base_ir), document_bytes(&metadata_ir));
    assert_ne!(
        document_bytes(&base_profile),
        document_bytes(&metadata_profile)
    );
    assert_eq!(
        structure_digest(&base_ir).unwrap(),
        structure_digest(&metadata_ir).unwrap()
    );
    assert_eq!(
        policy_digest(&base_ir).unwrap(),
        policy_digest(&metadata_ir).unwrap()
    );
    assert_eq!(
        profile_digest(&base_profile).unwrap(),
        profile_digest(&metadata_profile).unwrap()
    );
    check_contract(&metadata_ir, &metadata_profile).unwrap();

    let unknown = changed(DocumentKind::Profile, |v| {
        for field in ["required_features", "supported_features"] {
            v[field]
                .as_array_mut()
                .unwrap()
                .push(json!("vendor.future@1"));
        }
        v["extensions"] = json!({"test.display":"supported by this helper"});
    });
    let error = capability(&unknown).unwrap_err();
    assert_eq!(
        (error.code, error.path.as_str()),
        ("CAP002", "/required_features")
    );
    let unknown_ir = changed(DocumentKind::ContractIr, |v| {
        v["required_features"]
            .as_array_mut()
            .unwrap()
            .push(json!("vendor.future@1"));
    });
    let ContractError::Findings(findings) = check_contract(&unknown_ir, &base_profile).unwrap_err()
    else {
        panic!("unknown semantic feature must reach capability rejection")
    };
    assert!(findings.iter().all(|f| f.code == "CAP002"));
    assert!(!findings.is_empty());
    let private = changed(DocumentKind::Profile, |v| {
        v["backend_config"] = v["sdk"].clone()
    });
    let error = capability(&private).unwrap_err();
    assert_eq!(
        (error.code, error.path.as_str()),
        ("CAP005", "/backend_config")
    );
}

#[test]
fn shape_and_policy_evolution_change_candidate_identity_and_reject_old_report() {
    let ir = document(DocumentKind::ContractIr);
    let reordered = changed(DocumentKind::ContractIr, |v| {
        v["symbols"][1]["shape"]["parameters"]
            .as_array_mut()
            .unwrap()
            .reverse();
    });
    let policy = changed(DocumentKind::ContractIr, |v| {
        v["policies"][0]["values"]["blocking"] = json!("unbounded");
    });
    let baseline = value(DocumentKind::Report)["identity"].clone();
    for (field, before, after) in [
        (
            "contract_digest",
            structure_digest(&ir).unwrap(),
            structure_digest(&reordered).unwrap(),
        ),
        (
            "policy_digest",
            policy_digest(&ir).unwrap(),
            policy_digest(&policy).unwrap(),
        ),
    ] {
        assert_ne!(before, after);
        let candidate = changed(DocumentKind::Candidate, |v| v[field] = json!(after));
        let mut expected = baseline.clone();
        expected["candidate_digest"] = json!(candidate_digest(&candidate).unwrap());
        assert_old_report_rejected(&baseline, &expected, &["candidate"]);
    }
}

#[test]
fn every_evidence_dependency_is_invalidated_by_its_actual_fixture_input() {
    let original_profile = document(DocumentKind::Profile);
    let new_profile = changed(DocumentKind::Profile, |v| {
        v["tools"][0]["version"] = json!("fixture-2")
    });
    let original_case = document(DocumentKind::Case);
    let new_case = changed(DocumentKind::Case, |v| {
        v["assertions"][0]["expected"][0]["decimal"] = json!("1")
    });
    let new_ir = changed(DocumentKind::ContractIr, |v| {
        v["policies"][0]["values"]["blocking"] = json!("unbounded")
    });
    let candidate = changed(DocumentKind::Candidate, |v| {
        v["policy_digest"] = json!(policy_digest(&new_ir).unwrap())
    });
    // These are byte fixtures for the identity rules, not executable tools or a
    // new run. Package source identities are never substituted for tool images.
    let tool_before = b"test-only executable-image fixture revision 1";
    let tool_after = b"test-only executable-image fixture revision 2";
    // A one-case catalog uses its actual serialized file bytes, not a new
    // unapproved semantic catalog domain or projection.
    let changes = [
        (
            "input_digest",
            "input",
            input_identity(b"int value(void);\n"),
            input_identity(b"// moved\nint value(void);\n"),
        ),
        (
            "tool_digest",
            "tools",
            byte_digest(tool_before),
            byte_digest(tool_after),
        ),
        (
            "profile_digest",
            "profile",
            profile_digest(&original_profile).unwrap(),
            profile_digest(&new_profile).unwrap(),
        ),
        (
            "candidate_digest",
            "candidate",
            candidate_digest(&document(DocumentKind::Candidate)).unwrap(),
            candidate_digest(&candidate).unwrap(),
        ),
        (
            "lab_revision",
            "lab_revision",
            "0".repeat(40),
            "1".repeat(40),
        ),
        (
            "case_catalog_digest",
            "case_catalog",
            byte_digest(&document_bytes(&original_case)),
            byte_digest(&document_bytes(&new_case)),
        ),
    ];
    let mut baseline = value(DocumentKind::Report)["identity"].clone();
    for (field, _, before, after) in &changes {
        assert_ne!(before, after, "{field}");
        baseline[field] = json!(before);
    }
    let mut all_changed = baseline.clone();
    for (field, reason, _, after) in &changes {
        let mut expected = baseline.clone();
        expected[field] = json!(after);
        assert_old_report_rejected(&baseline, &expected, &[reason]);
        all_changed[field] = json!(after);
    }
    assert_old_report_rejected(
        &baseline,
        &all_changed,
        &[
            "input",
            "tools",
            "profile",
            "candidate",
            "lab_revision",
            "case_catalog",
        ],
    );
}

#[test]
fn repinning_report_identity_cannot_hide_a_changed_independent_case_definition() {
    let selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    let changed_case = changed(DocumentKind::Case, |v| {
        v["assertions"][0]["expected"][0]["decimal"] = json!("1");
    });
    let report = changed(DocumentKind::Report, |v| {
        v["identity"]["case_catalog_digest"] = json!(byte_digest(&document_bytes(&changed_case)));
    });
    let identity = evidence_identity(&report).unwrap();
    assert_eq!(
        evidence_freshness(&identity, Some(&identity)),
        Freshness::Current
    );
    assert_ne!(
        case_digest(&changed_case).unwrap(),
        report.value()["results"][0]["case_digest"]
    );
    let required = required_case(&changed_case, &selected, EvidenceLevel::HostRun).unwrap();
    let findings = validate_required_results(
        &identity,
        &identity,
        &[required],
        &report_results(&report).unwrap(),
    );
    assert_eq!(
        findings.iter().map(|f| f.code).collect::<Vec<_>>(),
        ["EVD004"]
    );
}

fn document_bytes(document: &Document) -> Vec<u8> {
    serde_json::to_vec(document.value()).unwrap()
}

fn input_identity(bytes: &[u8]) -> String {
    source_digest(
        &json!([{"path":"include/value.h","sha256":byte_digest(bytes),"size":bytes.len()}]),
    )
    .unwrap()
}

fn assert_old_report_rejected(baseline: &Value, expected: &Value, reasons: &[&'static str]) {
    let old_report = changed(DocumentKind::Report, |v| {
        v["identity"] = baseline.clone();
        v["selector"]["profile_digest"] = baseline["profile_digest"].clone();
    });
    let original_request = changed(DocumentKind::VerifyRequest, |v| {
        v["identity"] = baseline.clone();
        v["selector"]["profile_digest"] = baseline["profile_digest"].clone();
    });
    validate_port_pair(&original_request, &old_report).unwrap();
    let new_request = changed(DocumentKind::VerifyRequest, |v| {
        v["identity"] = expected.clone();
        v["selector"]["profile_digest"] = expected["profile_digest"].clone();
    });
    assert_eq!(
        validate_port_pair(&new_request, &old_report)
            .unwrap_err()
            .code,
        "PORT002"
    );
    let actual = evidence_identity(&old_report).unwrap();
    let current = evidence_identity(&new_request).unwrap();
    assert_eq!(
        evidence_freshness(&actual, Some(&actual)),
        Freshness::Current
    );
    assert_eq!(
        evidence_freshness(&current, Some(&actual)),
        Freshness::Stale(reasons.to_vec())
    );
    let selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    let required = required_case(
        &document(DocumentKind::Case),
        &selected,
        EvidenceLevel::HostRun,
    )
    .unwrap();
    let results = report_results(&old_report).unwrap();
    assert!(
        validate_required_results(&actual, &actual, std::slice::from_ref(&required), &results)
            .is_empty()
    );
    let findings = validate_required_results(&current, &actual, &[required], &results);
    assert_eq!(
        findings.iter().map(|f| f.code).collect::<Vec<_>>(),
        ["EVD001"]
    );
}
