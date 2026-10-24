use clap::{Arg, ArgAction, ArgMatches, Command};
use madb_app::{
    ErrorKind, Id, LogicalPath, QueryError,
    query::{QueryOutcome, QueryRequest},
};
use std::io::{self, Write};
use std::path::Path;

pub fn run() -> u8 {
    let matches = cli().get_matches();
    let (command, args) = matches.subcommand().expect("subcommand required");
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
            text.push_str("Required task: metadata. Execution entry: madb build --task metadata --frozen (executor/reader pending C04/C05; not run).\n");
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
                    text.push_str("PLAN003: provide --task-file for static planning; automatic backend planning is pending C04/C05.\n");
                    return write_result(&mut out, &text, 2);
                }
            } else {
                text.push_str(
                    "Contract checks passed. No boundary route is implemented or certified.\n",
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
        .after_help("Read-only snapshot queries are available. No boundary route is implemented or certified.")
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
    command
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
