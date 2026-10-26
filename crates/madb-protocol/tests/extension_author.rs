use madb_protocol::{
    ExtensionFrame, Value, byte_digest,
    extension_author::{AuthorEvent, AuthorSession, read_frame, select_version},
};
use serde_json::json;
use std::io::Cursor;

fn setup() -> (AuthorSession, Value) {
    let manifest = json!({"schema":"madb.extension/1","id":"test.helper","version":"0.1.0","compatibility":{"protocol":["1.2","1.9","1.10"],"ir":["1.0"]},"languages":[],"components":[{"id":"builder","port":"toolchain","supported_features":["core.values@1"],"entrypoint":{"kind":"process","path":"bin/helper","args":[]}}],"dependencies":[],"routes":[],"assets":[],"execution":{"transport":"stdio-jsonl","network":"none","env":[]}});
    let bytes = serde_json::to_vec(&manifest).unwrap();
    (
        AuthorSession::from_manifest(&bytes, "builder").unwrap(),
        json!({"schema":"madb.extension-wire/1","kind":"hello","session_id":"s","extension_id":"test.helper","extension_version":"0.1.0","component_id":"builder","manifest_digest":byte_digest(&bytes),"protocol_versions":["1.9","1.10"],"ir_versions":["1.0"],"required_features":["core.values@1"]}),
    )
}
fn frame(value: Value) -> ExtensionFrame {
    ExtensionFrame::from_value(value).unwrap()
}
fn invoke() -> Value {
    json!({"schema":"madb.extension-wire/1","kind":"invoke","session_id":"s","protocol":"1.10","ir":"1.0","request_id":"r","component":"test.helper/builder","operation":"build","input":{"path":"inputs/request.json","schema":"madb.build-task/1","sha256":format!("sha256:{}","1".repeat(64)),"size":1},"output_root":"outputs/r","timeout_ms":1000})
}
fn output() -> Value {
    json!({"path":"outputs/r/result.json","schema":"madb.build-evidence/1","sha256":format!("sha256:{}","1".repeat(64)),"size":1})
}
fn ready() -> AuthorSession {
    let (mut author, hello) = setup();
    assert!(matches!(
        author.receive(frame(hello)).unwrap(),
        AuthorEvent::Reply(_)
    ));
    author
}

#[test]
fn separate_version_intersections_choose_numeric_highest_without_invented_minor() {
    let (mut author, hello) = setup();
    let AuthorEvent::Reply(ack) = author.receive(frame(hello)).unwrap() else {
        panic!()
    };
    assert_eq!(ack.value()["protocol"], "1.10");
    assert_eq!(ack.value()["ir"], "1.0");
    assert_eq!(select_version(&json!(["1.10"]), &json!(["1.9"])), None);
    assert_eq!(select_version(&json!(["bad"]), &json!(["bad"])), None);
}
#[test]
fn handshake_rejections_preserve_identity_version_and_capability_categories() {
    for (key, value, code) in [
        (
            "extension_version",
            json!("0.2.0"),
            "ExtensionIdentityMismatch",
        ),
        (
            "manifest_digest",
            json!(format!("sha256:{}", "0".repeat(64))),
            "ExtensionIdentityMismatch",
        ),
        ("protocol_versions", json!(["2.0"]), "ProtocolIncompatible"),
        ("ir_versions", json!(["2.0"]), "ProtocolIncompatible"),
        (
            "required_features",
            json!(["unknown.feature@1"]),
            "CapabilityUnsupported",
        ),
    ] {
        let (mut author, mut hello) = setup();
        hello[key] = value;
        let AuthorEvent::Reply(reject) = author.receive(frame(hello)).unwrap() else {
            panic!()
        };
        assert_eq!(reject.value()["kind"], "reject");
        assert_eq!(reject.value()["error"]["code"], code);
    }
}
#[test]
fn component_operation_session_and_versions_cannot_change_after_handshake() {
    for (key, value) in [
        ("component", json!("other.package/builder")),
        ("operation", json!("generate")),
        ("session_id", json!("other")),
        ("protocol", json!("1.9")),
        ("ir", json!("2.0")),
    ] {
        let mut author = ready();
        let mut request = invoke();
        request[key] = value;
        assert!(author.receive(frame(request)).is_err());
        assert!(author.complete(output()).is_err());
    }
}
#[test]
fn one_request_one_terminal_and_idempotent_cancel() {
    let mut author = ready();
    assert!(matches!(
        author.receive(frame(invoke())).unwrap(),
        AuthorEvent::Invoke(_)
    ));
    let cancel = json!({"schema":"madb.extension-wire/1","kind":"cancel","session_id":"s","protocol":"1.10","ir":"1.0","request_id":"r"});
    for _ in 0..2 {
        assert!(matches!(
            author.receive(frame(cancel.clone())).unwrap(),
            AuthorEvent::Cancel
        ));
    }
    assert_eq!(author.cancelled().unwrap().value()["status"], "cancelled");
    assert!(author.cancelled().is_err());
    assert!(author.complete(output()).is_err());
    assert!(author.receive(frame(invoke())).is_err());
}
#[test]
fn no_terminal_before_invocation_and_cancel_targets_only_active_request() {
    let mut author = ready();
    assert!(author.complete(output()).is_err());
    assert!(author.cancelled().is_err());
    author.receive(frame(invoke())).unwrap();
    let cancel = json!({"schema":"madb.extension-wire/1","kind":"cancel","session_id":"s","protocol":"1.10","ir":"1.0","request_id":"wrong"});
    assert_eq!(
        author.receive(frame(cancel)).err().unwrap().code,
        "ProtocolInvalid"
    );
}
#[test]
fn bounded_reader_does_not_consume_the_next_frame_or_accept_partial_eof() {
    let (_, hello) = setup();
    let one = frame(hello).encode_line().unwrap();
    let mut bytes = one.clone();
    bytes.extend_from_slice(&one);
    let mut reader = Cursor::new(bytes);
    assert!(read_frame(&mut reader).unwrap().is_some());
    assert_eq!(reader.position(), one.len() as u64);
    assert!(read_frame(&mut reader).unwrap().is_some());
    assert!(read_frame(&mut reader).unwrap().is_none());
    let mut truncated = Cursor::new(one[..one.len() - 1].to_vec());
    assert_eq!(
        read_frame(&mut truncated).err().unwrap().code,
        "ProtocolInvalid"
    );
}
