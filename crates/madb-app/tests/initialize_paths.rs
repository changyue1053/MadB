use madb_app::use_cases::initialize::{AbsolutePath, InitialFile};
use madb_app::{Id, LogicalPath};
use madb_plan::project::FileOwnership;
#[test]
fn output_root_is_lexical_absolute_and_keeps_unicode_and_spaces() {
    assert_eq!(
        AbsolutePath::new("/model/中文 目录/./a/../demo")
            .unwrap()
            .as_str(),
        "/model/中文 目录/demo"
    );
    for value in ["relative", "/", "/../escape", "/model/\nname"] {
        assert!(AbsolutePath::new(value).is_err(), "{value:?}");
    }
}
#[test]
fn local_state_never_acquires_placeholder_bytes() {
    let owner = Id::new("root").unwrap();
    let path = LogicalPath::new(".madb/local.json").unwrap();
    assert!(
        InitialFile::rendered(
            owner.clone(),
            path.clone(),
            FileOwnership::LocalState,
            Vec::new()
        )
        .is_err()
    );
    let reserved = InitialFile::local_state(owner, path);
    assert!(reserved.bytes().is_none());
    assert!(reserved.reference().is_none());
}
