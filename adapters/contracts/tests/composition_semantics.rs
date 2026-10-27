//! Refusal boundaries only. The synthetic bound fixture is intentionally not a
//! C07 declaration, runtime package, successful finite recipe or execution token.
//! Real ordinary/enhanced/disabled source runs have separate integration proof.
include!("composition.rs");

const FINITE_PROFILE: &str = "c07-c-arkts-source-semantics-ordinary-aarch64-ohos23";
const FINITE_RECIPE: &str = "madb.local.c07-c-arkts/source-semantics-ordinary@2";
const FIRST_SOURCE: &str = "sdk/arkts/facade/Boundary.ets.in";

fn finite_boundary_fixture() -> Fixture {
    let mut f = Fixture::new();
    for (key, value) in [
        ("id", json!(FINITE_PROFILE)),
        ("provider", json!({"language":"c","runtime":"native"})),
        (
            "consumer",
            json!({"language":"arkts","runtime":"harmonyos"}),
        ),
        ("route", json!("ARKTS-C-NAPI")),
        ("direction", json!("consumer-to-provider")),
        ("target", json!("aarch64-linux-ohos")),
        ("abi", json!("harmonyos-napi8-c07-snapshot")),
        ("artifact_kind", json!("har")),
        ("build_system", json!("clang-hvigor")),
        ("deployment_target", json!("API23")),
    ] {
        f.profile[key] = value;
    }
    let features = json!([
        "core.async@1",
        "core.backend-resources@1",
        "core.callbacks@1",
        "core.errors@1",
        "core.exact-numbers@1",
        "core.explicit-close@1",
        "core.extension-composition@1",
        "core.external-runtime-verification@1",
        "core.resources@1",
        "core.streams@1",
        "core.values@1"
    ]);
    f.profile["required_features"] = features.clone();
    f.profile["supported_features"] = features;
    f.composition["recipe"] = json!(FINITE_RECIPE);
    f.project["boundaries"][0]["id"] = json!("c-arkts");
    let row = &mut f.composition["responsibilities"][0];
    row["boundary"] = json!("c-arkts");
    row["subject"]["public_id"] = json!("fixture.create.v1");
    row["scope"]["anchor"]["public_id"] = json!("fixture.create.v1");
    repin_finite_boundary(&mut f);
    f
}

// Deliberately re-pin all caller-controlled identity links. A refusal must be
// from the independently fixed semantic boundary, not a stale hash shortcut.
fn repin_finite_boundary(f: &mut Fixture) {
    f.recipe.id = id(f.composition["recipe"].as_str().unwrap());
    f.recipe.selection.profile_id = id(f.profile["id"].as_str().unwrap());
    f.recipe.selection.route = id(f.profile["route"].as_str().unwrap());
    f.recipe.selection.target = id(f.profile["target"].as_str().unwrap());
    f.recipe.selection.direction = match f.profile["direction"].as_str().unwrap() {
        "consumer-to-provider" => Direction::ConsumerToProvider,
        "host-callback" => Direction::HostCallback,
        other => panic!("unsupported test direction {other}"),
    };
    f.composition["selection"]["profile_id"] = f.profile["id"].clone();
    for key in ["route", "target", "direction"] {
        f.composition["selection"][key] = f.profile[key].clone();
        f.project["boundaries"][0][key] = f.profile[key].clone();
    }
    f.profile["tools"][0]["target"] = f.profile["target"].clone();
    f.repin_composition();
}

fn bind_finite_boundary(f: &Fixture) -> Result<BoundCompositionInputs, FormatError> {
    let refs = f
        .memory
        .0
        .iter()
        .map(|(p, b)| FileReference {
            path: p.clone(),
            digest: Digest::of_bytes(b),
            size: b.len() as u64,
        })
        .collect();
    let frozen = capture_inputs(&f.memory, refs).unwrap();
    bind_composition_inputs(
        CompositionInputContext {
            project: &doc(DocumentKind::Project, f.project.clone()),
            lock: &doc(DocumentKind::Lock, f.lock.clone()),
            profile: ProvidedFile {
                path: "profiles/composed.json",
                bytes: &encode(&f.profile),
            },
            boundary_id: "c-arkts",
            inputs: &frozen,
            schema_paths: &f.schemas,
        },
        &f.recipe,
    )
}

fn synthetic_snapshot(f: &Fixture, bound: &BoundCompositionInputs) -> Document {
    let mut ir = fixture("contract-ir");
    ir["sdk_namespace"] = f.project["sdk_namespace"].clone();
    ir["boundary_id"] = json!("c-arkts");
    ir["source_digest"] = json!(bound.source_digest());
    ir["selector"]["profile_id"] = f.profile["id"].clone();
    ir["selector"]["profile_digest"] = json!(bound.profile_digest());
    for key in ["route", "target", "direction"] {
        ir["selector"][key] = f.profile[key].clone();
    }
    ir["structure_digest"] = json!(
        madb_contracts::structure_digest(&doc(DocumentKind::ContractIr, ir.clone())).unwrap()
    );
    let ir = doc(DocumentKind::ContractIr, ir);
    check_composition_snapshot_binding(bound, &ir).unwrap();
    ir
}

fn finite_error(f: &Fixture) -> FormatError {
    let bound =
        bind_finite_boundary(f).expect("generic input binding must succeed before finite refusal");
    c07_source_requirements(&bound, &synthetic_snapshot(f, &bound)).unwrap_err()
}

fn assert_error(error: FormatError, code: &str, location: &str) {
    assert_eq!(
        (error.code, error.path.as_str()),
        (code, location),
        "{error}"
    );
}

#[test]
fn c07_semantics_fixed_profile_tuple_cannot_be_widened_by_coherent_repin() {
    for (key, value) in [
        ("provider", json!({"language":"cpp","runtime":"native"})),
        (
            "consumer",
            json!({"language":"arkts","runtime":"other-runtime"}),
        ),
        ("route", json!("ARKTS-CPP-NAPI")),
        ("direction", json!("host-callback")),
        ("target", json!("x86_64-linux-ohos")),
        ("abi", json!("other-abi")),
        ("artifact_kind", json!("static-library")),
        ("build_system", json!("cargo-cmake")),
        ("deployment_target", json!("API24")),
        ("macros", json!([{"name":"test.extra","value":"1"}])),
        (
            "restrictions",
            json!([{"feature":"core.streams@1","reason":"test restriction"}]),
        ),
    ] {
        let mut f = finite_boundary_fixture();
        f.profile[key] = value;
        repin_finite_boundary(&mut f);
        assert_error(finite_error(&f), "CapabilityUnsupported", key);
    }
}

#[test]
fn c07_semantics_feature_closure_and_private_config_keep_their_own_gate() {
    let mut f = finite_boundary_fixture();
    for field in ["required_features", "supported_features"] {
        f.profile[field]
            .as_array_mut()
            .unwrap()
            .push(json!("vendor.future@1"));
    }
    repin_finite_boundary(&mut f);
    assert_error(
        bind_finite_boundary(&f)
            .err()
            .expect("unknown feature must reject"),
        "CapabilityUnsupported",
        "required_features",
    );

    let mut f = finite_boundary_fixture();
    f.profile["supported_features"]
        .as_array_mut()
        .unwrap()
        .push(json!("core.variants@1"));
    repin_finite_boundary(&mut f);
    assert_error(
        finite_error(&f),
        "CapabilityUnsupported",
        "supported_features",
    );

    let mut f = finite_boundary_fixture();
    for field in ["required_features", "supported_features"] {
        f.profile[field]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != "core.streams@1");
    }
    repin_finite_boundary(&mut f);
    assert_error(
        finite_error(&f),
        "CapabilityUnsupported",
        "required_features",
    );

    let mut f = finite_boundary_fixture();
    f.profile["backend_config"] = f.profile["sdk"].clone();
    repin_finite_boundary(&mut f);
    assert_error(
        bind_finite_boundary(&f)
            .err()
            .expect("private config must reject"),
        "CAP005",
        "backend_config",
    );
}

#[test]
fn c07_semantics_old_recipe_and_profile_do_not_acquire_version_two_guarantees() {
    for (profile, recipe, expected) in [
        (
            "c07-c-arkts-chain-ordinary-aarch64-ohos23",
            "madb.local.c07-c-arkts/chain-ordinary@1",
            "profile",
        ),
        (
            FINITE_PROFILE,
            "madb.local.c07-c-arkts/source-semantics-ordinary@1",
            "recipe",
        ),
        (
            FINITE_PROFILE,
            "madb.local.c07-c-arkts/source-semantics-enhanced@2",
            "recipe",
        ),
    ] {
        let mut f = finite_boundary_fixture();
        f.profile["id"] = json!(profile);
        f.composition["recipe"] = json!(recipe);
        repin_finite_boundary(&mut f);
        assert_error(finite_error(&f), "InputInvalid", expected);
    }
}

#[test]
fn c07_semantics_all_modes_require_actual_fixed_sources_after_generic_binding() {
    for mode in ["ordinary", "enhanced", "disabled"] {
        let mut f = finite_boundary_fixture();
        f.profile["id"] = json!(format!(
            "c07-c-arkts-source-semantics-{mode}-aarch64-ohos23"
        ));
        f.composition["recipe"] =
            json!(format!("madb.local.c07-c-arkts/source-semantics-{mode}@2"));
        repin_finite_boundary(&mut f);
        let error = finite_error(&f);
        assert_eq!(error.message, "finite semantic source is absent");
        assert_error(error, "InputInvalid", FIRST_SOURCE);
    }
}

#[test]
fn c07_semantics_raw_source_alone_or_rehashed_replacements_do_not_satisfy_pins() {
    let source = std::fs::read(format!(
        "{}/../../{FIRST_SOURCE}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut f = finite_boundary_fixture();
    f.memory.0.insert(path(FIRST_SOURCE), source.clone());
    repin_finite_boundary(&mut f);
    let error = finite_error(&f);
    assert_eq!(error.message, "finite package semantic source is absent");
    assert_error(error, "InputInvalid", FIRST_SOURCE);

    let mut changed = source;
    changed.extend_from_slice(b"\n// independently re-pinned applicant change\n");
    f.memory.0.insert(path(FIRST_SOURCE), changed.clone());
    f.memory.0.insert(
        path(&format!(
            "packages/madb.local.c07-composition/0.1.0/{FIRST_SOURCE}"
        )),
        changed,
    );
    repin_finite_boundary(&mut f);
    let error = finite_error(&f);
    assert_eq!(error.message, "unsupported finite semantic source bytes");
    assert_error(error, "InputInvalid", FIRST_SOURCE);
}
