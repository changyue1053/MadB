use madb_protocol::{
    ExtensionFrame, MAX_EXTENSION_FRAME_BYTES, MAX_EXTENSION_FRAME_DEPTH, Value, strict_json,
};
use serde_json::json;

fn vectors() -> Value {
    strict_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/examples/extensions/schema-vectors.json"
    )))
    .unwrap()
}

fn hello() -> Value {
    json!({
        "schema": "madb.extension-wire/1", "kind": "hello", "session_id": "session-1",
        "extension_id": "example.peer", "extension_version": "0.1.0", "component_id": "reader",
        "manifest_digest": format!("sha256:{}", "1".repeat(64)),
        "protocol_versions": ["1.0"], "ir_versions": ["1.0"], "required_features": []
    })
}

fn line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn decoder_and_encoder_share_existing_wire_vectors() {
    let mut checked = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for vector in vectors()["vectors"].as_array().unwrap() {
        if vector["subject"] != "frame" {
            continue;
        }
        checked += 1;
        let decoded = ExtensionFrame::decode_line(&line(&vector["document"]));
        assert_eq!(
            decoded.is_ok(),
            vector["expected_schema"] == "valid",
            "{}: {decoded:?}",
            vector["id"]
        );
        match decoded {
            Ok(frame) => {
                kinds.insert(frame.value()["kind"].as_str().unwrap().to_owned());
                assert_eq!(frame.value(), &vector["document"]);
                let encoded = frame.encode_line().unwrap();
                assert_eq!(encoded.iter().filter(|&&c| c == b'\n').count(), 1);
                assert_eq!(
                    ExtensionFrame::decode_line(&encoded).unwrap().value(),
                    frame.value()
                );
                assert_eq!(
                    ExtensionFrame::from_value(vector["document"].clone())
                        .unwrap()
                        .encode_line()
                        .unwrap(),
                    encoded
                );
            }
            Err(error) => assert_eq!(error.code, "ProtocolInvalid", "{error}"),
        }
    }
    assert_eq!(checked, 14);
    assert_eq!(kinds.len(), 6);
}

#[test]
fn missing_fields_are_rejected_for_every_envelope_kind() {
    let schema: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/schemas/extension-wire.schema.json"
    )))
    .unwrap();
    for vector in vectors()["vectors"].as_array().unwrap() {
        if vector["subject"] != "frame" || vector["expected_schema"] != "valid" {
            continue;
        }
        let kind = vector["document"]["kind"].as_str().unwrap();
        for field in schema["$defs"][kind]["required"].as_array().unwrap() {
            let mut missing = vector["document"].clone();
            missing
                .as_object_mut()
                .unwrap()
                .remove(field.as_str().unwrap());
            let error = ExtensionFrame::decode_line(&line(&missing)).unwrap_err();
            assert_eq!(error.code, "ProtocolInvalid", "{kind} {field}: {error}");
        }
    }
}

#[test]
fn malformed_lines_are_not_repaired_or_read_as_multiple_frames() {
    let valid = line(&hello());
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid);
    let mut embedded_line = valid.clone();
    embedded_line.insert(1, b'\n');
    let mut invalid_utf8 = valid.clone();
    invalid_utf8[1] = 0xff;
    for bytes in [
        b"log: ready\n".to_vec(),
        b"\n".to_vec(),
        valid[..valid.len() - 1].to_vec(),
        duplicate,
        embedded_line,
        invalid_utf8,
        b"{\"kind\":\"hello\",\"kind\":\"hello_ack\"}\n".to_vec(),
        b"{} {}\n".to_vec(),
    ] {
        assert_eq!(
            ExtensionFrame::decode_line(&bytes).unwrap_err().code,
            "ProtocolInvalid"
        );
    }
}

#[test]
fn byte_limit_counts_the_whole_line_including_its_delimiter() {
    let mut bytes = line(&hello());
    bytes.pop();
    bytes.resize(MAX_EXTENSION_FRAME_BYTES - 1, b' ');
    bytes.push(b'\n');
    ExtensionFrame::decode_line(&bytes).unwrap();
    bytes.insert(0, b' ');
    let error = ExtensionFrame::decode_line(&bytes).unwrap_err();
    assert_eq!(error.code, "ProtocolInvalid");
    assert!(error.message.contains("byte budget"));
}

#[test]
fn decoded_values_cannot_bypass_nesting_or_numeric_limits() {
    let mut nested = Value::Null;
    for _ in 0..MAX_EXTENSION_FRAME_DEPTH {
        nested = Value::Array(vec![nested]);
    }
    // Exactly 32 containers reaches schema checking, while 33 fails the budget.
    let error = ExtensionFrame::from_value(nested.clone()).unwrap_err();
    assert!(error.message.starts_with("FMT004:"), "{error}");
    let excessive = Value::Array(vec![nested]);
    for error in [
        ExtensionFrame::decode_line(&line(&excessive)).unwrap_err(),
        ExtensionFrame::from_value(excessive).unwrap_err(),
    ] {
        assert_eq!(error.code, "ProtocolInvalid");
        assert!(error.message.contains("nesting budget"), "{error}");
    }
    let mut numeric = hello();
    numeric["extra"] = json!(9007199254740992_u64);
    let error = ExtensionFrame::from_value(numeric).unwrap_err();
    assert!(error.message.starts_with("FMT006:"), "{error}");
}

#[test]
fn escaped_newline_is_data_and_crlf_is_valid_json_whitespace() {
    let value = json!({
        "schema": "madb.extension-wire/1", "kind": "reject", "session_id": "session-1",
        "error": {"code": "ToolFailed", "stage": "handshake", "message": "first\nsecond"}
    });
    let mut bytes = line(&value);
    bytes.insert(bytes.len() - 1, b'\r');
    let frame = ExtensionFrame::decode_line(&bytes).unwrap();
    assert_eq!(frame.value(), &value);
    assert_eq!(frame.encode_line().unwrap(), line(&value));
}

#[test]
fn format_validity_does_not_claim_negotiation_or_file_access() {
    let mut value = hello();
    value["required_features"] = json!(["other.unknown@99"]);
    ExtensionFrame::from_value(value).unwrap();
    // Schema shape alone cannot authorize an unknown feature or a real file;
    // those checks must stay at the contracts/host boundaries.
}
