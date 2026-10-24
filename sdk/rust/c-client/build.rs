use std::{env, path::PathBuf};

fn main() {
    let crate_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    for generated in ["src/raw.rs", "src/identity.rs"] {
        println!("cargo:rerun-if-changed={generated}");
        assert!(
            crate_dir.join(generated).is_file(),
            "missing generated {generated}; consume the complete first-route candidate"
        );
    }

    // The public candidate has rust/madb-c-client beside lib/libmadb_c.a.
    // No source checkout, CLI, build cache, or environment-selected library is used.
    let library_dir = crate_dir.join("../../lib");
    let library = library_dir.join("libmadb_c.a");
    println!("cargo:rerun-if-changed={}", library.display());
    assert!(
        library.is_file(),
        "missing candidate lib/libmadb_c.a; preserve the public candidate layout"
    );
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!("cargo:rustc-link-lib=static=madb_c");
}
