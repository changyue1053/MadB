mod common;
use madb_app::{
    ErrorKind, Id, QueryError,
    use_cases::{
        doctor::*,
        initialize::{Diagnostic, VerifyLevel},
    },
};
use madb_contracts::doctor::DoctorAdapter;
use serde_json::Value;
fn model() -> Report {
    let bytes = std::fs::read(
        common::root().join("tests/fixtures/contracts/doctor-result-complete-model.json"),
    )
    .unwrap();
    DoctorAdapter::new().unwrap().decode_result(&bytes).unwrap()
}
#[test]
fn complete_report_roundtrips_but_is_never_execution_authority() {
    let adapter = DoctorAdapter::new().unwrap();
    let mut report = model();
    report.finish().unwrap();
    let (result, bytes) = adapter.encode_result(&report).unwrap();
    assert_eq!(result, adapter.decode_result(&bytes).unwrap());
    assert!(bytes.ends_with(b"\n"));
    assert_eq!(
        report
            .checks
            .iter()
            .map(|v| v.id.to_string())
            .collect::<Vec<_>>(),
        [
            "context.declared-inputs",
            "context.snapshot",
            "context.configuration",
            "tool.madb",
            "recipe",
            "host"
        ]
    );
}
#[test]
fn independent_set_identity_and_exact_exit_are_not_caller_claims() {
    let original = model();
    for mutation in 0..8 {
        let mut value = original.clone();
        match mutation {
            0 => {
                value.checks.pop();
            }
            1 => value.exit_code = 4,
            2 => {
                value
                    .context
                    .as_mut()
                    .unwrap()
                    .recipe
                    .as_mut()
                    .unwrap()
                    .version = "9.0.0".into()
            }
            3 => value.request.as_mut().unwrap().verify = VerifyLevel::Run,
            4 => value.checks[0].kind = CheckKind::Host,
            5 => {
                value
                    .context
                    .as_mut()
                    .unwrap()
                    .snapshot
                    .as_mut()
                    .unwrap()
                    .file
                    .path = madb_app::LogicalPath::new("other.json").unwrap()
            }
            6 => value.checks[0].observed = Some(Fact::State(State::Missing)),
            _ => value.checks.push(value.checks[0].clone()),
        }
        assert!(value.validate().is_err(), "mutation {mutation}");
    }
}
#[test]
fn blocked_dependencies_require_prior_real_failure_and_are_sorted() {
    let error = QueryError::new(ErrorKind::InvalidInput, "META001", "snapshot", "missing");
    let mut report = Report::default();
    report.record(Check::blocked(
        "recipe",
        CheckKind::Recipe,
        "context.configuration",
    ));
    report.record(Check::blocked(
        "context.configuration",
        CheckKind::Configuration,
        "context.snapshot",
    ));
    report.record(Check::failed(
        "context.snapshot",
        CheckKind::Input,
        "snapshot",
        &error,
    ));
    assert!(report.validate().is_err());
    report.finish().unwrap();
    assert_eq!(
        report
            .checks
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>(),
        ["context.snapshot", "context.configuration", "recipe"]
    );
    report.checks[0].blocked_by = vec![Id::new("recipe").unwrap()];
    assert!(report.finish().is_err());
}
#[test]
fn report_budget_is_applied_before_protocol_limit_and_retains_conflict() {
    for conflict in [false, true] {
        let adapter = DoctorAdapter::new().unwrap();
        let mut report = model();
        report.request.as_mut().unwrap().verify = VerifyLevel::Build;
        report.checks.clear();
        for name in required_checks(VerifyLevel::Build) {
            let error = QueryError::new(
                if conflict && name == "sdk" {
                    ErrorKind::Conflict
                } else {
                    ErrorKind::InvalidInput
                },
                "FILE004",
                &name,
                "bad bytes",
            );
            let mut row = Check::failed(&name, required_kind(&name).unwrap(), &name, &error);
            row.diagnostics = vec![
                Diagnostic {
                    code: Id::new("FILE004").unwrap(),
                    message: "字".repeat(4096),
                    location: None
                };
                16
            ];
            report.record(row);
        }
        report.finish().unwrap();
        let (short, bytes) = adapter.encode_result(&report).unwrap();
        assert!(short.truncated);
        assert_eq!(short.exit_code, if conflict { 4 } else { 3 });
        assert!(bytes.len() < RESULT_LIMIT);
        assert_eq!(short.checks.len(), 1);
        assert_eq!(
            short.checks[0].error_kind,
            Some(if conflict {
                ErrorKind::Conflict
            } else {
                ErrorKind::InvalidInput
            })
        );
        assert_eq!(adapter.decode_result(&bytes).unwrap(), short);
    }
}
#[test]
fn changed_bad_bytes_or_missing_path_repair_is_conflict() {
    let e = QueryError::new(ErrorKind::Host, "HOST001", "tool", "missing");
    let first = Check::failed("tool.rustc", CheckKind::Tool, "rustc", &e);
    let mut last = first.clone();
    last.observed = Some(Fact::bytes(b"changed"));
    let mut report = Report::default();
    reconcile(&mut report, vec![first], vec![last]).unwrap();
    assert_eq!(report.exit_code, 4);
    assert_eq!(report.checks[0].error_kind, Some(ErrorKind::Conflict));
}
#[test]
fn machine_early_error_is_bounded_and_preserves_error_kind() {
    for kind in [
        ErrorKind::CheckFailed,
        ErrorKind::InvalidInput,
        ErrorKind::Host,
        ErrorKind::Conflict,
    ] {
        let error = QueryError::new(kind, "HOST001", "x\0\n", format!("{}\n", "界".repeat(9000)));
        let (report, bytes) = madb_contracts::doctor::early_error(&error);
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["exit_code"], error_exit(kind));
        assert!(bytes.len() < RESULT_LIMIT);
        assert_eq!(
            DoctorAdapter::new().unwrap().decode_result(&bytes).unwrap(),
            report
        );
    }
}

#[test]
fn observation_inventory_changes_are_conflicts() {
    let first = Check::failed(
        "sdk",
        CheckKind::Sdk,
        "sdk",
        &QueryError::new(ErrorKind::Host, "HOST001", "sdk", "missing"),
    );
    for after in [
        vec![],
        vec![Check::failed(
            "host",
            CheckKind::Host,
            "host",
            &QueryError::new(ErrorKind::Host, "HOST001", "host", "missing"),
        )],
    ] {
        assert_eq!(
            reconcile(&mut Report::default(), vec![first.clone()], after)
                .unwrap_err()
                .kind,
            ErrorKind::Conflict
        );
    }
}

#[test]
fn late_revalidation_failure_keeps_completed_input_phase_order() {
    let mut report = model();
    report.record(Check::failed(
        "context.configuration",
        CheckKind::Configuration,
        "captured inputs",
        &QueryError::new(
            ErrorKind::Conflict,
            "QUERY002",
            "source",
            "changed after validation",
        ),
    ));
    report.finish().unwrap();
    let index = |id| {
        report
            .checks
            .iter()
            .position(|row| row.id.as_str() == id)
            .unwrap()
    };
    assert!(index("context.declared-inputs") < index("context.snapshot"));
    assert!(index("context.snapshot") < index("context.configuration"));
    assert_eq!(report.exit_code, 4);
}
