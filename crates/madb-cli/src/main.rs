mod bootstrap;
mod commands;
mod explain;
mod init;

fn main() -> std::process::ExitCode {
    commands::run().into()
}

mod doctor;
