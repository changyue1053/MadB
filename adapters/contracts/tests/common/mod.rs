#![allow(dead_code)]
use madb_contracts::{Document, DocumentKind, SchemaRegistry};
use madb_protocol::{Value, strict_json};
use std::{path::PathBuf, sync::OnceLock};
pub fn registry() -> &'static SchemaRegistry {
    static R: OnceLock<SchemaRegistry> = OnceLock::new();
    R.get_or_init(|| SchemaRegistry::new().unwrap())
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn value(kind: DocumentKind) -> Value {
    strict_json(
        &std::fs::read(root().join(format!("tests/fixtures/contracts/{}.json", kind.name())))
            .unwrap(),
    )
    .unwrap()
}
pub fn document(kind: DocumentKind) -> Document {
    registry().validate(kind, value(kind)).unwrap()
}
pub fn changed(kind: DocumentKind, edit: impl FnOnce(&mut Value)) -> Document {
    let mut v = value(kind);
    edit(&mut v);
    registry().validate(kind, v).unwrap()
}
pub fn id(s: &str) -> madb_model::Id {
    madb_model::Id::new(s).unwrap()
}
pub fn digest(s: &str) -> madb_model::Digest {
    madb_model::Digest::new(madb_protocol::byte_digest(s.as_bytes())).unwrap()
}
pub fn assert_code(findings: &[madb_model::Finding], code: &str) {
    assert!(
        findings.iter().any(|f| f.code == code),
        "missing {code}: {findings:?}"
    );
}
