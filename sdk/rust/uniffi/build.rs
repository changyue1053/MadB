fn main() {
    if std::env::var("TARGET").is_ok_and(|target| target.contains("apple")) {
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libmadb_boundary_uniffi.dylib"
        );
    }
    println!("cargo:rerun-if-env-changed=MADB_PROFILE_DIGEST");
    println!("cargo:rerun-if-changed=src/boundary.udl");
    uniffi::generate_scaffolding("src/boundary.udl").expect("valid boundary UDL");
}
