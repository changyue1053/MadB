mod project_init_common;
use madb_app::use_cases::initialize::*;
use madb_app::{ErrorKind, QueryError, Result};
use madb_model::evidence::Applicability;
use project_init_common::*;

#[test]
fn ordinary_model_requires_commit_and_exact_saved_result_bytes() {
    let (a, r, p) = prepared(false);
    let o = observations(&r, &p);
    let mut e = sink();
    assert!(complete_initialization(&r, &p, None, o.clone(), &a, &mut e).is_err());
    assert_eq!(e.calls, 0);
    let attempt = commit_new(&mut MockCommit, &r, &p).unwrap();
    let receipt = attempt.validated().unwrap();
    e.corrupt = true;
    assert!(complete_initialization(&r, &p, Some(receipt), o.clone(), &a, &mut e).is_err());
    e.corrupt = false;
    let completed = complete_initialization(&r, &p, Some(receipt), o, &a, &mut e).unwrap();
    assert_eq!(completed.report().creation, Creation::Created);
    assert_eq!(
        a.decode_result(completed.bytes()).unwrap(),
        *completed.report()
    );
}
#[test]
fn dry_run_saves_evidence_without_ever_committing_user_project() {
    let (a, r, p) = prepared(true);
    assert!(commit_new(&mut MockCommit, &r, &p).is_err());
    let mut e = sink();
    let done = complete_initialization(&r, &p, None, observations(&r, &p), &a, &mut e).unwrap();
    assert_eq!(e.calls, 1);
    assert_eq!(done.report().creation, Creation::NotCreated);
    assert!(done.report().checks.is_empty());
}
#[test]
fn empty_or_optional_observations_cannot_erase_independent_required_checks() {
    let (a, r, p) = prepared(false);
    let attempt = commit_new(&mut MockCommit, &r, &p).unwrap();
    for optional in [false, true] {
        let mut o = observations(&r, &p);
        if optional {
            o.checks[0].requirement = Applicability::Optional
        } else {
            o.checks.clear()
        };
        let mut e = sink();
        assert!(
            complete_initialization(&r, &p, Some(attempt.validated().unwrap()), o, &a, &mut e)
                .is_err()
        );
        assert_eq!(e.calls, 0);
    }
}
#[test]
fn duplicate_check_and_wrong_root_are_not_success_facts() {
    let (a, r, p) = prepared(false);
    let attempt = commit_new(&mut MockCommit, &r, &p).unwrap();
    let mut o = observations(&r, &p);
    o.checks.push(o.checks[0].clone());
    assert!(
        complete_initialization(
            &r,
            &p,
            Some(attempt.validated().unwrap()),
            o,
            &a,
            &mut sink()
        )
        .is_err()
    );
    let mut o = observations(&r, &p);
    o.evidence_root = p.root().clone();
    assert!(
        complete_initialization(
            &r,
            &p,
            Some(attempt.validated().unwrap()),
            o,
            &a,
            &mut sink()
        )
        .is_err()
    );
}
#[test]
fn host_error_with_null_digest_keeps_host_category() {
    let a = adapter();
    let report = a
        .decode_result(&read("vectors/positive-offline-cache-miss.json"))
        .unwrap();
    let failed = failure_report(
        report,
        None,
        None,
        InitializationFailure::Stage(QueryError::new(
            ErrorKind::Host,
            "FILE001",
            "cache",
            "model cache miss",
        )),
    )
    .unwrap();
    assert!(failed.request_digest.is_none());
    assert_eq!(failed.exit_code, 3);
    assert_eq!(failed.creation, Creation::NotCreated);
    a.encode_result(&failed).unwrap();
}
struct PostCommitFailure {
    mismatch: bool,
}
impl CommitNewPort for PostCommitFailure {
    fn commit_new(&mut self, p: &PreparedProject) -> Result<HostCommitObservation> {
        Ok(HostCommitObservation {
            root: p.root().clone(),
            files: if self.mismatch {
                Ok(Vec::new())
            } else {
                Err(QueryError::new(
                    ErrorKind::Host,
                    "FILE001",
                    "inventory",
                    "model post-commit read failure",
                ))
            },
        })
    }
}
#[test]
fn post_commit_mismatch_and_read_failure_preserve_created_without_success_receipt() {
    let (a, r, p) = prepared(false);
    for mismatch in [false, true] {
        let attempt = commit_new(&mut PostCommitFailure { mismatch }, &r, &p).unwrap();
        assert!(attempt.validated().is_err());
        let progress = a
            .decode_result(&read("vectors/positive-offline-cache-miss.json"))
            .unwrap();
        let failure = failure_report(
            progress,
            Some(&r),
            Some(attempt.created()),
            InitializationFailure::Stage(QueryError::new(
                ErrorKind::Host,
                "PORT001",
                "commit",
                "model verification failed after create",
            )),
        )
        .unwrap();
        assert_eq!(failure.creation, Creation::Created);
        assert_eq!(failure.exit_code, 3);
        assert_eq!(failure.request_digest.as_ref(), Some(r.digest()));
        a.encode_result(&failure).unwrap();
    }
}
#[test]
fn observed_result_is_not_a_receipt_and_cross_field_counterexamples_reject() {
    let a = adapter();
    for n in [
        "negative-success-without-created",
        "negative-missing-required-check",
        "negative-duplicate-check-key",
        "negative-wrong-package-sha",
        "negative-open-request-mismatch",
        "negative-template-root-collapsed",
    ] {
        assert!(
            a.decode_result(&read(&format!("vectors/{n}.json")))
                .is_err(),
            "{n}"
        );
    }
}
#[test]
fn dev_version_and_entry_identity_do_not_gain_formal_or_ide_support() {
    let a = adapter();
    let mut t = a
        .decode_template(&read("package-input/meta/template.json"))
        .unwrap()
        .template()
        .clone();
    let mut r = request(true);
    r.parameter_values.insert(
        id("name"),
        madb_plan::project::ParameterValue::String("model_demo".into()),
    );
    assert!(
        freeze_request(
            &madb_plan::project::CheckedTemplate::new(t.clone()).unwrap(),
            r.clone(),
            "0.1.0-dev.0",
            &a
        )
        .is_err()
    );
    t.ide_entries.push(madb_plan::project::IdeEntry {
        id: id("entry-a"),
        module: id("root"),
        ide: madb_plan::project::IdeRequirement {
            id: id("clion"),
            min_inclusive: "1.0.0".into(),
            max_exclusive: "2.0.0".into(),
        },
        path: path("CMakeLists.txt"),
        run_target: Some(id("native")),
        extensions: Vec::new(),
    });
    t.profiles[0].ide_entries.push(id("entry-a"));
    let t = madb_plan::project::CheckedTemplate::new(t).unwrap();
    r.open = Some(id("entry-a"));
    let error = freeze_request(&t, r.clone(), "0.1.0", &a).unwrap_err();
    assert!(error.to_string().contains("IDE entry is absent"));
    r.open = Some(id("clion"));
    let error = freeze_request(&t, r, "0.1.0", &a).unwrap_err();
    assert!(error.to_string().contains("no supported IDE"));
}

#[test]
fn invalid_required_table_is_rejected_before_digest_or_host_can_be_called() {
    use std::cell::Cell;
    struct CountDigest(Cell<u32>);
    impl InitRequestDigest for CountDigest {
        fn digest(&self, _: &ResolvedRequest) -> Result<madb_app::Digest> {
            self.0.set(self.0.get() + 1);
            Ok(madb_app::Digest::of_bytes(b"model"))
        }
    }
    let a = adapter();
    for remove in [false, true] {
        let mut t = a
            .decode_template(&read("package-input/meta/template.json"))
            .unwrap()
            .template()
            .clone();
        if remove {
            t.validation.retain(|v| v.id.as_str() != "structure");
            t.profiles[0]
                .validation
                .retain(|v| v.as_str() != "structure");
            for v in &mut t.validation {
                v.prerequisites.retain(|v| v.as_str() != "structure");
            }
        } else {
            t.validation
                .iter_mut()
                .find(|v| v.id.as_str() == "structure")
                .unwrap()
                .required = false;
        }
        let t = madb_plan::project::CheckedTemplate::new(t).unwrap();
        let d = CountDigest(Cell::new(0));
        let mut r = request(false);
        r.parameter_values.insert(
            id("name"),
            madb_plan::project::ParameterValue::String("model_demo".into()),
        );
        let failure = freeze_request(&t, r, "0.1.0", &d).unwrap_err();
        assert_eq!(failure.kind, ErrorKind::InvalidInput);
        assert_eq!(d.0.get(), 0);
    }
}

#[test]
fn cancelling_after_commit_keeps_created_and_the_frozen_digest() {
    let (a, r, p) = prepared(false);
    let attempt = commit_new(&mut MockCommit, &r, &p).unwrap();
    let progress = a
        .decode_result(&read("vectors/positive-offline-cache-miss.json"))
        .unwrap();
    let report = failure_report(
        progress,
        Some(&r),
        Some(attempt.created()),
        InitializationFailure::Cancelled,
    )
    .unwrap();
    assert_eq!(report.creation, Creation::Created);
    assert_eq!(report.exit_code, 1);
    assert_eq!(report.termination, Termination::Cancelled);
    assert_eq!(report.request_digest.as_ref(), Some(r.digest()));
}

#[test]
fn diagnostics_use_existing_codes_at_every_result_location() {
    let a = adapter();
    let base: serde_json::Value =
        serde_json::from_slice(&read("vectors/positive-created-model.json")).unwrap();
    for location in ["/diagnostics", "/checks/0/diagnostics", "/open/diagnostics"] {
        for code in ["PLAN001", "FILE004", "QUERY001", "UNKNOWN", "EXEC013"] {
            let mut v = base.clone();
            *v.pointer_mut(location).unwrap() =
                serde_json::json!([{"code":code,"message":"model diagnostic", "location":null}]);
            let bytes = serde_json::to_vec(&v).unwrap();
            let observed = a.decode_result(&bytes);
            if ["UNKNOWN", "EXEC013"].contains(&code) {
                assert!(observed.is_err(), "{location}:{code}");
                let mut typed = a
                    .decode_result(&serde_json::to_vec(&base).unwrap())
                    .unwrap();
                let diagnostic = Diagnostic {
                    code: id(code),
                    message: "model diagnostic".into(),
                    location: None,
                };
                match location {
                    "/diagnostics" => typed.diagnostics.push(diagnostic),
                    "/checks/0/diagnostics" => typed.checks[0].diagnostics.push(diagnostic),
                    _ => typed.open.diagnostics.push(diagnostic),
                }
                assert!(
                    a.encode_result(&typed).is_err(),
                    "encoder {location}:{code}"
                );
            } else {
                a.encode_result(&observed.unwrap()).unwrap();
            }
        }
    }
}
#[test]
fn malformed_port_diagnostic_rejects_without_panicking() {
    let a = adapter();
    let report = a
        .decode_result(&read("vectors/positive-offline-cache-miss.json"))
        .unwrap();
    assert!(
        failure_report(
            report,
            None,
            None,
            InitializationFailure::Stage(QueryError::new(
                ErrorKind::Host,
                "invalid code",
                "model",
                "model malformed port error"
            ))
        )
        .is_err()
    );
}
