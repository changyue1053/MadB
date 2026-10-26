use madb_app::Digest;
use madb_contracts::{DocumentKind, SchemaRegistry};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "madb-explain-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_madb"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn machine(output: Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.ends_with(b"\n"));
    let value = SchemaRegistry::new()
        .unwrap()
        .decode(DocumentKind::ExplainResult, &output.stdout)
        .unwrap()
        .into_value();
    assert_eq!(value["exit_code"], status);
    value
}

#[test]
fn code_only_unknown_and_argument_failures_have_exact_machine_exit() {
    let workspace = Workspace::new();
    let known = machine(workspace.run(&["explain", "SEM007", "--json"]), 0);
    assert_eq!(known["code"], "SEM007");
    assert!(known["context"].is_null() && known["source"].is_null());
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
    for code in ["UNKNOWN999", "sem007", "TYPE-001"] {
        let result = machine(
            workspace.run(&[
                "explain",
                code,
                "--context",
                "absent.json",
                "--root",
                "absent-directory",
                "--json",
            ]),
            2,
        );
        assert_eq!(result["diagnostics"][0]["code"], "EXPLAIN001");
    }
    for args in [
        vec!["explain", "--json"],
        vec!["explain", "bad code", "--json"],
        vec!["explain", "SEM007", "--root", ".", "--json"],
        vec!["explain", "SEM007", "--wat", "--json"],
        vec!["explain", "SEM007", "--context", "-", "--json"],
    ] {
        let result = machine(workspace.run(&args), 2);
        assert_eq!(result["diagnostics"][0]["code"], "EXPLAIN002");
        assert!(result["explanation"].is_null() && result["source"].is_null());
    }
    let host = machine(
        workspace.run(&["explain", "SEM007", "--context", "absent.json", "--json"]),
        3,
    );
    assert_eq!(host["diagnostics"][0]["code"], "EXPLAIN003");
}

#[test]
fn source_binding_checks_original_utf8_bytes_without_writes() {
    let workspace = Workspace::new();
    let source = "\u{feff}a😀x\r\nnext\n".as_bytes();
    fs::write(workspace.0.join("Source.ets"), source).unwrap();
    let mut context = json!({
        "schema":"madb.diagnostic-context/1",
        "diagnostic":{"code":"ARKTS_DECL_UNSUPPORTED_MODIFIERS","message":"actual source point supplied explicitly", "location":{"path":"Source.ets","line":1,"column":4}},
        "source":{"path":"Source.ets","sha256":Digest::of_bytes(source).as_str(),"size":source.len()},
        "position_encoding":"utf16-js","reported_precision":"compiler-derived"
    });
    let bytes = serde_json::to_vec(&context).unwrap();
    fs::write(workspace.0.join("context.json"), &bytes).unwrap();
    let args = [
        "explain",
        "ARKTS_DECL_UNSUPPORTED_MODIFIERS",
        "--context",
        "context.json",
        "--json",
    ];
    let result = machine(workspace.run(&args), 0);
    assert_eq!(result["source"]["byte_offset"], 8);
    assert_eq!(
        result["source"]["reference"]["sha256"],
        Digest::of_bytes(source).as_str()
    );
    assert_eq!(fs::read(workspace.0.join("Source.ets")).unwrap(), source);
    assert_eq!(fs::read(workspace.0.join("context.json")).unwrap(), bytes);
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 2);
    context["diagnostic"]["location"]["column"] = json!(3); // middle of the astral scalar
    fs::write(
        workspace.0.join("context.json"),
        serde_json::to_vec(&context).unwrap(),
    )
    .unwrap();
    assert_eq!(
        machine(workspace.run(&args), 2)["diagnostics"][0]["code"],
        "EXPLAIN002"
    );
    fs::write(workspace.0.join("context.json"), &bytes).unwrap();
    fs::write(workspace.0.join("Source.ets"), b"changed").unwrap();
    assert_eq!(
        machine(workspace.run(&args), 2)["diagnostics"][0]["code"],
        "EXPLAIN002"
    );
}

#[test]
fn help_text_streams_and_legacy_parser_remain_distinct() {
    let workspace = Workspace::new();
    for flag in ["--help", "--version"] {
        let output = workspace.run(&["explain", flag, "--json"]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.starts_with(b"{"));
    }
    let output = workspace.run(&["explain", "SEM007"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("SEM007:")
    );
    let output = workspace.run(&["explain", "UNKNOWN999"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("EXPLAIN001")
    );
    for name in ["inspect", "check", "plan", "generate", "build", "package"] {
        let output = workspace.run(&[name, "--not-a-real-option"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.starts_with(b"{"));
    }
}
