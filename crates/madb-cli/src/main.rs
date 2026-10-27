mod bootstrap;
mod commands;
mod explain;
mod init;
mod project;

fn main() -> std::process::ExitCode {
    commands::run().into()
}

mod doctor;
