mod bootstrap;
mod commands;

fn main() -> std::process::ExitCode {
    commands::run().into()
}
