//! Presentation and argument handling only; application and adapters own data.
use crate::bootstrap::ExplanationService;
use clap::{Arg, ArgAction, Command, error::ErrorKind as ClapErrorKind};
use madb_app::{
    ErrorKind, Id, LogicalPath, QueryError, Result,
    explain::{ExplainRequest, ExplainSuccess},
};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::Path,
};

pub fn command() -> Command {
    Command::new("explain")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Explain an exact diagnostic code, optionally checking explicit source bytes")
        .arg(Arg::new("code").required(true).value_name("CODE"))
        .arg(Arg::new("context").long("context").value_name("PATH"))
        .arg(
            Arg::new("root")
                .long("root")
                .requires("context")
                .value_name("DIRECTORY"),
        )
        .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
}

pub fn run(arguments: Vec<OsString>, command: Command) -> u8 {
    let json_requested = arguments
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
    let service = match ExplanationService::new() {
        Ok(service) => service,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            return 3;
        }
    };
    let matches = match matches {
        Ok(matches) => matches,
        Err(error) => {
            let code = supplied_code(&arguments);
            return present(
                &service,
                code.as_ref(),
                Err(invalid(error.to_string())),
                json_requested,
            );
        }
    };
    let (_, args) = matches.subcommand().expect("explain subcommand");
    let raw = args.get_one::<String>("code").expect("required CODE");
    let code = match Id::new(raw) {
        Ok(code) => code,
        Err(error) => {
            return present(
                &service,
                None,
                Err(invalid(error.to_string())),
                args.get_flag("json"),
            );
        }
    };
    let context = args
        .get_one::<String>("context")
        .map(|value| {
            if value == "-" {
                return Err(invalid("context must be an explicit file path"));
            }
            LogicalPath::new(value).map_err(|e| invalid(e.to_string()))
        })
        .transpose();
    let result = context.and_then(|context| {
        service.query(
            Path::new(args.get_one::<String>("root").map_or(".", String::as_str)),
            &ExplainRequest {
                code: code.clone(),
                context,
            },
        )
    });
    present(&service, Some(&code), result, args.get_flag("json"))
}

fn invalid(message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "EXPLAIN002", "arguments", message)
}

fn exit_code(result: &Result<ExplainSuccess>) -> u8 {
    match result {
        Ok(_) => 0,
        Err(error) => match error.kind {
            ErrorKind::InvalidInput | ErrorKind::CheckFailed => 2,
            ErrorKind::Host => 3,
            ErrorKind::Conflict => 4,
        },
    }
}

fn present(
    service: &ExplanationService,
    code: Option<&Id>,
    result: Result<ExplainSuccess>,
    json: bool,
) -> u8 {
    let status = exit_code(&result);
    if json {
        return match service.encode(code, &result) {
            Ok(bytes) => write(&mut io::stdout().lock(), &bytes, status),
            Err(error) => {
                let _ = writeln!(io::stderr().lock(), "{error}");
                3
            }
        };
    }
    match result {
        Err(error) => write(
            &mut io::stderr().lock(),
            error.to_string().as_bytes(),
            status,
        ),
        Ok(result) => {
            let entry = &result.explanation;
            let mut text = format!("{}: {}\n", code.expect("successful code"), entry.meaning);
            for trigger in &entry.triggers {
                text.push_str(&format!("Trigger: {trigger}\n"));
            }
            for repair in &entry.repairs {
                text.push_str(&format!("Repair: {}", repair.instruction));
                if let Some(location) = &repair.location {
                    text.push_str(&format!(
                        " ({}:{}:{})",
                        location.path.as_str(),
                        location.line,
                        location.column
                    ));
                }
                text.push('\n');
            }
            if let Some(source) = &result.source {
                let location = &source.source.location;
                text.push_str(&format!(
                    "Source: {}:{}:{}; byte offset {}\n",
                    location.path.as_str(),
                    location.line,
                    location.column,
                    source.byte_offset
                ));
            }
            for limit in &entry.limits {
                text.push_str(&format!("Limit: {limit}\n"));
            }
            write(&mut io::stdout().lock(), text.as_bytes(), status)
        }
    }
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

// Recover only a syntactically valid positional CODE for parser-failure output.
// Do not mistake a known option's value for the query or infer a code from text.
fn supplied_code(arguments: &[OsString]) -> Option<Id> {
    let mut values = arguments.iter().skip(2);
    while let Some(argument) = values.next() {
        let value = argument.to_str()?;
        if value == "--" {
            return values.next()?.to_str().and_then(|v| Id::new(v).ok());
        }
        if matches!(value, "--root" | "--context") {
            values.next();
            continue;
        }
        if value.starts_with('-') {
            continue;
        }
        return Id::new(value).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_output_does_not_report_query_success() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(write(&mut Broken, b"result\n", 0), 3);
        assert_eq!(write(&mut Broken, b"failure\n", 2), 3);
    }
}
