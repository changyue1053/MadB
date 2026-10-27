//! Real CLI filesystem tests for the explicitly approved formal producer.
//! Structure and dry-run do not claim native generation, build, run, or IDE use.
#[cfg(target_os = "macos")]
use madb_contracts::{DocumentKind, SchemaRegistry};
#[cfg(target_os = "macos")]
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "madb-init-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_madb"))
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/no-init-tools")
            .output()
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[cfg(target_os = "macos")]
fn report(output: Output, expected: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.ends_with(b"\n"));
    let value = SchemaRegistry::new()
        .unwrap()
        .decode(DocumentKind::InitResult, &output.stdout)
        .unwrap()
        .into_value();
    assert_eq!(value["exit_code"], expected);

    assert_eq!(value["tool"]["version"], env!("CARGO_PKG_VERSION"));
    value
}
#[cfg(target_os = "macos")]
#[test]
fn formal_structure_creates_the_complete_plan_with_saved_actual_result() {
    let w = Workspace::new();
    let actual_tool = madb_app::Digest::of_bytes(&fs::read(env!("CARGO_BIN_EXE_madb")).unwrap());
    for (name, offline) in [("中文 工程", false), ("离线 工程", true)] {
        let mut args = vec![
            "init",
            "--template",
            "org.madb.template/rust-c-sdk",
            "--out",
            name,
            "--name",
            "hello_c",
            "--verify",
            "structure",
            "--json",
        ];
        if offline {
            args.push("--offline");
        }
        let output = w.run(&args);
        let bytes = output.stdout.clone();
        let value = report(output, 0);
        assert_eq!(value["tool"]["sha256"].as_str(), Some(actual_tool.as_str()));
        assert_eq!(value["creation"], "created");
        assert_eq!(value["termination"], "completed");
        assert_eq!(value["checks"].as_array().unwrap().len(), 1);
        assert_eq!(value["checks"][0]["id"], "structure");
        assert_eq!(value["checks"][0]["evidence_level"], "static");
        for entry in value["plan"]["files"].as_array().unwrap() {
            let path = w.0.join(name).join(entry["path"].as_str().unwrap());
            if entry["ownership"] == "local-state" {
                assert!(!path.exists());
                continue;
            }
            let actual = fs::read(path).unwrap();
            assert_eq!(
                madb_app::Digest::of_bytes(&actual).as_str(),
                entry["sha256"].as_str().unwrap()
            );
            assert_eq!(actual.len() as u64, entry["size"].as_u64().unwrap());
        }
        let root = evidence_root(&value);
        assert_eq!(fs::read(root.join("init-result.json")).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(target_os = "macos")]
#[test]
fn dry_run_saves_a_plan_and_complete_original_inputs_without_user_project() {
    let w = Workspace::new();
    let output = w.run(&[
        "init",
        "--out",
        "dry_project",
        "--dry-run",
        "--offline",
        "--json",
    ]);
    let bytes = output.stdout.clone();
    let value = report(output, 0);
    assert_eq!(value["creation"], "not-created");
    assert!(value["checks"].as_array().unwrap().is_empty());
    assert!(value["plan"]["downloads"].as_array().unwrap().is_empty());
    assert_eq!(value["open"]["status"], "skipped");
    assert_eq!(fs::read_dir(&w.0).unwrap().count(), 0);
    let root = evidence_root(&value);
    assert_eq!(fs::read(root.join("init-result.json")).unwrap(), bytes);
    let archive = fs::read(root.join(value["template_package"]["path"].as_str().unwrap())).unwrap();
    assert_eq!(
        madb_app::Digest::of_bytes(&archive).as_str(),
        value["template_package"]["sha256"].as_str().unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}
#[cfg(target_os = "macos")]
#[test]
fn unsupported_build_run_modes_fail_before_creation_and_keep_the_frozen_digest() {
    let w = Workspace::new();
    for level in ["build", "run"] {
        let value = report(
            w.run(&["init", "--out", "future_build", "--verify", level, "--json"]),
            2,
        );
        assert_eq!(value["creation"], "not-created");
        assert_eq!(value["termination"], "failed");
        assert!(value["request_digest"].is_string());
        assert!(value["checks"].as_array().unwrap().is_empty());
        assert_eq!(fs::read_dir(&w.0).unwrap().count(), 0);
        let root = evidence_root(&value);
        assert!(root.join("init-result.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(target_os = "macos")]
fn evidence_root(report: &Value) -> PathBuf {
    let root = PathBuf::from(report["evidence_root"]["path"].as_str().unwrap());
    assert!(root.is_absolute());
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("madb-init-evidence-")
    );
    root
}
#[cfg(target_os = "macos")]
#[test]
fn machine_argument_errors_reject_unapproved_sources_and_version_overrides() {
    let w = Workspace::new();
    for args in [
        vec!["init", "--json"],
        vec![
            "init",
            "--out",
            "out",
            "--package",
            "attacker.tar",
            "--json",
        ],
        vec![
            "init",
            "--out",
            "out",
            "--template-url",
            "https://example.invalid/template",
            "--json",
        ],
        vec!["init", "--out", "out", "--tool-version", "0.1.0", "--json"],
        vec!["init", "--out", "out", "--targets", "macos,", "--json"],
        vec!["init", "--out", "out", "--verify", "unknown", "--json"],
        vec!["init", "--out", "out", "--out", "again", "--json"],
    ] {
        let value = report(w.run(&args), 2);
        assert_eq!(value["diagnostics"][0]["code"], "PLAN001");
        assert!(value["request_digest"].is_null() && value["evidence_root"].is_null());
        assert_eq!(value["creation"], "not-created");
        assert_eq!(fs::read_dir(&w.0).unwrap().count(), 0);
    }
}
#[cfg(target_os = "macos")]
#[test]
fn existing_user_content_is_preserved_by_real_conflict() {
    let w = Workspace::new();
    fs::create_dir(w.0.join("existing")).unwrap();
    fs::write(w.0.join("existing/keep.txt"), b"user bytes\n").unwrap();
    let value = report(
        w.run(&[
            "init",
            "--out",
            "existing",
            "--verify",
            "structure",
            "--json",
        ]),
        4,
    );
    assert_eq!(value["creation"], "not-created");
    assert!(value["request_digest"].is_string());
    fs::remove_dir_all(evidence_root(&value)).unwrap();
    assert_eq!(
        fs::read(w.0.join("existing/keep.txt")).unwrap(),
        b"user bytes\n"
    );
    assert_eq!(fs::read_dir(w.0.join("existing")).unwrap().count(), 1);
}
#[test]
fn help_exposes_only_the_adopted_command_options() {
    let w = Workspace::new();
    let output = w.run(&["init", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8(output.stdout).unwrap();
    for flag in [
        "--template",
        "--out",
        "--profile",
        "--targets",
        "--name",
        "--namespace",
        "--verify",
        "--open",
        "--dry-run",
        "--offline",
        "--json",
    ] {
        assert!(help.contains(flag), "{flag}");
    }
    assert!(!help.contains("--package") && !help.contains("--tool-version"));
    assert_eq!(fs::read_dir(&w.0).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
#[test]
fn missing_parent_is_a_host_failure_with_a_frozen_request() {
    let w = Workspace::new();
    let value = report(
        w.run(&[
            "init",
            "--out",
            "absent/child",
            "--verify",
            "structure",
            "--json",
        ]),
        3,
    );
    assert_eq!(value["creation"], "not-created");
    assert!(value["request_digest"].is_string());
    assert!(!w.0.join("absent").exists());
    fs::remove_dir_all(evidence_root(&value)).unwrap();
}
#[cfg(target_os = "macos")]
#[test]
fn catalog_does_not_accept_url_path_or_unlocked_version_selectors() {
    let w = Workspace::new();
    for selector in [
        "https://example.invalid/template.tar",
        "./template.tar",
        "org.madb.template/rust-c-sdk@latest",
    ] {
        let value = report(
            w.run(&[
                "init",
                "--template",
                selector,
                "--out",
                "rejected",
                "--verify",
                "structure",
                "--json",
            ]),
            2,
        );
        assert!(value["request_digest"].is_null());
        assert!(value["evidence_root"].is_null());
        assert_eq!(value["creation"], "not-created");
        assert_eq!(fs::read_dir(&w.0).unwrap().count(), 0);
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn unsupported_host_does_not_invent_tool_identity_or_create_a_project() {
    let workspace = Workspace::new();
    for dry_run in [false, true] {
        let mut args = vec![
            "init",
            "--out",
            "untouched",
            "--verify",
            "structure",
            "--json",
        ];
        if dry_run {
            args.push("--dry-run");
        }
        let output = workspace.run(&args);
        assert_eq!(output.status.code(), Some(3));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("HOST004"));
        assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
    }
}
