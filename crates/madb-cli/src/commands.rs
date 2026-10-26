use clap::{Arg, ArgAction, ArgMatches, Command};
use madb_app::{
    ErrorKind, Id, LogicalPath, QueryError,
    query::{QueryOutcome, QueryRequest},
};
use std::io::{self, Write};
use std::path::Path;

pub fn run() -> u8 {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|value| value == "explain") {
        return super::explain::run(arguments, cli());
    }
    let matches = cli().get_matches();
    let (command, args) = matches.subcommand().expect("subcommand required");
    if matches!(command, "generate" | "build" | "package") {
        return execute(command, args);
    }
    let result = request(args).and_then(|request| {
        super::bootstrap::query(
            Path::new(args.get_one::<String>("root").expect("default root")),
            &request,
        )
    });
    let mut out = io::BufWriter::new(io::stdout().lock());
    let (code, text) = match result {
        Ok(QueryOutcome::MetadataRequired { findings }) => {
            let mut text = String::from("MetadataRequired\n");
            for f in findings {
                text.push_str(&format!("{} at {}: {}\n", f.code, f.subject, f.message));
            }
            text.push_str("Required task: metadata. Execution entry: madb build --task metadata --execution <metadata.execution.json> --frozen (explicit locked reader recipe required; not run).\n");
            (2, text)
        }
        Ok(QueryOutcome::Checked {
            contract,
            graph,
            observed_files,
        }) => {
            let mut text = format!(
                "TrackedInputsMatch: {} files checked; dependency closure not certified.\nBoundary: {}\nProfile: {}\nSource: {}\n",
                observed_files,
                contract.boundary_id,
                contract.selector.profile_id,
                contract.source_digest.as_str()
            );
            if command == "inspect" {
                text.push_str(&format!(
                    "Structure: {}\nPolicy: {}\n",
                    contract.structure_digest.as_str(),
                    contract.policy_digest.as_str()
                ));
                for symbol in &contract.symbols {
                    text.push_str(&format!(
                        "{} at {}:{}:{} ({:?})\n",
                        symbol.public_id.as_str(),
                        symbol.location.path.as_str(),
                        symbol.location.line,
                        symbol.location.column,
                        symbol.precision
                    ));
                }
            }
            if command == "inspect" {
                for policy in &contract.policies {
                    for (property, value) in &policy.values {
                        let origin = &value.origin;
                        text.push_str(&format!(
                            "Policy {} {:?}.{} = {:?} <- {}:{}:{} ({})\n",
                            policy.symbol_id.as_str(),
                            policy.slot,
                            property,
                            value.value,
                            origin.location.path.as_str(),
                            origin.location.line,
                            origin.location.column,
                            origin.authority.as_str()
                        ));
                    }
                }
            }
            if command == "plan" {
                if let Some(graph) = graph {
                    text.push_str("Static task plan (no execution or artifact validation):\n");
                    for task in graph.tasks() {
                        text.push_str(&format!(
                            "{} [{:?}] <- {}\n",
                            task.id,
                            task.phase,
                            task.dependencies
                                .iter()
                                .map(Id::as_str)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                } else {
                    text.push_str("PLAN003: provide --task-file for static planning; automatic backend planning belongs to language integration (C05 and later).\n");
                    return write_result(&mut out, &text, 2);
                }
            } else {
                text.push_str(
                    "Contract checks passed. This command does not execute or certify SDK consumers.\n",
                );
            }
            (0, text)
        }
        Err(error) => {
            let code = match error.kind {
                ErrorKind::CheckFailed => 1,
                ErrorKind::InvalidInput => 2,
                ErrorKind::Host => 3,
                ErrorKind::Conflict => 4,
            };
            if writeln!(io::stderr().lock(), "{error}").is_err() {
                return 3;
            }
            return code;
        }
    };
    write_result(&mut out, &text, code)
}

fn write_result(out: &mut impl Write, text: &str, code: u8) -> u8 {
    if out
        .write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .is_err()
    {
        3
    } else {
        code
    }
}

fn cli() -> Command {
    let mut command = Command::new("madb").version(env!("CARGO_PKG_VERSION"))
        .about("Cross-language SDK engineering tools")
        .after_help("Read-only snapshot queries and explicit frozen execution recipes are available. Route certification requires independent consumer evidence.")
        .subcommand_required(true).arg_required_else_help(true);
    for (name, help) in [
        (
            "inspect",
            "Inspect a recorded contract and its declared input bytes",
        ),
        (
            "check",
            "Check recorded structure, policies and locked references",
        ),
        (
            "plan",
            "Validate and order explicit task documents without executing them",
        ),
    ] {
        let mut sub = Command::new(name)
            .about(help)
            .arg(
                Arg::new("root")
                    .long("root")
                    .default_value(".")
                    .help("Workspace directory; all file arguments are relative to it"),
            )
            .arg(Arg::new("project").long("project").required(true))
            .arg(
                Arg::new("request")
                    .long("request")
                    .required(true)
                    .help("Existing madb.read-request/1 JSON"),
            )
            .arg(
                Arg::new("snapshot")
                    .long("snapshot")
                    .required(true)
                    .help("Existing madb.contract-ir/1 JSON"),
            );
        if name == "plan" {
            sub = sub
                .arg(
                    Arg::new("task-file")
                        .long("task-file")
                        .action(ArgAction::Append)
                        .help("Existing madb.build-task/1 JSON; repeat for dependencies"),
                )
                .arg(Arg::new("task").long("task").requires("task-file"))
                .arg(
                    Arg::new("active-task")
                        .long("active-task")
                        .action(ArgAction::Append)
                        .requires("task-file"),
                );
        }
        command = command.subcommand(sub);
    }
    for (name, help) in [
        (
            "generate",
            "Execute a locked generator and commit a source batch, or check for drift",
        ),
        (
            "build",
            "Execute a fixed build graph or an explicit metadata preparation recipe",
        ),
        (
            "package",
            "Prepare and commit a candidate from a fixed source/build pair",
        ),
    ] {
        let mut sub = Command::new(name)
            .about(help)
            .arg(Arg::new("root").long("root").default_value("."))
            .arg(Arg::new("tool-root").long("tool-root").help("Root of the locked installed tool images; defaults to --root"))
            .arg(Arg::new("execution").long("execution").required(true)
                .help("Existing madb.execution/1 document, relative to --root"))
            .arg(Arg::new("process-timeout-seconds").long("process-timeout-seconds")
                .value_name("SECONDS").value_parser(clap::value_parser!(u64).range(1..=3600))
                .default_value("30")
                .help("Authorized execution budget per child process (1..3600); does not change SDK cancellation or cleanup semantics"))
            .arg(Arg::new("process-termination-grace-ms").long("process-termination-grace-ms")
                .value_name("MILLISECONDS").value_parser(clap::value_parser!(u64).range(0..=30000))
                .default_value("250")
                .help("Authorized SIGTERM-to-SIGKILL grace per child process (0..30000); does not change SDK cancellation semantics"))
            .arg(Arg::new("frozen").long("frozen").action(ArgAction::SetTrue).required(true)
                .help("Use the exact locked local tools and inputs; never resolve or upgrade dependencies"));
        if name == "generate" {
            sub = sub.arg(
                Arg::new("check")
                    .long("check")
                    .action(ArgAction::SetTrue)
                    .help(
                        "Recompute in isolation and report drift without modifying managed outputs",
                    ),
            );
        }
        if name == "build" {
            sub = sub.arg(Arg::new("task").long("task")).arg(
                Arg::new("active-task")
                    .long("active-task")
                    .action(ArgAction::Append),
            );
        }
        command = command.subcommand(sub);
    }
    command.subcommand(super::explain::command())
}

fn execution_options(command: &str, args: &ArgMatches) -> super::bootstrap::ExecutionOptions {
    super::bootstrap::ExecutionOptions {
        command: command.to_owned(),
        root: args.get_one::<String>("root").expect("default root").into(),
        execution: args
            .get_one::<String>("execution")
            .expect("required execution")
            .to_owned(),
        tool_root: args.get_one::<String>("tool-root").map(Into::into),
        process_timeout: std::time::Duration::from_secs(
            *args
                .get_one::<u64>("process-timeout-seconds")
                .expect("default execution budget"),
        ),
        process_termination_grace: std::time::Duration::from_millis(
            *args
                .get_one::<u64>("process-termination-grace-ms")
                .expect("default termination grace"),
        ),
        check: args
            .try_get_one::<bool>("check")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false),
        task: args.try_get_one::<String>("task").ok().flatten().cloned(),
        active_tasks: args
            .try_get_many::<String>("active-task")
            .ok()
            .flatten()
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
    }
}

fn execute(command: &str, args: &ArgMatches) -> u8 {
    let options = execution_options(command, args);
    match super::bootstrap::execute(&options) {
        Ok(text) => write_result(&mut io::stdout().lock(), &text, 0),
        Err(error) => {
            let code = match error.kind {
                ErrorKind::CheckFailed => 1,
                ErrorKind::InvalidInput => 2,
                ErrorKind::Host => 3,
                ErrorKind::Conflict => 4,
            };
            if writeln!(io::stderr().lock(), "{error}").is_err() {
                3
            } else {
                code
            }
        }
    }
}

fn request(args: &ArgMatches) -> madb_app::Result<QueryRequest> {
    let path = |value: &str| {
        LogicalPath::new(value)
            .map_err(|e| QueryError::new(ErrorKind::InvalidInput, "FILE001", value, e.to_string()))
    };
    let id = |value: &str| {
        Id::new(value)
            .map_err(|e| QueryError::new(ErrorKind::InvalidInput, "PLAN001", value, e.to_string()))
    };
    let strings = |name: &str| {
        args.try_get_many::<String>(name)
            .ok()
            .flatten()
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
    };
    Ok(QueryRequest {
        project: path(args.get_one::<String>("project").expect("required"))?,
        request: path(args.get_one::<String>("request").expect("required"))?,
        snapshot: path(args.get_one::<String>("snapshot").expect("required"))?,
        tasks: strings("task-file")
            .into_iter()
            .map(path)
            .collect::<madb_app::Result<_>>()?,
        target: args
            .try_get_one::<String>("task")
            .ok()
            .flatten()
            .map(|v| id(v))
            .transpose()?,
        active_tasks: strings("active-task")
            .into_iter()
            .map(id)
            .collect::<madb_app::Result<_>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn execution_budget_defaults_and_explicit_limits_reach_options() {
        for command in ["generate", "build", "package"] {
            for seconds in [None, Some(1), Some(3600)] {
                let mut args = vec![
                    "madb".to_owned(),
                    command.to_owned(),
                    "--execution".to_owned(),
                    "execution.json".to_owned(),
                    "--frozen".to_owned(),
                ];
                if let Some(seconds) = seconds {
                    args.extend(["--process-timeout-seconds".to_owned(), seconds.to_string()]);
                }
                let parsed = cli().try_get_matches_from(args).unwrap();
                let (_, values) = parsed.subcommand().unwrap();
                assert_eq!(
                    execution_options(command, values).process_timeout,
                    Duration::from_secs(seconds.unwrap_or(30))
                );
            }
        }
        assert_eq!(
            madb_app::process::ProcessPolicy::default().timeout,
            Duration::from_secs(30)
        );
    }
}

#[cfg(test)]
mod termination_grace_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn termination_grace_defaults_and_explicit_limits_reach_options() {
        for command in ["generate", "build", "package"] {
            for milliseconds in [None, Some(0), Some(6000), Some(30000)] {
                let mut args = vec![
                    "madb".to_owned(),
                    command.to_owned(),
                    "--execution".to_owned(),
                    "execution.json".to_owned(),
                    "--frozen".to_owned(),
                ];
                if let Some(milliseconds) = milliseconds {
                    args.extend([
                        "--process-termination-grace-ms".to_owned(),
                        milliseconds.to_string(),
                    ]);
                }
                let parsed = cli().try_get_matches_from(args).unwrap();
                let (_, values) = parsed.subcommand().unwrap();
                assert_eq!(
                    execution_options(command, values).process_termination_grace,
                    Duration::from_millis(milliseconds.unwrap_or(250))
                );
            }
        }
        assert_eq!(
            madb_app::process::ProcessPolicy::default().termination_grace,
            Duration::from_millis(250)
        );
    }
}
