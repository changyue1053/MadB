use madb_model::{Id, LogicalPath};
use madb_plan::project::{FileOwnership, ProjectFile, ProjectModule, ProjectPlan};

fn module(id: &str, directory: Option<&str>, dependencies: &[&str]) -> ProjectModule {
    ProjectModule {
        id: Id::new(id).unwrap(),
        directory: directory.map(|path| LogicalPath::new(path).unwrap()),
        dependencies: dependencies
            .iter()
            .map(|id| Id::new(*id).unwrap())
            .collect(),
    }
}

fn file(module: &str, path: &str, ownership: FileOwnership) -> ProjectFile {
    ProjectFile {
        module: Id::new(module).unwrap(),
        path: LogicalPath::new(path).unwrap(),
        ownership,
    }
}

#[test]
fn plan_orders_dependencies_and_files_without_changing_ownership() {
    let modules = vec![
        module("app", Some("app"), &["shared", "root"]),
        module("shared", Some("shared"), &["root"]),
        module("root", None, &[]),
    ];
    let files = vec![
        file("app", "app/Main.kt", FileOwnership::User),
        file("root", "tool.lock", FileOwnership::Provenance),
        file("shared", "shared/generated/Api.kt", FileOwnership::Managed),
        file("root", ".local/sdk.properties", FileOwnership::LocalState),
    ];
    let plan = ProjectPlan::new(modules.clone(), files.clone()).unwrap();
    let mut reordered_modules: Vec<_> = modules.into_iter().rev().collect();
    reordered_modules[2].dependencies.reverse();
    let reversed = ProjectPlan::new(reordered_modules, files.into_iter().rev().collect()).unwrap();
    assert_eq!(plan, reversed);
    assert_eq!(
        plan.modules()
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["root", "shared", "app"]
    );
    assert_eq!(
        plan.files().iter().map(|f| f.ownership).collect::<Vec<_>>(),
        [
            FileOwnership::LocalState,
            FileOwnership::User,
            FileOwnership::Managed,
            FileOwnership::Provenance
        ]
    );
}

#[test]
fn graph_rejects_missing_duplicate_and_cyclic_dependencies() {
    for (modules, message) in [
        (
            vec![module("a", None, &["absent"])],
            "missing module dependency",
        ),
        (
            vec![module("a", None, &[]), module("b", Some("b"), &["a", "a"])],
            "duplicate module dependency",
        ),
        (vec![module("a", None, &["a"])], "module dependency cycle"),
        (
            vec![module("a", None, &["b"]), module("b", Some("b"), &["a"])],
            "module dependency cycle",
        ),
    ] {
        assert_eq!(
            ProjectPlan::new(modules, vec![]).unwrap_err().message,
            message
        );
    }
}

#[test]
fn module_identities_and_directories_have_one_owner() {
    for (modules, message) in [
        (vec![], "project has no modules"),
        (
            vec![module("a", None, &[]), module("a", Some("a"), &[])],
            "duplicate module identity",
        ),
        (
            vec![
                module("a", Some("same"), &[]),
                module("b", Some("same"), &[]),
            ],
            "multiple modules claim the same directory",
        ),
        (
            vec![module("a", None, &[]), module("b", None, &[])],
            "multiple modules claim the same directory",
        ),
    ] {
        assert_eq!(
            ProjectPlan::new(modules, vec![]).unwrap_err().message,
            message
        );
    }
}

#[test]
fn every_file_names_an_existing_owner_and_stays_in_its_directory() {
    let modules = vec![module("shared", Some("shared"), &[])];
    for (asset, message) in [
        (
            file("absent", "shared/API.h", FileOwnership::Managed),
            "file owner is not a project module",
        ),
        (
            file("shared", "shared-other/API.h", FileOwnership::Managed),
            "file is outside its owning module directory",
        ),
        (
            file("shared", "shared", FileOwnership::Managed),
            "file is outside its owning module directory",
        ),
    ] {
        let error = ProjectPlan::new(modules.clone(), vec![asset.clone()]).unwrap_err();
        assert_eq!(error.subject, asset.path.as_str());
        assert_eq!(error.message, message);
    }
}

#[test]
fn conflicting_files_and_case_aliased_parents_are_rejected() {
    for (first, second) in [
        ("out/File.h", "out/file.h"),
        ("Out/a", "out/b"),
        ("out/A/x", "out/a/y"),
        ("out/api/header.h", "out/api"),
        ("out/api", "out/api/header.h"),
        ("same", "same"),
    ] {
        for ownership in [FileOwnership::User, FileOwnership::Managed] {
            assert!(
                ProjectPlan::new(
                    vec![module("root", None, &[])],
                    vec![
                        file("root", first, ownership),
                        file("root", second, FileOwnership::Managed)
                    ],
                )
                .is_err(),
                "{first} / {second}"
            );
        }
    }
}

#[test]
fn module_directory_claims_participate_even_without_files() {
    for (modules, files, message) in [
        (
            vec![
                module("a", Some("Modules/a"), &[]),
                module("b", Some("modules/b"), &[]),
            ],
            vec![],
            "case-aliased project path",
        ),
        (
            vec![module("a", Some("out/api"), &[]), module("root", None, &[])],
            vec![file("root", "out", FileOwnership::User)],
            "duplicate file or file/directory collision",
        ),
    ] {
        assert_eq!(
            ProjectPlan::new(modules, files).unwrap_err().message,
            message
        );
    }
}

#[test]
fn file_ownership_cannot_be_claimed_by_two_modules() {
    let error = ProjectPlan::new(
        vec![
            module("root", None, &[]),
            module("app", Some("app"), &["root"]),
        ],
        vec![
            file("root", "app/Main.kt", FileOwnership::User),
            file("app", "app/Main.kt", FileOwnership::Managed),
        ],
    )
    .unwrap_err();
    assert_eq!(error.message, "duplicate file or file/directory collision");
}

#[test]
fn portable_subset_does_not_claim_unicode_alias_safety() {
    let error = ProjectPlan::new(
        vec![module("root", None, &[])],
        vec![file("root", "模块/API.h", FileOwnership::User)],
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "portable project assets require ASCII logical paths"
    );
}

#[test]
fn invalid_relative_paths_cannot_enter_the_plan() {
    for path in [
        "",
        ".",
        "..",
        "../file",
        "a/../file",
        "/absolute",
        "a//file",
        "a/./file",
        "a/",
        "C:/file",
        "a\\file",
        "a/\nfile",
    ] {
        assert!(LogicalPath::new(path).is_err(), "{path:?}");
    }
}
