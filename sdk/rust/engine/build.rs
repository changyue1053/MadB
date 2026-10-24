use std::{env, fs, path::PathBuf};

fn digest(name: &str) -> [u8; 32] {
    println!("cargo:rerun-if-env-changed={name}");
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} must contain the generated 64-hex identity"));
    assert!(
        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "{name} must be exactly 64 hexadecimal characters"
    );
    std::array::from_fn(|i| {
        u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).expect("validated hex")
    })
}

fn main() {
    let interface = digest("MADB_INTERFACE_DIGEST");
    let profile = digest("MADB_PROFILE_DIGEST");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(output.join("identity.rs"), format!("pub const INTERFACE: [u8; 32] = {interface:?};\npub const PROFILE: [u8; 32] = {profile:?};\n")).expect("write generated identity");
}
