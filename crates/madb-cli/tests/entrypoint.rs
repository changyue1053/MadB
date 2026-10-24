use std::process::Command;

#[test]
fn help_states_the_actual_scope() {
    let output = Command::new(env!("CARGO_BIN_EXE_madb"))
        .arg("--help")
        .output()
        .expect("run the built CLI");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(help.contains("--version"));
    assert!(help.contains("Route certification requires independent consumer evidence."));
}

#[test]
fn version_matches_the_built_package() {
    let output = Command::new(env!("CARGO_BIN_EXE_madb"))
        .arg("--version")
        .output()
        .expect("run the built CLI");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 version"),
        format!("madb {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unavailable_operations_fail_instead_of_reporting_success() {
    for operation in ["init", "verify"] {
        let output = Command::new(env!("CARGO_BIN_EXE_madb"))
            .arg(operation)
            .output()
            .expect("run the built CLI");
        assert_eq!(output.status.code(), Some(2), "{operation}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(operation));
    }
}

#[test]
fn execution_commands_require_explicit_locked_recipes() {
    for operation in ["generate", "build", "package"] {
        let output = Command::new(env!("CARGO_BIN_EXE_madb"))
            .arg(operation)
            .output()
            .expect("run CLI");
        assert_eq!(output.status.code(), Some(2));
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(text.contains("--execution"), "{text}");
        assert!(text.contains("--frozen"), "{text}");
    }
}

#[test]
fn execution_budget_is_bounded_and_documented() {
    for command in ["generate", "build", "package"] {
        let help = Command::new(env!("CARGO_BIN_EXE_madb"))
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(help.status.success());
        let help = String::from_utf8(help.stdout).unwrap();
        assert!(help.contains("--process-timeout-seconds"));
        assert!(help.contains("[default: 30]"));
        for seconds in ["0", "3601", "1.5", "not-a-number"] {
            let output = Command::new(env!("CARGO_BIN_EXE_madb"))
                .args([
                    command,
                    "--execution",
                    "unused.json",
                    "--frozen",
                    "--process-timeout-seconds",
                    seconds,
                ])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(String::from_utf8_lossy(&output.stderr).contains("--process-timeout-seconds"));
        }
    }
}

#[test]
fn dependency_discovery_rejects_an_unsupported_budget_before_reading_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_madb"))
        .args([
            "build",
            "--task",
            "dependencies",
            "--execution",
            "unused.json",
            "--frozen",
            "--process-timeout-seconds",
            "900",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("process-timeout-seconds"));
    assert!(message.contains("dedicated discovery interface"));
    assert!(message.contains("does not accept a non-default process budget"));
}

#[test]
fn termination_grace_is_bounded_and_documented() {
    for command in ["generate", "build", "package"] {
        let help = Command::new(env!("CARGO_BIN_EXE_madb"))
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(help.status.success());
        let help = String::from_utf8(help.stdout).unwrap();
        assert!(help.contains("--process-termination-grace-ms"));
        assert!(help.contains("[default: 250]"));
        for milliseconds in ["-1", "30001", "1.5", "not-a-number"] {
            let output = Command::new(env!("CARGO_BIN_EXE_madb"))
                .args([
                    command,
                    "--execution",
                    "unused.json",
                    "--frozen",
                    &format!("--process-termination-grace-ms={milliseconds}"),
                ])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("--process-termination-grace-ms")
            );
        }
    }
}

#[test]
fn dependency_discovery_rejects_custom_grace_before_reading_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_madb"))
        .args([
            "build",
            "--task",
            "dependencies",
            "--execution",
            "unused.json",
            "--frozen",
            "--process-termination-grace-ms",
            "6000",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("process-termination-grace-ms"));
    assert!(message.contains("dedicated discovery interface"));
    assert!(message.contains("does not accept a non-default process termination grace"));
}
