use clap::Command;

fn main() {
    let _ = Command::new("madb")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Cross-language SDK engineering tools")
        .after_help("C01 bootstrap: only help and version are available. No boundary route is implemented or certified.")
        .arg_required_else_help(true)
        .get_matches();
}
