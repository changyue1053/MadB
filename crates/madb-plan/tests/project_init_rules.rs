use madb_model::{Digest, Id, LogicalPath, evidence::EvidenceLevel};
use madb_plan::project::*;
use std::collections::BTreeMap;
fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn file(value: &str, size: u64) -> FileIdentity {
    FileIdentity {
        path: path(value),
        sha256: Digest::of_bytes(value.as_bytes()),
        size,
    }
}
fn version(value: &str) -> Version {
    Version::parse(value).unwrap()
}
fn range(min: &str, max: &str) -> VersionRange {
    VersionRange {
        min_inclusive: version(min),
        max_exclusive: version(max),
    }
}
fn parameter(name: &str, default: Option<&str>) -> Parameter {
    Parameter::String {
        id: id(name),
        required: true,
        default: default.map(str::to_owned),
        min_length: 1,
        max_length: 48,
        pattern: Some(StringPattern::Identifier),
        choices: vec![],
    }
}
fn check(
    name: &str,
    level: CheckLevel,
    evidence: EvidenceLevel,
    prior: &[&str],
) -> ValidationCheck {
    ValidationCheck {
        id: id(name),
        level,
        target: id("mac-arm64"),
        task_root: Some(id("native")),
        required: true,
        minimum_evidence: evidence,
        prerequisites: prior.iter().map(|p| id(p)).collect(),
    }
}
fn fixture() -> Template {
    let files = vec![file("assets/main.c", 12), file("profile.json", 42)];
    let requirements = RequirementSet {
        hosts: vec![HostRequirement {
            os: id("macos"),
            arch: id("aarch64"),
            versions: range("26.6.2", "26.6.3"),
        }],
        ..Default::default()
    };
    Template {
        id: id("org.madb.template/rust-c-sdk"),
        version: version("1.0.0"),
        madb_compatibility: range("1.0.0", "2.0.0"),
        default_profile: id("macos-arm64-values"),
        parameters: vec![
            parameter("name", None),
            parameter("namespace", Some("reading_demo")),
        ],
        profiles: vec![TemplateProfile {
            id: id("macos-arm64-values"),
            targets: vec![TemplateTarget {
                id: id("mac-arm64"),
                family: id("macos"),
                boundary_refs: vec![id("native-values")],
            }],
            modules: vec![id("root"), id("consumer")],
            boundary_refs: vec![id("native-values")],
            assets: vec![id("main"), id("local")],
            ide_entries: vec![],
            task_roots: vec![id("native")],
            validation: vec![
                id("structure"),
                id("native-generate"),
                id("c-link"),
                id("ordinary-run"),
            ],
            requirements: Requirements {
                structure: requirements.clone(),
                build: requirements.clone(),
                run: requirements.clone(),
                open: requirements,
            },
        }],
        modules: vec![
            TemplateModule {
                id: id("root"),
                role: id("project"),
                directory: None,
                dependencies: vec![],
            },
            TemplateModule {
                id: id("consumer"),
                role: id("c-consumer"),
                directory: Some(path("consumer")),
                dependencies: vec![id("root")],
            },
        ],
        boundary_refs: vec![BoundaryRef {
            id: id("native-values"),
            profile: FileReference {
                file: files[1].clone(),
                schema: id("madb.profile/1"),
            },
        }],
        assets: vec![
            TemplateAsset {
                id: id("main"),
                module: id("consumer"),
                source: Some(files[0].clone()),
                output: path("consumer/main.c"),
                ownership: FileOwnership::User,
                renderer: Some(Renderer::Identifiers {
                    language: IdentifierLanguage::C,
                    parameters: vec![id("namespace")],
                }),
            },
            TemplateAsset {
                id: id("local"),
                module: id("root"),
                source: None,
                output: path(".madb/local.json"),
                ownership: FileOwnership::LocalState,
                renderer: None,
            },
        ],
        ide_entries: vec![],
        task_roots: vec![TemplateTaskRoot {
            id: id("native"),
            module: id("root"),
            recipe: id(VALUES_RECIPE),
            boundary_refs: vec![id("native-values")],
            targets: vec![id("mac-arm64")],
        }],
        validation: vec![
            check(
                "structure",
                CheckLevel::Structure,
                EvidenceLevel::Static,
                &[],
            ),
            check(
                "native-generate",
                CheckLevel::Build,
                EvidenceLevel::Generated,
                &["structure"],
            ),
            check(
                "c-link",
                CheckLevel::Build,
                EvidenceLevel::Linked,
                &["native-generate"],
            ),
            check(
                "ordinary-run",
                CheckLevel::Run,
                EvidenceLevel::HostRun,
                &["c-link"],
            ),
        ],
        files,
    }
}
fn selected(template: Template) -> SelectedTemplate {
    CheckedTemplate::new(template)
        .unwrap()
        .select(None, None)
        .unwrap()
}
fn values() -> ParameterValues {
    BTreeMap::from([
        (id("name"), ParameterValue::String("demo".into())),
        (
            id("namespace"),
            ParameterValue::String("reading_demo".into()),
        ),
    ])
}

#[test]
fn formal_versions_are_finite_and_development_builds_are_not_mapped() {
    for invalid in [
        "",
        "1.0",
        "1.0.0.0",
        "01.0.0",
        "1.-1.0",
        "65536.0.0",
        "1.0.0+build",
        "1.0.0-rc.1",
        "0.1.0-dev.0",
        "latest",
        "1.０.0",
    ] {
        assert!(Version::parse(invalid).is_err(), "{invalid}");
    }
    assert_eq!(version("0.65535.0").to_string(), "0.65535.0");
    assert!(version("1.10.0") > version("1.2.0"));
    assert!(range("1.0.0", "1.0.0").validate().is_err());
    assert!(range("2.0.0", "1.0.0").validate().is_err());
    let checked = CheckedTemplate::new(fixture()).unwrap();
    assert_eq!(
        checked.check_madb_version("1.0.0").unwrap(),
        version("1.0.0")
    );
    for actual in ["0.1.0-dev.0", "0.9.9", "2.0.0"] {
        assert!(checked.check_madb_version(actual).is_err());
    }
}
#[test]
fn typed_defaults_unknown_parameters_and_scalar_lengths() {
    let parameters = vec![
        parameter("name", Some("ignored_for_basename")),
        parameter("namespace", Some("demo")),
        Parameter::Boolean {
            id: id("enabled"),
            required: true,
            default: Some(false),
        },
        Parameter::Integer {
            id: id("count"),
            required: true,
            default: Some(3),
            minimum: -5,
            maximum: 5,
        },
        Parameter::String {
            id: id("optional"),
            required: false,
            default: None,
            min_length: 0,
            max_length: 1,
            pattern: None,
            choices: vec![],
        },
    ];
    let resolved = resolve_parameters(&parameters, &BTreeMap::new(), "project").unwrap();
    assert_eq!(
        resolved[&id("name")],
        ParameterValue::String("project".into())
    );
    assert_eq!(resolved[&id("enabled")], ParameterValue::Boolean(false));
    assert!(!resolved.contains_key(&id("optional")));
    assert!(resolve_parameters(&parameters, &BTreeMap::new(), "中文 项目").is_err());
    let explicit = BTreeMap::from([(id("name"), ParameterValue::String("valid".into()))]);
    assert!(resolve_parameters(&parameters, &explicit, "中文 项目").is_ok());
    for (key, value) in [
        ("unknown", ParameterValue::String("x".into())),
        ("enabled", ParameterValue::Integer(1)),
        ("count", ParameterValue::Boolean(true)),
        ("count", ParameterValue::Integer(6)),
        ("name", ParameterValue::String("x;command".into())),
    ] {
        assert!(
            resolve_parameters(&parameters, &BTreeMap::from([(id(key), value)]), "demo").is_err(),
            "{key}"
        );
    }
    let scalar = Parameter::String {
        id: id("text"),
        required: true,
        default: None,
        min_length: 2,
        max_length: 2,
        pattern: None,
        choices: vec![],
    };
    assert!(
        scalar
            .validate_value(&ParameterValue::String("😀中".into()))
            .is_ok()
    );
    assert!(
        scalar
            .validate_value(&ParameterValue::String("e\u{301}".into()))
            .is_ok()
    );
    assert!(
        scalar
            .validate_value(&ParameterValue::String("😀".into()))
            .is_err()
    );
}
#[test]
fn invalid_default_choices_and_numeric_bounds_fail_even_if_overridden() {
    let mut parameter = parameter("namespace", Some("bad value"));
    assert!(
        resolve_parameters(
            &[parameter.clone()],
            &BTreeMap::from([(id("namespace"), ParameterValue::String("good".into()))]),
            "demo"
        )
        .is_err()
    );
    if let Parameter::String {
        default, choices, ..
    } = &mut parameter
    {
        *default = None;
        *choices = vec!["good".into(), "bad value".into()];
    }
    assert!(resolve_parameters(&[parameter.clone()], &BTreeMap::new(), "demo").is_err());
    if let Parameter::String { choices, .. } = &mut parameter {
        *choices = vec!["good".into(), "good".into()];
    }
    assert!(resolve_parameters(&[parameter], &BTreeMap::new(), "demo").is_err());
    for (min, max, default) in [
        (2, 1, None),
        (-MAX_SAFE_INTEGER - 1, 1, None),
        (0, MAX_SAFE_INTEGER + 1, None),
        (0, 1, Some(2)),
    ] {
        assert!(
            resolve_parameters(
                &[Parameter::Integer {
                    id: id("n"),
                    required: false,
                    default,
                    minimum: min,
                    maximum: max
                }],
                &BTreeMap::new(),
                "demo"
            )
            .is_err()
        );
    }
    assert!(
        resolve_parameters(
            &[self::parameter("missing", None)],
            &BTreeMap::new(),
            "demo"
        )
        .is_err()
    );
}
#[test]
fn fixed_identifier_shape_and_language_reservations() {
    for value in ["Demo", "reading_demo", "x1", "name_", &"a".repeat(48)] {
        assert!(validate_identifier(value).is_ok(), "{value}");
    }
    for value in [
        "",
        "中文",
        "x-y",
        "1x",
        "_x",
        "for",
        "Self",
        "async",
        "gen",
        "memmove",
        "strfuture",
        "isfuture",
        "atomic_future",
        "E9",
        &"x".repeat(49),
        "x\n",
        "x$",
        "x;",
        "x\\",
    ] {
        assert!(validate_identifier(value).is_err(), "{value}");
    }
}
#[test]
fn c11_future_reservations_are_character_sensitive_and_rust_weak_words_are_valid() {
    for allowed in [
        "raw",
        "safe",
        "macro_rules",
        "A__B",
        "FE_demo",
        "LC_demo",
        "SIGdemo",
        "SIG_demo",
        "atomic_X",
        "cnd_X",
        "PRIY",
        "SCNY",
        "INTfuture",
    ] {
        assert!(validate_identifier(allowed).is_ok(), "{allowed}");
    }
    for rejected in [
        "FE_X",
        "LC_X",
        "SIGX",
        "SIG_X",
        "atomic_x",
        "memory_x",
        "ATOMIC_X",
        "TIME_X",
        "PRIu",
        "PRIX",
        "SCNx",
        "SCNX",
        "int_demo_t",
        "UINT_DEMO_MAX",
        "malloc",
        "free",
        "printf",
        "sqrt",
        "sqrtf",
        "sqrtl",
        "cerf",
        "cerfl",
        "clog10f",
        "wmemcpy",
        "va_list",
        "setjmp",
        "static_assert",
        "thread_local",
    ] {
        assert!(validate_identifier(rejected).is_err(), "{rejected}");
    }
}

#[test]
fn declared_and_selected_identities_preserve_exact_plan_and_dag() {
    let checked = CheckedTemplate::new(fixture()).unwrap();
    let selected = checked.select(None, None).unwrap();
    assert_eq!(selected.families(), &[id("macos")]);
    assert_eq!(selected.targets().len(), 1);
    assert_eq!(selected.plan().modules()[0].id, id("root"));
    assert_eq!(selected.plan().files().len(), 2);
    assert_eq!(
        selected
            .validation()
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["structure", "native-generate", "c-link", "ordinary-run"]
    );
    assert!(selected.check_values_recipe_shape(&values()).is_ok());
    assert!(
        checked
            .select(None, Some(&[id("macos"), id("linux")]))
            .is_err()
    );
    assert!(checked.select(Some(&id("unknown")), None).is_err());
    assert!(checked.select(None, Some(&[])).is_err());
    let mut reordered = fixture();
    reordered.modules.reverse();
    reordered.assets.reverse();
    reordered.validation.reverse();
    reordered.profiles[0].modules.reverse();
    reordered.profiles[0].assets.reverse();
    assert_eq!(selected.plan(), selected_template(reordered).plan());
}
fn selected_template(template: Template) -> SelectedTemplate {
    selected(template)
}
#[test]
fn duplicate_declarations_and_dangling_references_fail() {
    let mut mutations = Vec::new();
    let mut t = fixture();
    t.parameters.push(t.parameters[0].clone());
    mutations.push(t);
    let mut t = fixture();
    t.modules.push(t.modules[0].clone());
    mutations.push(t);
    let mut t = fixture();
    t.profiles.push(t.profiles[0].clone());
    mutations.push(t);
    let mut t = fixture();
    t.assets.push(t.assets[0].clone());
    mutations.push(t);
    let mut t = fixture();
    t.profiles[0].assets.push(id("missing"));
    mutations.push(t);
    let mut t = fixture();
    t.profiles[0].assets.push(id("main"));
    mutations.push(t);
    let mut t = fixture();
    let duplicate_target = t.profiles[0].targets[0].clone();
    t.profiles[0].targets.push(duplicate_target);
    mutations.push(t);
    let mut t = fixture();
    t.default_profile = id("missing");
    mutations.push(t);
    let mut t = fixture();
    t.task_roots[0].targets = vec![id("missing")];
    mutations.push(t);
    let mut t = fixture();
    t.validation[0].target = id("missing");
    mutations.push(t);
    let mut t = fixture();
    t.modules[0].dependencies.push(id("consumer"));
    mutations.push(t);
    for (index, t) in mutations.into_iter().enumerate() {
        assert!(CheckedTemplate::new(t).is_err(), "{index}");
    }
}
#[test]
fn source_identity_full_inventory_and_local_state_are_not_interchangeable() {
    let mut mutations = Vec::new();
    let mut t = fixture();
    t.files.remove(0);
    mutations.push(t);
    let mut t = fixture();
    t.assets[0].source.as_mut().unwrap().size += 1;
    mutations.push(t);
    let mut t = fixture();
    t.boundary_refs[0].profile.file.sha256 = Digest::of_bytes(b"different");
    mutations.push(t);
    let mut t = fixture();
    t.assets[1].source = Some(t.files[0].clone());
    mutations.push(t);
    let mut t = fixture();
    t.assets[1].renderer = Some(Renderer::Copy);
    mutations.push(t);
    let mut t = fixture();
    t.assets[0].renderer = None;
    mutations.push(t);
    let mut t = fixture();
    t.files.reverse();
    mutations.push(t);
    for t in mutations {
        assert!(CheckedTemplate::new(t).is_err());
    }
}
#[test]
fn path_inventory_budgets_check_every_prefix_and_unselected_file() {
    for files in [
        vec![file("A/a", 0), file("a/b", 0)],
        vec![file("a", 0), file("a/b", 0)],
        vec![file("z", 0), file("z", 0)],
        vec![file("中文", 0)],
        vec![file(&format!("{}x", "a/".repeat(32)), 0)],
        vec![file("large", MAX_TEMPLATE_FILE_BYTES + 1)],
    ] {
        assert!(check_inventory_paths(&files).is_err());
    }
    let mut total: Vec<_> = (0..4)
        .map(|n| file(&format!("{n}"), MAX_TEMPLATE_FILE_BYTES))
        .collect();
    assert!(check_inventory_paths(&total).is_ok());
    total.push(file("4", 1));
    assert!(check_inventory_paths(&total).is_err());
    let mut count: Vec<_> = (0..4096).map(|n| file(&format!("f{n:04}"), 0)).collect();
    assert!(check_inventory_paths(&count).is_ok());
    count.push(file("manifest", 0));
    assert!(check_inventory_paths(&count).is_err());
    let directories: Vec<_> = (0..4096)
        .map(|n| file(&format!("p{n:04}/a/b/file"), 0))
        .collect();
    assert_eq!(
        check_inventory_paths(&directories).unwrap_err().message,
        "template parent directory count exceeds budget"
    );
    let mut t = fixture();
    t.files.push(file("unused", MAX_TEMPLATE_FILE_BYTES + 1));
    assert!(CheckedTemplate::new(t).is_err());
}
#[test]
fn selected_module_owner_and_combination_closure_are_checked() {
    let mut mutations = Vec::new();
    let mut t = fixture();
    t.profiles[0].modules.retain(|m| m.as_str() != "root");
    mutations.push(t);
    let mut t = fixture();
    t.assets[0].output = path("outside.c");
    mutations.push(t);
    let mut t = fixture();
    t.assets[1].output = path("Consumer/local");
    mutations.push(t);
    let mut t = fixture();
    t.assets[1].output = path("consumer/main.c/child");
    mutations.push(t);
    let mut t = fixture();
    t.profiles[0].task_roots.clear();
    mutations.push(t);
    let mut t = fixture();
    t.profiles[0]
        .validation
        .retain(|v| v.as_str() != "structure");
    mutations.push(t);
    for (index, t) in mutations.into_iter().enumerate() {
        assert!(
            CheckedTemplate::new(t).unwrap().select(None, None).is_err(),
            "{index}"
        );
    }
}
#[test]
fn unknown_ids_remain_typed_but_have_no_fallback_execution() {
    let mut t = fixture();
    t.assets[0].renderer = Some(Renderer::Unknown {
        id: id("vendor.unknown@1"),
    });
    let checked = CheckedTemplate::new(t).unwrap();
    let selected = checked.select(None, None).unwrap();
    assert!(
        matches!(&selected.template().assets[0].renderer,Some(Renderer::Unknown{id}) if id.as_str()=="vendor.unknown@1")
    );
    assert_eq!(
        selected
            .check_values_recipe_shape(&values())
            .unwrap_err()
            .message,
        "unknown renderer cannot execute"
    );
    let mut t = fixture();
    t.task_roots[0].recipe = id("vendor.unknown@1");
    let s = selected_template(t);
    assert!(s.check_values_recipe_shape(&values()).is_err());
    assert!(s.required_checks(CheckLevel::Structure).is_err());
    let mut t = fixture();
    t.boundary_refs[0].profile.schema = id("madb.profile/2");
    let s = selected_template(t);
    assert_eq!(
        s.check_values_recipe_shape(&values()).unwrap_err().message,
        "values recipe supports profile/1 only"
    );
    let mut t = fixture();
    t.modules[1].role = id("unknown-role");
    assert!(
        selected_template(t)
            .check_values_recipe_shape(&values())
            .is_err()
    );
}
#[test]
fn validation_cycle_and_cross_selection_prerequisite_fail() {
    let mut t = fixture();
    t.validation[0].prerequisites.push(id("ordinary-run"));
    assert_eq!(
        CheckedTemplate::new(t)
            .unwrap()
            .select(None, None)
            .unwrap_err()
            .message,
        "validation prerequisite cycle"
    );
    let mut t = fixture();
    t.validation[1].prerequisites.push(id("structure"));
    assert!(CheckedTemplate::new(t).is_err());
}
#[test]
fn independent_required_table_rejects_omission_optional_and_downgrade() {
    let s = selected(fixture());
    assert_eq!(s.required_checks(CheckLevel::Structure).unwrap().len(), 1);
    assert_eq!(s.required_checks(CheckLevel::Build).unwrap().len(), 3);
    assert_eq!(s.required_checks(CheckLevel::Run).unwrap().len(), 4);
    for slot in 0..4 {
        let mut t = fixture();
        t.validation[slot].required = false;
        assert_eq!(
            selected(t)
                .required_checks(CheckLevel::Run)
                .unwrap_err()
                .message,
            "template weakens a built-in required check"
        );
    }
    let mut t = fixture();
    t.profiles[0]
        .validation
        .retain(|v| v.as_str() != "ordinary-run");
    assert_eq!(
        selected(t)
            .required_checks(CheckLevel::Run)
            .unwrap_err()
            .message,
        "template omits a built-in required check"
    );
    let mut t = fixture();
    t.validation[2].minimum_evidence = EvidenceLevel::Compiled;
    assert!(selected(t).required_checks(CheckLevel::Build).is_err());
    let mut t = fixture();
    t.validation[2].level = CheckLevel::Run;
    assert!(selected(t).required_checks(CheckLevel::Build).is_err());
}
#[test]
fn extra_required_and_optional_prerequisite_closure_survive() {
    let mut t = fixture();
    let mut extra = check(
        "extra",
        CheckLevel::Build,
        EvidenceLevel::Compiled,
        &["extra-prior"],
    );
    extra.task_root = None;
    let mut prior = check(
        "extra-prior",
        CheckLevel::Structure,
        EvidenceLevel::Static,
        &[],
    );
    prior.required = false;
    t.profiles[0]
        .validation
        .extend([id("extra"), id("extra-prior")]);
    t.validation.extend([extra, prior]);
    let required = selected(t).required_checks(CheckLevel::Build).unwrap();
    assert_eq!(required.len(), 5);
    assert!(required.iter().any(|c| c.id.as_str() == "extra-prior"));
    let mut t = fixture();
    t.validation[0].prerequisites = vec![id("ordinary-run")];
    t.validation[3].prerequisites.clear();
    assert_eq!(
        selected(t)
            .required_checks(CheckLevel::Structure)
            .unwrap_err()
            .message,
        "required prerequisite exceeds requested verification level"
    );
}
#[test]
fn pointer_semantics_reject_aliases_ancestors_and_invalid_escape() {
    assert_eq!(pointer_tokens("/a~1b/~0key").unwrap(), ["a/b", "~key"]);
    assert_eq!(pointer_tokens("").unwrap(), Vec::<String>::new());
    for pointer in ["a", "/~", "/~2"] {
        assert!(pointer_tokens(pointer).is_err());
    }
    for pointers in [["/a", "/a"], ["/a", "/a/b"], ["", "/a"]] {
        let mut t = fixture();
        t.assets[0].renderer = Some(Renderer::JsonValues {
            bindings: pointers
                .iter()
                .map(|p| JsonBinding {
                    parameter: id("name"),
                    pointer: (*p).into(),
                })
                .collect(),
        });
        assert_eq!(
            CheckedTemplate::new(t).unwrap_err().message,
            "JSON bindings overlap"
        );
    }
    let mut t = fixture();
    t.assets[0].renderer = Some(Renderer::JsonValues {
        bindings: vec![JsonBinding {
            parameter: id("missing"),
            pointer: "/x".into(),
        }],
    });
    assert!(CheckedTemplate::new(t).is_err());
}
#[test]
fn tool_union_keeps_exact_pins_and_host_alternatives_do_not_claim_support() {
    let a = ComponentIdentity {
        id: id("tool"),
        version: "1.2-opaque".into(),
        sha256: Digest::of_bytes(b"tool"),
    };
    assert_eq!(
        merge_tool_requirements(&[std::slice::from_ref(&a), std::slice::from_ref(&a)]).unwrap(),
        std::slice::from_ref(&a)
    );
    let mut b = a.clone();
    b.version = "other".into();
    assert!(merge_tool_requirements(&[std::slice::from_ref(&a), &[b]]).is_err());
    let mut b = a.clone();
    b.sha256 = Digest::of_bytes(b"other");
    assert!(merge_tool_requirements(&[std::slice::from_ref(&a), &[b]]).is_err());
    assert!(merge_tool_requirements(&[&[a.clone(), a]]).is_err());
    let requirements = &fixture().profiles[0].requirements.structure;
    assert!(requirements.matches_declared_host(&id("macos"), &id("aarch64"), version("26.6.2")));
    assert!(!requirements.matches_declared_host(&id("macos"), &id("aarch64"), version("26.6.3")));
    assert!(!requirements.matches_declared_host(&id("linux"), &id("aarch64"), version("26.6.2")));
}

#[test]
fn provenance_is_in_the_same_plan_with_exact_owner_and_no_collision() {
    let selected = selected(fixture());
    let manifest = file("meta/template.json", 123);
    let complete = selected.plan_with_provenance(&manifest).unwrap();
    assert_eq!(complete.files().len(), 6);
    for path in [
        ".madb/template-package.tar",
        ".madb/template-input/meta/template.json",
        ".madb/template-input/profile.json",
        ".madb/template-input/assets/main.c",
    ] {
        let row = complete
            .files()
            .iter()
            .find(|f| f.path.as_str() == path)
            .unwrap();
        assert_eq!(row.module, id("root"));
        assert_eq!(row.ownership, FileOwnership::Provenance);
    }
    assert!(
        selected
            .plan_with_provenance(&file("profile.json", 10))
            .is_err()
    );
    for output in [
        ".madb/template-package.tar",
        ".madb/template-input",
        ".MADB/alias",
        ".madb/template-input/profile.json",
    ] {
        let mut t = fixture();
        t.assets[1].output = path(output);
        assert!(
            selected_template(t)
                .plan_with_provenance(&manifest)
                .is_err(),
            "{output}"
        );
    }
    let mut t = fixture();
    t.modules[0].directory = Some(path("root"));
    t.assets[1].output = path("root/local");
    assert_eq!(
        selected_template(t)
            .plan_with_provenance(&manifest)
            .unwrap_err()
            .message,
        "initial provenance requires a selected root module"
    );
}

#[test]
fn finite_recipe_rejects_required_ide_checks_and_extra_target_instead_of_omitting_them() {
    for level in [CheckLevel::Import, CheckLevel::Debug] {
        let mut t = fixture();
        t.profiles[0].validation.push(id("ide-extra"));
        t.validation
            .push(check("ide-extra", level, EvidenceLevel::HostRun, &[]));
        let checked = CheckedTemplate::new(t).unwrap();
        let chosen = checked.select(None, None).unwrap();
        assert_eq!(
            chosen.required_checks(CheckLevel::Run).unwrap_err().message,
            "values recipe cannot execute required import or debug checks"
        );
        assert!(chosen.check_values_recipe_shape(&values()).is_err());
    }
    let mut t = fixture();
    t.profiles[0].targets[0].family = id("linux");
    assert!(selected(t).check_values_recipe_shape(&values()).is_err());
    let mut t = fixture();
    t.profiles[0].targets[0].boundary_refs.clear();
    assert!(selected(t).check_values_recipe_shape(&values()).is_err());
    let mut t = fixture();
    t.task_roots[0].targets.clear();
    assert_eq!(
        CheckedTemplate::new(t)
            .unwrap()
            .select(None, None)
            .unwrap_err()
            .message,
        "validation target is outside its task root"
    );
}
