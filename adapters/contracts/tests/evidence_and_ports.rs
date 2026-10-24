mod common;
use common::*;
use madb_contracts::artifacts::*;
use madb_contracts::*;
use madb_model::contract::Direction;
use madb_model::evidence::*;
use madb_protocol::{Value, strict_json};
use madb_rules::*;

#[test]
fn every_port_matches_its_payload_and_identity() {
    for (q, a) in [
        (DocumentKind::ReadRequest, DocumentKind::ContractIr),
        (DocumentKind::GenerateRequest, DocumentKind::OutputSet),
        (DocumentKind::BuildTask, DocumentKind::BuildEvidence),
        (DocumentKind::PackageRequest, DocumentKind::Candidate),
        (DocumentKind::VerifyRequest, DocumentKind::Report),
    ] {
        validate_port_pair(&document(q), &document(a)).unwrap();
        assert_eq!(
            validate_port_pair(&document(q), &document(DocumentKind::Project))
                .unwrap_err()
                .code,
            "PORT001"
        );
    }
    let changed = changed(DocumentKind::BuildEvidence, |v| {
        v["phase"] = Value::String("run".into())
    });
    assert_eq!(
        validate_port_pair(&document(DocumentKind::BuildTask), &changed)
            .unwrap_err()
            .code,
        "PORT002"
    );
}
#[test]
fn mixed_generation_and_incomplete_build_cannot_pass() {
    let request = document(DocumentKind::BuildTask);
    for (field, new) in [
        (
            "generation_id",
            Value::String(digest("old generation").as_str().into()),
        ),
        ("exit_code", Value::from(1)),
        ("outputs", Value::Array(vec![])),
    ] {
        let result = changed(DocumentKind::BuildEvidence, |v| v[field] = new);
        assert!(validate_port_pair(&request, &result).is_err());
    }
    let outputs = changed(DocumentKind::OutputSet, |v| {
        v["task_digest"] = Value::String(digest("old task").as_str().into())
    });
    assert!(validate_port_pair(&document(DocumentKind::GenerateRequest), &outputs).is_err());
}
#[test]
fn file_inventory_is_verified_against_actual_bytes() {
    let file = ProvidedFile {
        path: "include/fixture.h",
        bytes: b"/* fixture header */\n",
    };
    for kind in [DocumentKind::OutputSet, DocumentKind::Candidate] {
        let manifest = document(kind);
        verify_files(&manifest, std::slice::from_ref(&file)).unwrap();
        assert_eq!(verify_files(&manifest, &[]).unwrap_err().code, "FILE003");
        assert_eq!(
            verify_files(
                &manifest,
                &[ProvidedFile {
                    path: file.path,
                    bytes: b"corrupt"
                }]
            )
            .unwrap_err()
            .code,
            "FILE004"
        );
        assert_eq!(
            verify_files(
                &manifest,
                &[
                    ProvidedFile {
                        path: file.path,
                        bytes: file.bytes
                    },
                    ProvidedFile {
                        path: "extra.txt",
                        bytes: b"x"
                    }
                ]
            )
            .unwrap_err()
            .code,
            "FILE005"
        );
        let self_ref = changed(kind, |v| {
            v["manifest_path"] = Value::String(file.path.into())
        });
        assert_eq!(
            verify_files(&self_ref, std::slice::from_ref(&file))
                .unwrap_err()
                .code,
            "FILE002"
        );
    }
}
#[test]
fn independent_case_expectations_control_report_acceptance() {
    let report = document(DocumentKind::Report);
    let identity = evidence_identity(&report).unwrap();
    let selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    let required = required_case(
        &document(DocumentKind::Case),
        &selected,
        EvidenceLevel::HostRun,
    )
    .unwrap();
    let results = report_results(&report).unwrap();
    assert!(
        validate_required_results(
            &identity,
            &identity,
            std::slice::from_ref(&required),
            &results
        )
        .is_empty()
    );
    for status in [
        ExecutionStatus::Failed,
        ExecutionStatus::Error,
        ExecutionStatus::Skipped,
        ExecutionStatus::Unsupported,
        ExecutionStatus::Blocked,
    ] {
        let mut r = results.clone();
        r[0].status = status;
        assert_code(
            &validate_required_results(&identity, &identity, std::slice::from_ref(&required), &r),
            "EVD005",
        );
    }
    for level in [
        EvidenceLevel::Static,
        EvidenceLevel::Generated,
        EvidenceLevel::Compiled,
        EvidenceLevel::Linked,
    ] {
        let mut r = results.clone();
        r[0].level = level;
        assert_code(
            &validate_required_results(&identity, &identity, std::slice::from_ref(&required), &r),
            "EVD006",
        );
    }
    let mut r = results.clone();
    r[0].cleanup_complete = false;
    assert_code(
        &validate_required_results(&identity, &identity, std::slice::from_ref(&required), &r),
        "EVD006",
    );
    let mut r = results.clone();
    r[0].resources[0].released = 0;
    assert_code(
        &validate_required_results(&identity, &identity, std::slice::from_ref(&required), &r),
        "EVD007",
    );
    let mut r = results;
    r[0].attachment_count = 0;
    assert_code(
        &validate_required_results(&identity, &identity, std::slice::from_ref(&required), &r),
        "EVD006",
    );
    assert_code(
        &validate_required_results(&identity, &identity, &[required], &[]),
        "EVD003",
    );
}
#[test]
fn crashes_do_not_satisfy_an_expected_rejection() {
    let report = document(DocumentKind::Report);
    let identity = evidence_identity(&report).unwrap();
    let mut results = report_results(&report).unwrap();
    let expected = ExpectedOutcome::Rejection {
        code: id("SEM008"),
        stage: id("policy"),
    };
    let case = RequiredCase {
        id: results[0].case_id.clone(),
        digest: results[0].case_digest.clone(),
        expected: expected.clone(),
        minimum_level: EvidenceLevel::Static,
    };
    results[0].expected = expected;
    results[0].actual = ActualOutcome::IsolatedAbort {
        exit_code: 134,
        reason: id("unrelated-crash"),
    };
    assert_code(
        &validate_required_results(&identity, &identity, &[case], &results),
        "EVD005",
    );
}
#[test]
fn all_identity_dependencies_invalidate_old_evidence() {
    let identity = evidence_identity(&document(DocumentKind::Report)).unwrap();
    assert_eq!(evidence_freshness(&identity, None), Freshness::Missing);
    assert_eq!(
        evidence_freshness(&identity, Some(&identity)),
        Freshness::Current
    );
    for field in [
        "input",
        "tools",
        "profile",
        "candidate",
        "lab_revision",
        "case_catalog",
    ] {
        let mut actual = identity.clone();
        match field {
            "input" => actual.input = digest("new"),
            "tools" => actual.tools = digest("new"),
            "profile" => actual.profile = digest("new"),
            "candidate" => actual.candidate = digest("new"),
            "lab_revision" => actual.lab_revision = "1".repeat(40),
            "case_catalog" => actual.case_catalog = digest("new"),
            _ => unreachable!(),
        }
        assert_eq!(
            evidence_freshness(&identity, Some(&actual)),
            Freshness::Stale(vec![field])
        );
    }
}
#[test]
fn real_problem_catalog_instantiates_without_claiming_execution() {
    let catalog =
        strict_json(&std::fs::read(root().join("docs/catalog/problems.json")).unwrap()).unwrap();
    let problems = catalog["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 48);
    let ids: Vec<_> = problems
        .iter()
        .map(|p| id(p["id"].as_str().unwrap()))
        .collect();
    let choices: Vec<_> = problems
        .iter()
        .map(|p| Requirement {
            problem_id: id(p["id"].as_str().unwrap()),
            applicability: Applicability::Required,
            reason: "C02 instantiation test only; no route certification".into(),
            case_ids: p["required_case_groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| id(c.as_str().unwrap()))
                .collect(),
        })
        .collect();
    let mut selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    for direction in [Direction::ConsumerToProvider, Direction::HostCallback] {
        selected.direction = direction;
        let result = instantiate_requirements(&ids, &selected, &choices).unwrap();
        assert_eq!(result.len(), 48);
        assert_eq!(
            result
                .iter()
                .map(|i| i.requirement.case_ids.len())
                .sum::<usize>(),
            96
        );
        assert!(result.iter().all(|i| i.selector == selected));
    }
    let mut missing = choices.clone();
    missing.pop();
    assert_code(
        &instantiate_requirements(&ids, &selected, &missing).unwrap_err(),
        "COV004",
    );
    let mut excluded = choices;
    excluded[0].applicability = Applicability::NotApplicable;
    excluded[0].reason.clear();
    assert_code(
        &instantiate_requirements(&ids, &selected, &excluded).unwrap_err(),
        "COV002",
    );
}
#[test]
fn cases_must_be_resolved_for_the_exact_consumer() {
    let case = document(DocumentKind::Case);
    let mut selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    selected.target = id("different-target");
    assert_eq!(
        required_case(&case, &selected, EvidenceLevel::HostRun)
            .unwrap_err()
            .code,
        "CASE001"
    );
    let template = changed(DocumentKind::Case, |v| {
        v["kind"] = Value::String("template".into());
        v["selector"] = Value::Null;
    });
    assert_eq!(
        required_case(&template, &selected, EvidenceLevel::HostRun)
            .unwrap_err()
            .code,
        "CASE001"
    );
}

#[test]
fn referenced_input_bytes_are_not_semantic_hashes() {
    let request = document(DocumentKind::ReadRequest);
    let reference = &request.value()["inputs"][0];
    verify_reference(
        reference,
        &ProvidedFile {
            path: "native/fixture.rs",
            bytes: b"pub fn fixture() {}\n",
        },
    )
    .unwrap();
    assert_eq!(
        verify_reference(
            reference,
            &ProvidedFile {
                path: "native/fixture.rs",
                bytes: b"changed source"
            }
        )
        .unwrap_err()
        .code,
        "FILE004"
    );
}

#[test]
fn assertions_cannot_smuggle_out_of_range_typed_constants() {
    let case = changed(DocumentKind::Case, |v| {
        v["assertions"][0]["expected"][0]["decimal"] = Value::String("18446744073709551616".into())
    });
    let selected = contract(&document(DocumentKind::ContractIr))
        .unwrap()
        .selector;
    assert_eq!(
        required_case(&case, &selected, EvidenceLevel::HostRun)
            .unwrap_err()
            .code,
        "NUM001"
    );
}
