//! Internal bounded bridge for the finite natural-Rust syntax reader.
use madb_contracts::native_values;
use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("madb-native-values-read: {error}");
            ExitCode::FAILURE
        }
    }
}
fn run() -> Result<(), String> {
    if std::env::args_os().len() != 1 {
        return Err("NATIVE001: accepts only a JSON envelope on stdin".into());
    }
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(native_values::MAX_ENVELOPE_BYTES as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("read stdin: {error}"))?;
    let output = native_values::read_envelope(&input).map_err(|error| error.to_string())?;
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("write stdout: {error}"))
}
