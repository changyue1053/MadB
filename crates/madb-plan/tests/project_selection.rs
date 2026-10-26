use madb_model::{Id, LogicalPath};
use madb_plan::project::{
    FileOwnership, ProjectFile, ProjectModule, ProjectPlan, ProjectProfile, ProjectTarget,
    select_project_profile,
};

fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}

fn families(values: &[&str]) -> Vec<Id> {
    values.iter().map(|value| id(value)).collect()
}

fn module(name: &str, directory: Option<&str>, dependencies: &[&str]) -> ProjectModule {
    ProjectModule {
        id: id(name),
        directory: directory.map(|path| LogicalPath::new(path).unwrap()),
        dependencies: families(dependencies),
    }
}

fn file(owner: &str, path: &str, ownership: FileOwnership) -> ProjectFile {
    ProjectFile {
        module: id(owner),
        path: LogicalPath::new(path).unwrap(),
        ownership,
    }
}

fn target(name: &str, family: &str) -> ProjectTarget {
    ProjectTarget {
        id: id(name),
        family: id(family),
    }
}

fn mobile() -> ProjectProfile {
    ProjectProfile {
        id: id("mobile"),
        targets: vec![
            target("ios-simulator-arm64", "ios"),
            target("android-arm64", "android"),
            target("ios-device-arm64", "ios"),
        ],
        modules: vec![
            module("app", Some("app"), &["shared", "root"]),
            module("shared", Some("shared"), &["root"]),
            module("root", None, &[]),
        ],
        files: vec![
            file("app", "app/Main.kt", FileOwnership::User),
            file("root", "tool.lock", FileOwnership::Provenance),
            file("shared", "shared/Api.kt", FileOwnership::Managed),
            file("root", ".local/sdk.properties", FileOwnership::LocalState),
        ],
    }
}

fn all_profiles() -> Vec<ProjectProfile> {
    let mut ohos = mobile();
    ohos.id = id("mobile-ohos");
    ohos.targets.push(target("ohos-arm64", "ohos"));
    ohos.modules.push(module("ohos", Some("ohos"), &["shared"]));
    ohos.files
        .push(file("ohos", "ohos/Main.ets", FileOwnership::User));
    vec![mobile(), ohos]
}

#[test]
fn four_branches_select_complete_profiles_and_resolve_sorted_families() {
    let profiles = all_profiles();
    let original = profiles.clone();
    let default = id("mobile");
    let explicit = id("mobile-ohos");
    let targets = families(&["ohos", "ios", "android"]);
    for (profile, requested, expected) in [
        (None, None, "mobile"),
        (Some(&explicit), None, "mobile-ohos"),
        (None, Some(targets.as_slice()), "mobile-ohos"),
        (Some(&explicit), Some(targets.as_slice()), "mobile-ohos"),
    ] {
        let result = select_project_profile(&profiles, Some(&default), profile, requested).unwrap();
        let candidate = profiles.iter().find(|p| p.id.as_str() == expected).unwrap();
        assert_eq!(result.profile_id(), &candidate.id);
        assert_eq!(result.targets().len(), candidate.targets.len());
        assert!(
            candidate
                .targets
                .iter()
                .all(|t| result.targets().contains(t))
        );
        assert_eq!(
            result.plan(),
            &ProjectPlan::new(candidate.modules.clone(), candidate.files.clone()).unwrap()
        );
        assert_eq!(
            result.families(),
            families(if expected == "mobile" {
                &["android", "ios"]
            } else {
                &["android", "ios", "ohos"]
            })
        );
    }
    assert_eq!(profiles, original);
}

#[test]
fn explicit_profile_does_not_require_or_inherit_a_default() {
    for default in [None, Some(id("absent")), Some(id("mobile"))] {
        let result = select_project_profile(
            &all_profiles(),
            default.as_ref(),
            Some(&id("mobile-ohos")),
            None,
        )
        .unwrap();
        assert_eq!(result.families(), families(&["android", "ios", "ohos"]));
    }
}

#[test]
fn explicit_targets_reject_empty_and_duplicates_before_any_profile_lookup() {
    for profiles in [vec![], all_profiles()] {
        for profile in [None, Some(id("mobile")), Some(id("unknown"))] {
            for (targets, message, subject) in [
                (vec![], "explicit targets are empty", "targets"),
                (
                    families(&["ios", "android", "ios"]),
                    "duplicate requested platform family",
                    "ios",
                ),
            ] {
                let error =
                    select_project_profile(&profiles, None, profile.as_ref(), Some(&targets))
                        .unwrap_err();
                assert_eq!(error.message, message);
                assert_eq!(error.subject, subject);
            }
        }
    }
}

#[test]
fn no_selectors_require_a_named_existing_default_not_the_first_profile() {
    for profiles in [vec![], all_profiles()] {
        for default in [None, Some(id("absent"))] {
            assert_eq!(
                select_project_profile(&profiles, default.as_ref(), None, None)
                    .unwrap_err()
                    .message,
                "missing or unknown default profile"
            );
        }
    }
    let result =
        select_project_profile(&all_profiles(), Some(&id("mobile-ohos")), None, None).unwrap();
    assert_eq!(result.profile_id(), &id("mobile-ohos"));
}

#[test]
fn targets_alone_match_exactly_and_default_only_breaks_a_matching_tie() {
    let mut profiles = all_profiles();
    let targets = families(&["ios", "android"]);
    // A single match is independent of the default.
    let unique = select_project_profile(&profiles, None, None, Some(&targets)).unwrap();
    assert_eq!(unique.profile_id(), &id("mobile"));
    let mut enhanced = mobile();
    enhanced.id = id("mobile-enhanced");
    enhanced
        .files
        .push(file("app", "app/Observation.kt", FileOwnership::Managed));
    profiles.push(enhanced);
    for default in [None, Some(id("missing")), Some(id("mobile-ohos"))] {
        assert_eq!(
            select_project_profile(&profiles, default.as_ref(), None, Some(&targets))
                .unwrap_err()
                .message,
            "ambiguous template profile selection"
        );
    }
    for default in [id("mobile"), id("mobile-enhanced")] {
        let selected =
            select_project_profile(&profiles, Some(&default), None, Some(&targets)).unwrap();
        assert_eq!(selected.profile_id(), &default);
    }
    let explicit = select_project_profile(
        &profiles,
        Some(&id("mobile")),
        Some(&id("mobile-enhanced")),
        Some(&targets),
    )
    .unwrap();
    assert!(
        explicit
            .plan()
            .files()
            .iter()
            .any(|f| f.path.as_str() == "app/Observation.kt")
    );
}

#[test]
fn unknown_profiles_and_subset_superset_or_unknown_families_do_not_fall_back() {
    let profiles = all_profiles();
    assert_eq!(
        select_project_profile(&profiles, Some(&id("mobile")), Some(&id("unknown")), None)
            .unwrap_err()
            .message,
        "unknown template profile"
    );
    for request in [
        families(&["ios"]),
        families(&["android", "ios", "ohos", "linux"]),
        families(&["unknown-family"]),
    ] {
        assert_eq!(
            select_project_profile(&profiles, Some(&id("mobile")), None, Some(&request))
                .unwrap_err()
                .message,
            "no template profile has the requested platform families"
        );
    }
    for request in [families(&["ios", "android"]), families(&["ohos"])] {
        assert_eq!(
            select_project_profile(&profiles, None, Some(&id("mobile-ohos")), Some(&request))
                .unwrap_err()
                .message,
            "template profile and requested platform families differ"
        );
    }
}

#[test]
fn target_identities_are_preserved_without_inventing_sibling_abis() {
    let mut candidate = mobile();
    candidate.targets = vec![target("one-declared-device", "ios")];
    let selected = select_project_profile(&[candidate], None, Some(&id("mobile")), None).unwrap();
    assert_eq!(selected.targets(), &[target("one-declared-device", "ios")]);
    assert_eq!(selected.families(), families(&["ios"]));
    // No module or file is removed by inferring meaning from its name.
    assert_eq!(selected.plan().modules().len(), 3);
    assert_eq!(selected.plan().files().len(), 4);
}

#[test]
fn profile_and_target_identities_are_unique_but_families_may_repeat() {
    let error =
        select_project_profile(&[mobile(), mobile()], None, Some(&id("mobile")), None).unwrap_err();
    assert_eq!(error.message, "duplicate template profile identity");
    for family in ["android", "ios"] {
        let mut candidate = mobile();
        candidate.targets.push(target("android-arm64", family));
        assert_eq!(
            select_project_profile(&[candidate], None, Some(&id("mobile")), None)
                .unwrap_err()
                .message,
            "duplicate target identity in template profile"
        );
    }
    let mut empty = mobile();
    empty.targets.clear();
    assert_eq!(
        select_project_profile(&[empty], None, Some(&id("mobile")), None)
            .unwrap_err()
            .message,
        "template profile has no targets"
    );
    // Repeated ios family retains both explicit device and simulator targets.
    let result = select_project_profile(&[mobile()], None, Some(&id("mobile")), None).unwrap();
    assert_eq!(result.targets().len(), 3);
    assert_eq!(result.families().len(), 2);
}

#[test]
fn selection_and_plan_are_deterministic_under_declared_set_reordering() {
    let original = all_profiles();
    let mut reordered = original.clone();
    reordered.reverse();
    for profile in &mut reordered {
        profile.targets.reverse();
        profile.modules.reverse();
        for module in &mut profile.modules {
            module.dependencies.reverse();
        }
        profile.files.reverse();
    }
    let first = select_project_profile(
        &original,
        None,
        None,
        Some(&families(&["ohos", "ios", "android"])),
    )
    .unwrap();
    let second = select_project_profile(
        &reordered,
        None,
        None,
        Some(&families(&["android", "ohos", "ios"])),
    )
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn selected_plan_errors_are_propagated_without_repair_or_reclassification() {
    let mut invalid = Vec::new();
    let mut missing = mobile();
    missing.modules.retain(|m| m.id != id("shared"));
    invalid.push(missing);
    let mut cyclic = mobile();
    cyclic.modules[2].dependencies.push(id("app"));
    invalid.push(cyclic);
    let mut outside = mobile();
    outside.files[0].path = LogicalPath::new("outside/Main.kt").unwrap();
    invalid.push(outside);
    let mut alias = mobile();
    alias
        .files
        .push(file("root", "App/other.kt", FileOwnership::User));
    invalid.push(alias);
    let mut duplicate = mobile();
    duplicate.files.push(duplicate.files[0].clone());
    invalid.push(duplicate);
    let mut empty = mobile();
    empty.modules.clear();
    invalid.push(empty);
    for candidate in invalid {
        let expected =
            ProjectPlan::new(candidate.modules.clone(), candidate.files.clone()).unwrap_err();
        let error =
            select_project_profile(&[candidate], None, Some(&id("mobile")), None).unwrap_err();
        assert_eq!(error, expected);
    }
}

#[test]
fn unselected_plans_are_not_completed_or_validated() {
    let mut profiles = all_profiles();
    profiles[1].modules.clear();
    let duplicate = profiles[1].files[0].clone();
    profiles[1].files.push(duplicate);
    let selected = select_project_profile(&profiles, None, Some(&id("mobile")), None).unwrap();
    assert_eq!(selected.profile_id(), &id("mobile"));
    assert_eq!(
        selected.plan(),
        &ProjectPlan::new(mobile().modules, mobile().files).unwrap()
    );
}

#[test]
fn nested_modules_and_root_owned_files_keep_existing_ownership_rules() {
    let mut candidate = mobile();
    candidate
        .modules
        .push(module("nested", Some("app/nested"), &["app"]));
    candidate.files.push(file(
        "root",
        "app/nested/root.txt",
        FileOwnership::Provenance,
    ));
    candidate
        .files
        .push(file("nested", "app/nested/child.kt", FileOwnership::User));
    let expected = ProjectPlan::new(candidate.modules.clone(), candidate.files.clone()).unwrap();
    let selected = select_project_profile(&[candidate], None, Some(&id("mobile")), None).unwrap();
    assert_eq!(selected.plan(), &expected);
}
