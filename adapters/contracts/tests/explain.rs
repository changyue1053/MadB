mod common;
use madb_app::explain::{ExplainFormat, ExplainRequest, PositionEncoding, bind_point, explain};
use madb_app::ports::ReadWorkspace;
use madb_app::{Digest, ErrorKind, Id, LogicalPath, QueryError};
use madb_contracts::explain::ExplainAdapter;
use madb_contracts::{DocumentKind, capability, contract};
use serde_json::{Value, json};
use std::{cell::Cell, collections::BTreeMap};

struct Workspace {
    files: BTreeMap<LogicalPath, Vec<u8>>,
    reads: Cell<usize>,
    change: Option<(usize, &'static str)>,
}
impl ReadWorkspace for Workspace {
    fn read(&self, path: &LogicalPath, _limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        let count = self.reads.get() + 1;
        self.reads.set(count);
        if let Some((at, change)) = self.change
            && count == at
        {
            return match change {
                "host" => Err(QueryError::new(
                    ErrorKind::Host,
                    "HOST001",
                    path.as_str(),
                    "permission denied",
                )),
                "path" => Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "FILE001",
                    path.as_str(),
                    "became a link",
                )),
                "missing" => Ok(None),
                _ => Ok(Some(b"changed bytes".to_vec())),
            };
        }
        Ok(self.files.get(path).cloned())
    }
}
fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).unwrap()
}
fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}
fn context(bytes: &[u8], line: u64, column: u64) -> Value {
    json!({"schema":"madb.diagnostic-context/1","diagnostic":{"code":"ARKTS_DECL_UNSUPPORTED_MODIFIERS","message":"unsupported export/member modifiers","location":{"path":"src/Bad.ets","line":line,"column":column}},
        "source":{"path":"src/Bad.ets","sha256":Digest::of_bytes(bytes).as_str(),"size":bytes.len()},"position_encoding":"utf16-js","reported_precision":"compiler-derived"})
}
fn workspace(value: Value, source: &[u8]) -> Workspace {
    Workspace {
        files: BTreeMap::from([
            (path("issue.json"), serde_json::to_vec(&value).unwrap()),
            (path("src/Bad.ets"), source.to_vec()),
        ]),
        reads: Cell::new(0),
        change: None,
    }
}
fn request() -> ExplainRequest {
    ExplainRequest {
        code: id("ARKTS_DECL_UNSUPPORTED_MODIFIERS"),
        context: Some(path("issue.json")),
    }
}

#[test]
fn all_finite_codes_explain_without_any_workspace_access() {
    let adapter = ExplainAdapter::new().unwrap();
    let catalog: Value = serde_json::from_slice(include_bytes!(
        "../../generators/diagnostic-explanations.json"
    ))
    .unwrap();
    let workspace = Workspace {
        files: BTreeMap::new(),
        reads: Cell::new(0),
        change: None,
    };
    assert_eq!(catalog["entries"].as_array().unwrap().len(), 52);
    assert!(
        catalog["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["code"] == "DOCTOR001")
    );
    for entry in catalog["entries"].as_array().unwrap() {
        let code = id(entry["code"].as_str().unwrap());
        let result = explain(
            &workspace,
            &adapter,
            &ExplainRequest {
                code: code.clone(),
                context: None,
            },
        );
        let encoded = adapter.encode_result("test", Some(&code), &result).unwrap();
        assert_eq!(encoded.last(), Some(&b'\n'));
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["exit_code"], 0);
        assert!(value["source"].is_null());
    }
    assert_eq!(workspace.reads.get(), 0);
}
#[test]
fn unknown_and_case_mismatched_codes_fail_before_reading_context() {
    let adapter = ExplainAdapter::new().unwrap();
    let workspace = Workspace {
        files: BTreeMap::new(),
        reads: Cell::new(0),
        change: None,
    };
    for code in ["SEM099", "sem001", "ARKTS_DECL_FUTURE", "FMT004"] {
        let code = id(code);
        let result = explain(
            &workspace,
            &adapter,
            &ExplainRequest {
                code: code.clone(),
                context: Some(path("missing.json")),
            },
        );
        assert_eq!(result.as_ref().unwrap_err().findings[0].code, "EXPLAIN001");
        let value: Value =
            serde_json::from_slice(&adapter.encode_result("test", Some(&code), &result).unwrap())
                .unwrap();
        assert_eq!(value["exit_code"], 2);
        assert!(value["explanation"].is_null());
    }
    assert_eq!(workspace.reads.get(), 0);
}
#[test]
fn source_bom_and_utf16_point_bind_raw_bytes_and_recheck_both_inputs() {
    let adapter = ExplainAdapter::new().unwrap();
    let bytes = "\u{feff}/* 🍎 */ export async function bad(): Promise<void> {}".as_bytes();
    assert!(bytes[14..].starts_with(b"export"));
    let workspace = workspace(context(bytes, 1, 10), bytes);
    let result = explain(&workspace, &adapter, &request()).unwrap();
    assert_eq!(result.source.as_ref().unwrap().byte_offset, 14);
    assert_eq!(workspace.reads.get(), 4);
    let value: Value = serde_json::from_slice(
        &adapter
            .encode_result("test", Some(&request().code), &Ok(result))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value["source"]["reference"]["sha256"],
        Digest::of_bytes(bytes).as_str()
    );
    assert_eq!(value["source"]["byte_offset"], 14);
    assert_eq!(value["explanation"]["repairs"][0]["location"]["column"], 10);
}
#[test]
fn position_modes_have_explicit_distinct_byte_and_line_rules() {
    let js = PositionEncoding::Utf16Js;
    let byte = PositionEncoding::ByteLf;
    for (bytes, mode, line, column, want) in [
        ("\u{feff}export".as_bytes(), js, 1, 1, Some(3)),
        ("\u{feff}\u{feff}x".as_bytes(), js, 1, 2, Some(6)),
        ("😀x".as_bytes(), js, 1, 3, Some(4)),
        ("😀x".as_bytes(), js, 1, 2, None),
        (b"a\r\nb".as_slice(), js, 2, 1, Some(3)),
        (b"a\rb".as_slice(), js, 2, 1, Some(2)),
        ("a\u{2028}b".as_bytes(), js, 2, 1, Some(4)),
        ("a\u{2029}b".as_bytes(), js, 2, 1, Some(4)),
        (b"a\rb".as_slice(), byte, 1, 3, Some(2)),
        (b"a".as_slice(), js, 1, 2, Some(1)),
        (b"".as_slice(), js, 1, 1, Some(0)),
        (b"\xff".as_slice(), js, 1, 1, None),
        (b"a".as_slice(), byte, 0, 1, None),
    ] {
        assert_eq!(bind_point(bytes, mode, line, column), want);
    }
}
#[test]
fn initial_source_pin_path_point_and_code_mismatch_are_invalid_without_partial_success() {
    let adapter = ExplainAdapter::new().unwrap();
    let bytes = "😀 export".as_bytes();
    for mutation in 0..6 {
        let mut value = context(bytes, 1, 4);
        match mutation {
            0 => value["source"]["sha256"] = json!(format!("sha256:{}", "0".repeat(64))),
            1 => value["source"]["size"] = json!(999),
            2 => value["source"]["path"] = json!("other.ets"),
            3 => value["diagnostic"]["location"]["column"] = json!(2),
            4 => value["diagnostic"]["code"] = json!("SEM007"),
            _ => value["diagnostic"]["location"]["line"] = json!(999),
        }
        let result = explain(&workspace(value, bytes), &adapter, &request());
        assert_eq!(result.as_ref().unwrap_err().kind, ErrorKind::InvalidInput);
        let encoded = adapter
            .encode_result("test", Some(&request().code), &result)
            .unwrap();
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert!(value["source"].is_null() && value["explanation"].is_null());
        assert_eq!(value["exit_code"], 2);
    }
}
#[test]
fn recheck_io_remains_host_while_bytes_disappearance_and_boundary_change_are_conflicts() {
    let adapter = ExplainAdapter::new().unwrap();
    let bytes = b"export";
    for (change, kind, exit) in [
        ("host", ErrorKind::Host, 3),
        ("changed", ErrorKind::Conflict, 4),
        ("missing", ErrorKind::Conflict, 4),
        ("path", ErrorKind::Conflict, 4),
    ] {
        let mut workspace = workspace(context(bytes, 1, 1), bytes);
        workspace.change = Some((4, change));
        let result = explain(&workspace, &adapter, &request());
        assert_eq!(result.as_ref().unwrap_err().kind, kind);
        let encoded = adapter
            .encode_result("test", Some(&request().code), &result)
            .unwrap();
        let value: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["exit_code"], exit);
    }
}
#[test]
fn missing_files_and_oversize_inputs_do_not_become_success() {
    let adapter = ExplainAdapter::new().unwrap();
    let workspace = Workspace {
        files: BTreeMap::new(),
        reads: Cell::new(0),
        change: None,
    };
    assert_eq!(
        explain(&workspace, &adapter, &request()).unwrap_err().kind,
        ErrorKind::Host
    );
    let bytes = vec![b'x'; 1024 * 1024 + 1];
    assert_eq!(
        explain(
            &self::workspace(context(&bytes, 1, 1), &bytes),
            &adapter,
            &request()
        )
        .unwrap_err()
        .kind,
        ErrorKind::InvalidInput
    );
}
#[test]
fn strict_context_decoder_rejects_ambiguous_and_unknown_data() {
    let adapter = ExplainAdapter::new().unwrap();
    for bytes in [
        br#"{"schema":"madb.diagnostic-context/1","schema":"madb.diagnostic-context/1"}"#
            .as_slice(),
        b"{\"line\":1.0}",
        b"{\"line\":9007199254740992}",
    ] {
        assert_eq!(
            adapter.decode_context(bytes).unwrap_err().findings[0].code,
            "EXPLAIN002"
        );
    }
    for mutation in 0..5 {
        let mut value = context(b"export", 1, 1);
        match mutation {
            0 => value["unknown"] = json!(1),
            1 => value["position_encoding"] = json!("unicode-scalar"),
            2 => value["source"] = Value::Null,
            3 => value["diagnostic"]["location"]["line"] = json!(0),
            _ => value["schema"] = json!("madb.diagnostic-context/2"),
        }
        assert!(
            adapter
                .decode_context(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
}
#[test]
fn actual_rules_failure_has_an_explanation_without_claiming_unavailable_source() {
    let adapter = ExplainAdapter::new().unwrap();
    let mut model = contract(&common::document(DocumentKind::ContractIr)).unwrap();
    let cap = capability(&common::document(DocumentKind::Profile)).unwrap();
    model.policies.clear();
    model.symbols[0].precision = madb_model::contract::Precision::Heuristic;
    let findings = madb_rules::validate_contract(&model, &cap);
    for code in ["SEM001", "CAP003"] {
        let finding = findings
            .iter()
            .find(|finding| finding.code == code)
            .unwrap();
        let value = json!({"schema":"madb.diagnostic-context/1","diagnostic":{"code":finding.code,"message":finding.message,"location":null},"source":null,"position_encoding":null,"reported_precision":null});
        let workspace = self::workspace(value, b"");
        let result = explain(
            &workspace,
            &adapter,
            &ExplainRequest {
                code: id(code),
                context: Some(path("issue.json")),
            },
        )
        .unwrap();
        assert!(result.source.is_none());
        if code == "CAP003" {
            assert!(result.explanation.meaning.contains("Heuristic"));
        }
        assert_eq!(workspace.reads.get(), 2);
    }
}
