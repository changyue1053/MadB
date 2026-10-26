mod bootstrap;
mod commands;
mod explain;

fn main() -> std::process::ExitCode {
    commands::run().into()
}
