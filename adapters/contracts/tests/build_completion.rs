mod common;
use common::{registry, value};
use madb_contracts::{DocumentKind, build_completion_policy, profile_digest};
use madb_model::build::{BuildCompletionPolicy, EXTERNAL_RUNTIME_VERIFICATION, Phase};
use serde_json::json;

#[test]
fn only_required_feature_selects_external_runtime_and_changes_identity() {
    let original = registry()
        .validate(DocumentKind::Profile, value(DocumentKind::Profile))
        .unwrap();
    let original_digest = profile_digest(&original).unwrap();
    assert_eq!(
        build_completion_policy(&original).unwrap(),
        BuildCompletionPolicy::CompileLinkRun
    );
    let mut v = original.value().clone();
    v["supported_features"]
        .as_array_mut()
        .unwrap()
        .push(json!(EXTERNAL_RUNTIME_VERIFICATION));
    let supported = registry()
        .validate(DocumentKind::Profile, v.clone())
        .unwrap();
    assert_eq!(
        build_completion_policy(&supported).unwrap(),
        BuildCompletionPolicy::CompileLinkRun
    );
    v["required_features"]
        .as_array_mut()
        .unwrap()
        .push(json!(EXTERNAL_RUNTIME_VERIFICATION));
    let selected = registry().validate(DocumentKind::Profile, v).unwrap();
    assert_eq!(
        build_completion_policy(&selected).unwrap(),
        BuildCompletionPolicy::ExternalRuntimeVerification
    );
    assert_ne!(profile_digest(&selected).unwrap(), original_digest);
    assert_ne!(
        profile_digest(&selected).unwrap(),
        profile_digest(&supported).unwrap()
    );
    assert!(
        build_completion_policy(&supported)
            .unwrap()
            .allows(Phase::Run)
    );
}

#[test]
fn unsupported_and_unknown_required_features_fail_closed() {
    for feature in [EXTERNAL_RUNTIME_VERIFICATION, "core.unknown-build-mode@1"] {
        let mut v = value(DocumentKind::Profile);
        v["required_features"]
            .as_array_mut()
            .unwrap()
            .push(json!(feature));
        if feature != EXTERNAL_RUNTIME_VERIFICATION {
            v["supported_features"]
                .as_array_mut()
                .unwrap()
                .push(json!(feature));
        }
        let profile = registry().validate(DocumentKind::Profile, v).unwrap();
        let error = build_completion_policy(&profile).unwrap_err();
        assert!(error.to_string().contains("CAP002"), "{error}");
    }
}
