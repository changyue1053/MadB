//! Test composition root for E02; not a new CLI product command.
#[path = "extension_session_probe/session.rs"]
mod session;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let (report, _) = session::run(
        Path::new(&args[1]),
        &args[2],
        Path::new(&args[3]),
        "toolchain",
        None,
    )?;
    println!("{report}");
    Ok(())
}
