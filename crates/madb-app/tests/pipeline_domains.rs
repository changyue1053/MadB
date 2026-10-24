use madb_app::{LogicalPath, pipeline::validate_observed_locations};
fn p(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
#[test]
fn observed_search_tree_cannot_contain_cache_or_runs() {
    assert!(
        validate_observed_locations(
            &p("out"),
            &p("runs"),
            Some(&p("include/cache")),
            &[p("include")]
        )
        .is_err()
    );
    assert!(validate_observed_locations(&p("out"), &p("src/runs"), None, &[p("src")]).is_err());
}
#[test]
fn absence_condition_cannot_be_inside_writable_domain() {
    assert!(
        validate_observed_locations(&p("out"), &p("runs"), None, &[p("runs/optional")]).is_err()
    );
    assert!(
        validate_observed_locations(&p("out"), &p("runs"), None, &[p("out/optional")]).is_err()
    );
}
#[test]
fn independent_observation_and_managed_domains_are_allowed() {
    validate_observed_locations(
        &p(".madb/output"),
        &p(".madb/runs"),
        Some(&p(".madb/cache")),
        &[p("src"), p("include/optional.h")],
    )
    .unwrap();
}
