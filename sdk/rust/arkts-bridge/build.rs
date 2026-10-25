use std::env;
use std::path::PathBuf;

fn main() {
    assert_eq!(env::var("TARGET").unwrap(), "aarch64-unknown-linux-ohos");
    println!("cargo:rerun-if-env-changed=MADB_BUILD_PHASE");
    for key in [
        "NAPI_DEBUG_GENERATED_CODE",
        "NAPI_TYPE_DEF_TMP_FOLDER",
        "NAPI_BUILD_TARGET_NAME",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    match env::var("MADB_BUILD_PHASE").as_deref() {
        Ok("metadata") => {
            let out = PathBuf::from(env::var("OUT_DIR").unwrap());
            std::fs::write(out.join("madb-phase.txt"), "metadata\n").unwrap();
        }
        Ok("native") => {
            // Compilation belongs to the existing C04 worker. This script only
            // links its freshly compiled, frozen-input archive into this DSO.
            napi_build_ohos::setup();
            let archive = PathBuf::from(
                env::var("MADB_NATIVE_ARCHIVE").expect("explicit C04 native archive"),
            );
            assert!(archive.is_absolute() && archive.is_file());
            assert_eq!(archive.file_name().unwrap(), "libmadb_arkts_gateway.a");
            println!("cargo:rerun-if-env-changed=MADB_NATIVE_ARCHIVE");
            println!("cargo:rerun-if-changed={}", archive.display());
            println!(
                "cargo:rustc-link-search=native={}",
                archive.parent().unwrap().display()
            );
            println!("cargo:rustc-link-lib=static=madb_arkts_gateway");
            println!("cargo:rustc-link-lib=dylib=c++_shared");
            println!("cargo:rustc-link-lib=dylib=uv");
            println!("cargo:rustc-link-arg=-Wl,--no-undefined");
        }
        _ => panic!("MADB_BUILD_PHASE must be metadata or native"),
    }
}
