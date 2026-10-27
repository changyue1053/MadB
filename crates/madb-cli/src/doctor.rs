//! Argument parsing and presentation only. Doctor never dispatches execution.
use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind as ClapErrorKind};
use madb_app::{ErrorKind, LogicalPath, QueryError, Result, use_cases::initialize::VerifyLevel};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};

pub struct DoctorOptions {
    pub root: PathBuf,
    pub project: LogicalPath,
    pub read_request: LogicalPath,
    pub snapshot: LogicalPath,
    pub verify: VerifyLevel,
}
pub fn command() -> Command {
    Command::new("doctor")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Observe frozen project and installation preconditions without executing or writing")
        .arg(
            Arg::new("root")
                .long("root")
                .default_value(".")
                .value_name("DIRECTORY"),
        )
        .arg(
            Arg::new("project")
                .long("project")
                .required(true)
                .value_name("PATH"),
        )
        .arg(
            Arg::new("request")
                .long("request")
                .required(true)
                .value_name("PATH"),
        )
        .arg(
            Arg::new("snapshot")
                .long("snapshot")
                .required(true)
                .value_name("PATH"),
        )
        .arg(
            Arg::new("verify")
                .long("verify")
                .value_parser(["structure", "build", "run"])
                .default_value("build"),
        )
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
}
fn options(args: &ArgMatches) -> Result<DoctorOptions> {
    let path = |name| {
        LogicalPath::new(args.get_one::<String>(name).expect("required path"))
            .map_err(|e| QueryError::new(ErrorKind::InvalidInput, "FILE001", name, e.to_string()))
    };
    Ok(DoctorOptions {
        root: args.get_one::<String>("root").expect("default root").into(),
        project: path("project")?,
        read_request: path("request")?,
        snapshot: path("snapshot")?,
        verify: match args
            .get_one::<String>("verify")
            .expect("default verify")
            .as_str()
        {
            "structure" => VerifyLevel::Structure,
            "run" => VerifyLevel::Run,
            _ => VerifyLevel::Build,
        },
    })
}
pub fn run(arguments: Vec<OsString>, command: Command) -> u8 {
    let json = arguments
        .iter()
        .skip(2)
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let parsed = command.try_get_matches_from(arguments);
    if let Err(error) = &parsed
        && matches!(
            error.kind(),
            ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
        )
    {
        return if error.print().is_ok() { 0 } else { 3 };
    }
    let request = parsed
        .map_err(|error| {
            QueryError::new(
                ErrorKind::InvalidInput,
                "PLAN001",
                "doctor arguments",
                error.to_string(),
            )
        })
        .and_then(|matches| options(matches.subcommand().expect("doctor subcommand").1));
    let (report, bytes) = crate::bootstrap::DoctorService::run(request);
    if json {
        return write(&mut io::stdout().lock(), &bytes, report.exit_code);
    }
    let mut text = String::from(
        "Doctor: preflight only; no generation, build, run or repair was performed.\n",
    );
    for check in &report.checks {
        text.push_str(&format!("{}: {:?}\n", check.id, check.status));
        for diagnostic in &check.diagnostics {
            text.push_str(&format!("  {}: {}\n", diagnostic.code, diagnostic.message));
        }
    }
    for diagnostic in &report.diagnostics {
        text.push_str(&format!("{}: {}\n", diagnostic.code, diagnostic.message));
    }
    for recovery in &report.recovery {
        text.push_str(&format!("{}\n", recovery.message));
    }
    write(&mut io::stdout().lock(), text.as_bytes(), report.exit_code)
}
fn write(output: &mut impl Write, bytes: &[u8], status: u8) -> u8 {
    if output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .is_err()
    {
        3
    } else {
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_is_build_and_unknown_execution_options_are_rejected() {
        let args = [
            "doctor",
            "--project",
            "project.json",
            "--request",
            "read.json",
            "--snapshot",
            "ir.json",
        ];
        assert_eq!(
            options(&command().try_get_matches_from(args).unwrap())
                .unwrap()
                .verify,
            VerifyLevel::Build
        );
        for forbidden in [
            "--fix",
            "--tool-root",
            "--command",
            "--template",
            "--profile",
        ] {
            assert!(
                command()
                    .try_get_matches_from(args.into_iter().chain([forbidden, "value"]))
                    .is_err()
            );
        }
    }
    #[test]
    fn failed_stdout_is_host_failure() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(write(&mut Broken, b"{}\n", 0), 3);
    }
}
