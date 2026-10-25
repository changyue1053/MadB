fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libmadb_boundary_gobley.dylib");
    println!("cargo:rerun-if-env-changed=MADB_PROFILE_DIGEST");
    println!("cargo:rerun-if-changed=../uniffi/src/boundary.udl");
    uniffi::generate_scaffolding_for_crate("../uniffi/src/boundary.udl", "madb_boundary_gobley")
        .expect("valid shared boundary UDL");
}
