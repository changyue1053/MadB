use super::*;
use madb_app::{
    Id,
    use_cases::initialize::{ComponentIdentity, InitRequest, VerifyLevel, freeze_request},
};
use madb_contracts::project_init::ProjectInitAdapter;
use madb_plan::project::ParameterValue;
use std::collections::BTreeMap;
#[test]
fn explicit_project_template_plans_dynamic_provenance_without_changing_default() {
    let tool = ComponentIdentity {
        id: Id::new("org.madb/cli").unwrap(),
        version: "0.1.0".into(),
        sha256: Digest::of_bytes(b"unit fixture image"),
    };
    let default = super::super::init::catalog::installed(&tool, None).unwrap();
    assert_eq!(default.version.to_string(), "0.1.1");
    let legacy =
        super::super::init::catalog::installed(&tool, Some("org.madb.template/rust-c-sdk@0.1.0"))
            .unwrap();
    assert_eq!(
        legacy.archive.sha256.as_str(),
        "sha256:97d0e9fd5747bd5ceea920574e75c8e17825e379037c94b68e878649a310a850"
    );
    let record =
        super::super::init::catalog::installed(&tool, Some("org.madb.template/rust-c-sdk@0.2.0"))
            .unwrap();
    let format = ProjectInitAdapter::new().unwrap();
    let (template, package) = super::super::init::prepare::load(&record, &format).unwrap();
    let request = InitRequest {
        template_selector: Some("org.madb.template/rust-c-sdk@0.2.0".into()),
        template_profile_selector: None,
        parameter_values: BTreeMap::from([(
            Id::new("name").unwrap(),
            ParameterValue::String("sample".into()),
        )]),
        targets: None,
        verify: VerifyLevel::Structure,
        open: None,
        dry_run: true,
        offline: true,
        output: AbsolutePath::new("/example/project").unwrap(),
    };
    let (frozen, _) = freeze_request(&template, request, "0.1.0", &format).unwrap();
    let project = super::super::init::prepare::project_with_tool(
        &record,
        &package,
        &frozen,
        &format,
        ToolIdentity {
            id: tool.id,
            version: tool.version,
            sha256: tool.sha256,
            size: 18,
            source_revision: "a".repeat(40),
        },
    )
    .unwrap();
    let descriptor = project
        .files()
        .iter()
        .find(|f| f.path().as_str() == DESCRIPTOR)
        .unwrap()
        .bytes()
        .unwrap();
    let actual = ProjectBuildAdapter::new()
        .unwrap()
        .decode_descriptor(descriptor)
        .unwrap();
    assert_eq!(actual.template.archive.sha256, record.archive.sha256);
    assert!(project.files().iter().any(|f| f.path().as_str() == ORIGIN));
    assert!(project.files().iter().any(|f| f.path().as_str() == "madbw"));
}

#[test]
fn failed_result_recovery_is_fixed_bounded_and_does_not_invent_commands() {
    let mut report = ProjectReport::empty(Operation::Build);
    report.record_failure(
        StageId::Capture,
        invalid("source", "actual capture failure").into(),
    );
    report.root = Some(AbsolutePath::new("/example/project").unwrap());
    report.artifact_root =
        Some(AbsolutePath::new("/example/project/.madb/build/attempts/failed").unwrap());
    recovery(&mut report);
    recovery(&mut report);
    assert_eq!(report.recovery.len(), 1);
    assert_eq!(report.recovery[0].kind, "retry");
    assert!(report.recovery[0].command.is_none());
    ProjectBuildAdapter::new()
        .unwrap()
        .encode_report(&report)
        .unwrap();
}
