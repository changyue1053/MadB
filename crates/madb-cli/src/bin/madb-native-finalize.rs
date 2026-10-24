//! Internal bounded stdin/stdout bridge for native declaration readers.
use madb_contracts::{ContractError, native};
use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("madb-native-finalize: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    if std::env::args_os().len() != 1 {
        return Err("accepts only a JSON envelope on stdin".into());
    }
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(native::MAX_ENVELOPE_BYTES as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("read stdin: {error}"))?;
    if input.len() > native::MAX_ENVELOPE_BYTES {
        return Err("FMT002: metadata envelope exceeds byte budget".into());
    }
    let output = native::finalize_envelope(&input).map_err(|error| match error {
        ContractError::Format(error) => error.to_string(),
        ContractError::Findings(findings) => findings
            .iter()
            .map(|finding| {
                format!(
                    "{} at {}: {}",
                    finding.code, finding.subject, finding.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    })?;
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("write stdout: {error}"))
}
