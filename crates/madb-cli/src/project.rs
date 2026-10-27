//! Parse the two finite project operations; all execution is composed once.
use clap::{Arg, ArgAction, Command, error::ErrorKind as ClapErrorKind};
use madb_app::{ErrorKind, QueryError, Result, use_cases::project_build::*};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};
pub struct ProjectOptions {
    pub root: PathBuf,
}
pub fn command() -> Command {
    let operation = |name| {
        Command::new(name)
            .arg(
                Arg::new("root")
                    .long("root")
                    .required(true)
                    .value_name("DIRECTORY"),
            )
            .arg(
                Arg::new("frozen")
                    .long("frozen")
                    .required(true)
                    .action(ArgAction::SetTrue),
            )
            .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
    };
    Command::new("project")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Build the finite frozen Rust-to-C author project or restore its pinned local tool")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(operation("build"))
        .subcommand(operation("install-tool"))
}
pub fn run(arguments: Vec<OsString>, command: Command) -> u8 {
    let operation = match arguments.get(2).and_then(|v| v.to_str()) {
        Some("build") => Some(Operation::Build),
        Some("install-tool") => Some(Operation::InstallTool),
        _ => None,
    };
    let json = arguments
        .iter()
        .skip(3)
        .take_while(|v| *v != "--")
        .any(|v| v == "--json");
    let parsed = command.try_get_matches_from(arguments);
    if let Err(error) = &parsed
        && (operation.is_none()
            || matches!(
                error.kind(),
                ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
            ))
    {
        let code = if matches!(
            error.kind(),
            ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
        ) {
            0
        } else {
            2
        };
        return if error.print().is_ok() { code } else { 3 };
    }
    let Some(operation) = operation else {
        return 2;
    };
    let options: Result<_> = parsed
        .map_err(|e| {
            QueryError::new(
                ErrorKind::InvalidInput,
                "PLAN001",
                "project arguments",
                e.to_string(),
            )
        })
        .map(|matches| {
            let args = matches
                .subcommand()
                .expect("project")
                .1
                .subcommand()
                .expect("operation")
                .1;
            ProjectOptions {
                root: args
                    .get_one::<String>("root")
                    .expect("required root")
                    .into(),
            }
        });
    let (mut report, bytes) = match crate::bootstrap::ProjectService::run(operation, options) {
        Ok(v) => v,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            return 3;
        }
    };
    let output = if json {
        bytes
    } else {
        let mut text = format!(
            "Project {}: {}\n",
            operation.as_str(),
            report.termination.as_str()
        );
        for stage in &report.stages {
            text.push_str(&format!(
                "{}: {}\n",
                stage.id.as_str(),
                stage.status.as_str()
            ));
        }
        for diagnostic in &report.diagnostics {
            text.push_str(&format!("{}: {}\n", diagnostic.code, diagnostic.message));
        }
        if let Some(root) = &report.evidence_root {
            text.push_str(&format!("Evidence: {}\n", root.as_str()));
        }
        text.into_bytes()
    };
    if io::stdout()
        .lock()
        .write_all(&output)
        .and_then(|()| io::stdout().lock().flush())
        .is_err()
    {
        report.evidence_failed(QueryError::new(ErrorKind::Host,"HOST003","stdout","result output could not be written; actual publication/installation remains in saved evidence"));
        let _ = writeln!(
            io::stderr().lock(),
            "HOST003: result output failed; published={:?}, installed={}",
            report.published,
            report.installed_tool.is_some()
        );
        return 3;
    }
    report.exit_code
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_operations_require_frozen_and_have_no_executable_override() {
        for operation in ["build", "install-tool"] {
            assert!(
                command()
                    .try_get_matches_from(["project", operation, "--root", "/tmp/p", "--frozen"])
                    .is_ok()
            );
            assert!(
                command()
                    .try_get_matches_from(["project", operation, "--root", "/tmp/p"])
                    .is_err()
            );
            for option in [
                "--command",
                "--tool-root",
                "--profile",
                "--repair",
                "--update",
            ] {
                assert!(
                    command()
                        .try_get_matches_from([
                            "project", operation, "--root", "/tmp/p", "--frozen", option, "value"
                        ])
                        .is_err()
                );
            }
        }
    }
    #[test]
    fn recognized_bad_parameters_have_a_valid_failed_machine_report() {
        for operation in [Operation::Build, Operation::InstallTool] {
            let (report, bytes) = crate::bootstrap::ProjectService::run(
                operation,
                Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "PLAN001",
                    "arguments",
                    "invalid option",
                )),
            )
            .unwrap();
            assert_eq!(report.exit_code, 2);
            madb_contracts::project_build::ProjectBuildAdapter::new()
                .unwrap()
                .decode_report(&bytes)
                .unwrap();
        }
    }
}
