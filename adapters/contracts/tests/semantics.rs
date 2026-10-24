mod common;
use common::*;
use madb_contracts::*;
use madb_model::contract::*;
use madb_model::policy::*;
use madb_model::{PublicId, SourceId};
use madb_protocol::Value;
use madb_rules::*;

fn model() -> Contract {
    contract(&document(DocumentKind::ContractIr)).unwrap()
}
fn cap() -> Capability {
    capability(&document(DocumentKind::Profile)).unwrap()
}
fn policy_origin() -> Origin {
    project_policies(&document(DocumentKind::Project))
        .unwrap()
        .bindings[0]
        .origin
        .clone()
}
fn resolve(
    bindings: &[Binding],
    families: &[Family],
) -> Result<Vec<EffectivePolicy>, Vec<madb_model::Finding>> {
    let m = model();
    let mut o = policy_origin();
    o.authority = Authority::ProfileDefault;
    resolve_policies(
        &m.namespace,
        &m.symbols,
        &m.types,
        bindings,
        families,
        &[PolicyValue::Ownership(Ownership::Copy)],
        &o,
    )
}
fn binding() -> Binding {
    project_policies(&document(DocumentKind::Project))
        .unwrap()
        .bindings[0]
        .clone()
}

#[test]
fn full_fixture_resolves_from_authoritative_inputs() {
    let ir = document(DocumentKind::ContractIr);
    let p = document(DocumentKind::Profile);
    check_contract(&ir, &p).unwrap();
    check_policy_resolution(&ir, &document(DocumentKind::Project), &p, &[]).unwrap();
}
#[test]
fn source_identity_is_stable_across_positions_but_not_overloads() {
    let mut m = model();
    let first = m.symbols[0].source_key.source_id();
    m.symbols[0].location.line += 100;
    assert_eq!(first, m.symbols[0].source_key.source_id());
    m.symbols[0].source_key.overload.push_str("(u64)");
    assert_ne!(first, m.symbols[0].source_key.source_id());
    assert_code(&validate_identities(&m.namespace, &m.symbols, &[]), "ID003");
}
#[test]
fn duplicate_or_retired_identities_are_rejected() {
    let mut m = model();
    m.symbols[1].public_id = m.symbols[0].public_id.clone();
    assert!(!validate_identities(&m.namespace, &m.symbols, &[]).is_empty());
    let m = model();
    assert!(
        !validate_identities(&m.namespace, &m.symbols, &[m.symbols[0].public_id.clone()])
            .is_empty()
    );
}
#[test]
fn selectors_are_exact_and_source_selectors_are_supported() {
    let mut b = binding();
    b.selector = ExactSelector::Public(PublicId::new("absent.v1").unwrap());
    assert_code(&resolve(&[b.clone()], &[]).unwrap_err(), "POL001");
    b.selector = ExactSelector::Source(model().symbols[0].source_id.clone());
    assert!(resolve(&[b], &[]).is_ok());
    assert!(SourceId::new("open").is_err());
}
#[test]
fn equal_explicit_values_still_conflict() {
    let a = binding();
    let mut b = a.clone();
    b.origin.authority = Authority::SourceAnnotation;
    b.origin.location.line = 700;
    let errors = resolve(&[a, b], &[]).unwrap_err();
    assert_code(&errors, "POL004");
    let conflict = errors.iter().find(|f| f.code == "POL004").unwrap();
    assert_eq!(conflict.locations.len(), 2);
    assert!(conflict.locations.iter().any(|l| l.line == 700));
}
#[test]
fn family_conflicts_reject_and_explicit_override_retains_origin() {
    let mut o = policy_origin();
    o.authority = Authority::PolicyFamily;
    let a = Family {
        id: id("a"),
        values: vec![PolicyValue::Blocking(Blocking::Bounded)],
        origin: o.clone(),
    };
    let b = Family {
        id: id("b"),
        values: a.values.clone(),
        origin: o.clone(),
    };
    let mut bind = binding();
    bind.families = vec![a.id.clone(), b.id.clone()];
    assert_code(
        &resolve(&[bind.clone()], &[a.clone(), b]).unwrap_err(),
        "POL003",
    );
    bind.families = vec![a.id.clone()];
    let result = resolve(&[bind], &[a]).unwrap();
    let call = result.iter().find(|p| p.slot == Slot::Call).unwrap();
    assert_eq!(call.values["blocking"].replaced_default, Some(o));
}
#[test]
fn defaults_never_invent_threading_or_copy_resources() {
    let result = resolve(&[], &[]).unwrap();
    assert!(
        result
            .iter()
            .all(|p| p.values.len() == 1 && p.slot != Slot::Call)
    );
    assert!(
        result
            .iter()
            .all(|p| p.values["ownership"].value == PolicyValue::Ownership(Ownership::Copy))
    );
    let mut m = model();
    let p = m
        .policies
        .iter_mut()
        .find(|p| p.slot == Slot::Return)
        .unwrap();
    let v = p.values.get_mut("ownership").unwrap();
    v.value = PolicyValue::Ownership(Ownership::Copy);
    v.origin.authority = Authority::ProfileDefault;
    let findings = validate_contract(&m, &cap());
    assert_code(&findings, "POL006");
    assert_code(&findings, "SEM005");
}
#[test]
fn required_features_and_backend_config_fail_closed() {
    let mut m = model();
    m.required_features.push(id("vendor.magic@9"));
    assert_code(&validate_contract(&m, &cap()), "CAP002");
    let p = changed(DocumentKind::Profile, |v| {
        v["required_features"] = Value::Array(vec![Value::String("vendor.magic@9".into())])
    });
    assert_eq!(capability(&p).unwrap_err().code, "CAP002");
    let p = changed(DocumentKind::Profile, |v| {
        v["backend_config"] = v["sdk"].clone()
    });
    assert_eq!(capability(&p).unwrap_err().code, "CAP005");
}
#[test]
fn heuristic_reader_and_cross_target_profile_cannot_authorize_wrapping() {
    let mut m = model();
    m.symbols[0].precision = Precision::Heuristic;
    assert_code(&validate_contract(&m, &cap()), "CAP003");
    let mut c = cap();
    c.selector.target = id("x86_64-pc-windows-msvc");
    assert_code(&validate_contract(&model(), &c), "CAP001");
}
#[test]
fn async_borrow_needs_more_than_a_call_frame() {
    let mut m = model();
    m.symbols[1].kind = CallKind::Async;
    m.required_features.push(id("core.async@1"));
    let findings = validate_contract(&m, &cap());
    assert_code(&findings, "SEM008");
    assert!(
        findings
            .iter()
            .filter(|f| f.code == "SEM008")
            .all(|f| f.locations.contains(&m.symbols[1].location))
    );
}
#[test]
fn release_signature_cancellation_and_required_policies_are_checked() {
    let mut m = model();
    m.symbols[4].parameters.clear();
    assert_code(&validate_contract(&m, &cap()), "SEM007");
    let mut m = model();
    let p = m
        .policies
        .iter_mut()
        .find(|p| p.slot == Slot::Call)
        .unwrap();
    p.values.get_mut("cancellation").unwrap().value = PolicyValue::Cancellation(Cancellation::Work);
    assert_code(&validate_contract(&m, &cap()), "SEM002");
    m.policies
        .iter_mut()
        .find(|p| p.slot == Slot::Call)
        .unwrap()
        .values
        .remove("threading");
    assert_code(&validate_contract(&m, &cap()), "SEM001");
}
#[test]
fn nested_resource_cannot_evade_slot_ownership() {
    let mut m = model();
    let resource = m
        .types
        .iter()
        .find(|t| matches!(t.definition, TypeDefinition::Resource { .. }))
        .unwrap()
        .id
        .clone();
    let b = m
        .types
        .iter_mut()
        .find(|t| matches!(t.definition, TypeDefinition::Buffer { .. }))
        .unwrap();
    if let TypeDefinition::Buffer { element, .. } = &mut b.definition {
        *element = resource;
    }
    assert_code(&validate_contract(&m, &cap()), "TYPE007");
}
#[test]
fn exact_numbers_enforce_declared_width_and_spelling() {
    for (signed, width, decimal, valid) in [
        (false, 64, "18446744073709551615", true),
        (false, 64, "18446744073709551616", false),
        (true, 64, "-9223372036854775808", true),
        (true, 64, "9223372036854775808", false),
        (false, 8, "-1", false),
        (true, 8, "01", false),
        (true, 128, "-170141183460469231731687303715884105728", true),
    ] {
        assert_eq!(
            validate_constant(&ExactConstant::Integer {
                signed,
                width,
                decimal: decimal.into()
            })
            .is_ok(),
            valid,
            "{decimal}"
        );
    }
    assert!(
        validate_constant(&ExactConstant::Float {
            width: 64,
            bits: "8000000000000000".into()
        })
        .is_ok()
    );
    assert!(
        validate_constant(&ExactConstant::Float {
            width: 32,
            bits: "nan".into()
        })
        .is_err()
    );
}

#[test]
fn non_overloaded_source_identity_is_independent_of_signature() {
    let mut m = model();
    let before = m.symbols[1].source_key.source_id().unwrap();
    assert_eq!(m.symbols[1].source_key.overload, "none");
    m.symbols[1].parameters.reverse();
    assert_eq!(before, m.symbols[1].source_key.source_id().unwrap());
    m.symbols[1].source_key.qualified_name = "fixture::api::校验".into();
    let unicode = m.symbols[1].source_key.source_id().unwrap();
    assert_ne!(before, unicode);
    m.symbols[1].source_id = unicode;
    assert!(validate_identities(&m.namespace, &m.symbols, &[]).is_empty());
}
