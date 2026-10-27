#[cfg(target_os = "macos")]
use madb_contracts::{DocumentKind, SchemaRegistry};
#[cfg(target_os = "macos")]
use std::fs;
use std::process::{Command, Output};
#[cfg(target_os = "macos")]
fn root() -> std::path::PathBuf {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "madb-doctor-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir(&root).unwrap();
    root
}
fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_madb"))
        .args(args)
        .env_clear()
        .env("PATH", "/no-doctor-tools")
        .output()
        .unwrap()
}
#[cfg(target_os = "macos")]
fn json(output: Output, expected: i32) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value = SchemaRegistry::new()
        .unwrap()
        .decode(DocumentKind::DoctorResult, &output.stdout)
        .unwrap()
        .into_value();
    assert_eq!(value["exit_code"], expected);
    value
}
#[cfg(target_os = "macos")]
#[test]
fn all_argument_errors_in_machine_dispatch_are_json() {
    for args in [
        vec!["doctor", "--json"],
        vec!["doctor", "--json", "--verify", "wrong"],
        vec!["doctor", "--json", "--tool-root", "/tmp"],
        vec!["doctor", "--json", "--root"],
    ] {
        let value = json(
            run(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
            2,
        );
        assert!(value["request"].is_null());
    }
    let root = root();
    let base = vec![
        "doctor".into(),
        "--json".into(),
        "--root".into(),
        root.to_string_lossy().into_owned(),
        "--project".into(),
        "project.json".into(),
        "--request".into(),
        "read.json".into(),
        "--snapshot".into(),
        "ir.json".into(),
    ];
    for (index, value) in [
        (3, "/private/tmp/bad\n".into()),
        (3, "/private/tmp/bad\u{7f}".into()),
        (3, format!("/{}", "x".repeat(5000))),
        (5, "../bad".into()),
        (7, "/absolute.json".into()),
        (9, "http://bad".into()),
    ] {
        let mut args = base.clone();
        args[index] = value;
        json(run(&args), 2);
    }
    let mut long = base.clone();
    long.push(format!("--{}", "x".repeat(9000)));
    json(run(&long), 2);
    assert!(fs::read_dir(&root).unwrap().next().is_none());
    fs::remove_dir(root).unwrap();
}
#[cfg(target_os = "macos")]
#[test]
fn missing_configuration_is_read_only_and_blocks_all_build_checks() {
    let root = root();
    let args = [
        "doctor",
        "--json",
        "--root",
        root.to_str().unwrap(),
        "--project",
        "project.json",
        "--request",
        "read.json",
        "--snapshot",
        "ir.json",
    ]
    .map(str::to_owned);
    let value = json(run(&args), 2);
    assert_eq!(value["request"]["verify"], "build");
    assert_eq!(value["checks"].as_array().unwrap().len(), 23);
    let config = value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "context.configuration")
        .unwrap();
    assert_eq!(config["error_kind"], "invalid-input");
    assert!(fs::read_dir(&root).unwrap().next().is_none());
    fs::remove_dir(root).unwrap();
}
#[test]
fn explicit_help_and_version_keep_their_text_exception() {
    for flag in ["--help", "--version"] {
        let output = run(&["doctor".into(), "--json".into(), flag.into()]);
        assert!(output.status.success());
        assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_err());
    }
}
