//! Initialization argument handling and presentation. The composition root owns
//! adapters; no template path, URL, version override, or shell command is exposed.
use crate::bootstrap::InitializationService;
use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind as ClapErrorKind};
use madb_app::use_cases::initialize::{DisplayPath, InitReport, VerifyLevel};
use madb_app::{ErrorKind, Id, QueryError, Result};
use madb_plan::project::ParameterValue;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct InitOptions {
    pub template: Option<String>,
    pub profile: Option<Id>,
    pub parameters: BTreeMap<Id, ParameterValue>,
    pub targets: Option<Vec<Id>>,
    pub verify: VerifyLevel,
    pub open: Option<Id>,
    pub dry_run: bool,
    pub offline: bool,
    pub output: PathBuf,
}

pub fn command() -> Command {
    Command::new("init")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Initialize a new project from an installed, pinned template")
        .after_help("The installed 0.1.1 template supports structure and dry-run, plus default build and run on the fixed macOS ARM64 installation with a source-bound tool. Explicit 0.1.0 retains structure/dry-run only. IDE opening and other execution targets are unsupported.")
        .arg(
            Arg::new("template")
                .long("template")
                .value_name("ID[@VERSION]"),
        )
        .arg(
            Arg::new("out")
                .long("out")
                .required(true)
                .value_name("DIRECTORY"),
        )
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_name("TEMPLATE_PROFILE"),
        )
        .arg(Arg::new("targets").long("targets").value_name("FAMILY,..."))
        .arg(Arg::new("name").long("name").value_name("NAME"))
        .arg(
            Arg::new("namespace")
                .long("namespace")
                .value_name("NAMESPACE"),
        )
        .arg(
            Arg::new("verify")
                .long("verify")
                .default_value("build")
                .value_parser(["structure", "build", "run"]),
        )
        .arg(Arg::new("open").long("open").value_name("IDE_ID"))
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("offline")
                .long("offline")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
}

pub fn run(arguments: Vec<OsString>, command: Command) -> u8 {
    let json = arguments
        .iter()
        .skip(2)
        .take_while(|a| *a != "--")
        .any(|a| a == "--json");
    let matches = command.try_get_matches_from(&arguments);
    if let Err(error) = &matches
        && matches!(
            error.kind(),
            ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
        )
    {
        return if error.print().is_ok() { 0 } else { 3 };
    }
    let options = matches
        .map_err(|error| invalid("arguments", error.to_string()))
        .and_then(|matches| {
            let (_, args) = matches
                .subcommand()
                .expect("init dispatch selected a subcommand");
            options(args)
        });
    let service = match if options.as_ref().is_ok_and(|options| {
        options.template.as_deref() == Some("org.madb.template/rust-c-sdk@0.2.0")
    }) {
        InitializationService::new_project()
    } else {
        InitializationService::new()
    } {
        Ok(service) => service,
        Err(error) => return present_error(error),
    };
    let report = service.initialize(options);
    present(&service, &report, json)
}

fn options(args: &ArgMatches) -> Result<InitOptions> {
    let mut parameters = BTreeMap::new();
    for name in ["name", "namespace"] {
        if let Some(value) = args.get_one::<String>(name) {
            parameters.insert(
                Id::new(name).expect("static parameter id"),
                ParameterValue::String(value.clone()),
            );
        }
    }
    let targets = args
        .get_one::<String>("targets")
        .map(|value| {
            value
                .split(',')
                .map(|family| id(family, "targets"))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    Ok(InitOptions {
        template: args.get_one::<String>("template").cloned(),
        profile: args
            .get_one::<String>("profile")
            .map(|v| id(v, "profile"))
            .transpose()?,
        parameters,
        targets,
        verify: match args.get_one::<String>("verify").map(String::as_str) {
            Some("structure") => VerifyLevel::Structure,
            Some("build") => VerifyLevel::Build,
            Some("run") => VerifyLevel::Run,
            _ => return Err(invalid("verify", "unknown verification level")),
        },
        open: args
            .get_one::<String>("open")
            .map(|v| id(v, "open"))
            .transpose()?,
        dry_run: args.get_flag("dry-run"),
        offline: args.get_flag("offline"),
        output: PathBuf::from(args.get_one::<String>("out").expect("required output")),
    })
}
fn id(value: &str, subject: &str) -> Result<Id> {
    Id::new(value).map_err(|error| invalid(subject, error.to_string()))
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}

fn present(service: &InitializationService, report: &InitReport, json: bool) -> u8 {
    if json {
        return match service.encode(report) {
            Ok(bytes) => write(&mut io::stdout().lock(), &bytes, report.exit_code),
            Err(error) => present_error(error),
        };
    }
    let mut text = String::new();
    for diagnostic in &report.diagnostics {
        text.push_str(&format!("{}: {}\n", diagnostic.code, diagnostic.message));
    }
    if report.exit_code == 0 {
        text.push_str(if report.request.as_ref().is_some_and(|r| r.dry_run) {
            "Dry-run plan saved; no project was created.\n"
        } else {
            "Project created and the requested checks completed.\n"
        });
    }
    if let Some(DisplayPath::Absolute(root)) = &report.evidence_root {
        text.push_str(&format!("Result: {}/init-result.json\n", root.as_str()));
    }
    if report.exit_code == 0 {
        write(&mut io::stdout().lock(), text.as_bytes(), 0)
    } else {
        write(&mut io::stderr().lock(), text.as_bytes(), report.exit_code)
    }
}
fn present_error(error: QueryError) -> u8 {
    if writeln!(io::stderr().lock(), "{error}").is_err() {
        return 3;
    }
    match error.kind {
        ErrorKind::InvalidInput => 2,
        ErrorKind::CheckFailed => 1,
        ErrorKind::Conflict => 4,
        ErrorKind::Host => 3,
    }
}

fn write(output: &mut impl Write, bytes: &[u8], exit: u8) -> u8 {
    if output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .is_err()
    {
        3
    } else {
        exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parsed(args: &[&str]) -> Result<InitOptions> {
        let parsed = command()
            .try_get_matches_from(args)
            .map_err(|e| invalid("arguments", e.to_string()))?;
        options(&parsed)
    }
    #[test]
    fn approved_arguments_keep_defaults_and_values_separate() {
        let r = parsed(&[
            "init",
            "--out",
            "中文 工程",
            "--name",
            "name",
            "--targets",
            "macos",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(r.verify, VerifyLevel::Build);
        assert!(r.dry_run);
        assert_eq!(r.output, PathBuf::from("中文 工程"));
        assert_eq!(r.targets.unwrap(), vec![Id::new("macos").unwrap()]);
        assert_eq!(r.parameters.len(), 1);
    }
    #[test]
    fn unknown_inputs_or_empty_target_segments_are_rejected() {
        for args in [
            vec!["init", "--out", "x", "--package", "x.tar"],
            vec!["init", "--out", "x", "--tool-version", "0.1.0"],
            vec!["init", "--out", "x", "--targets", "macos,"],
            vec!["init", "--out", "x", "--verify", "all"],
        ] {
            assert!(parsed(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn broken_stdout_is_host_failure_not_success() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(write(&mut Broken, b"result", 0), 3);
        assert_eq!(write(&mut Broken, b"failure", 2), 3);
    }
}
