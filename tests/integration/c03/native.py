"""P02 real generators, two Apple targets, two independent native consumers.

Invoke with a new --output directory. Tools must match toolchain.json; no implicit
installation, baseline update, production CLI implementation or certification.
"""
from pathlib import Path
import argparse
import json
import os
import platform
import shlex
import shutil
import subprocess

from common import Commands, FIXTURE, ROOT, digest, file_ref, write_json
from freeze import freeze


def discover(args, recipe):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise RuntimeError("P02 requires the locked macOS Arm64 host; missing is not passed")
    def query(*argv):
        return subprocess.check_output(argv, text=True).strip()
    for argv, expected in [(("xcodebuild", "-version"), recipe["xcode"]),
                           (("xcrun", "--sdk", "macosx", "--show-sdk-version"), recipe["sdk_version"]),
                           (("xcrun", "--sdk", "macosx", "--show-sdk-build-version"), recipe["sdk_build"])]:
        if query(*argv) != expected:
            raise RuntimeError(f"Locked tool mismatch: {argv}")
    clang = Path(query("xcrun", "--find", "clang")).resolve()
    if query(str(clang), "--version").splitlines()[0] != recipe["clang"]:
        raise RuntimeError("Apple Clang build mismatch")
    tools = {"clang": clang,
             "libtool": Path(query("xcrun", "--find", "libtool")).resolve(),
             "nm": Path(query("xcrun", "--find", "nm")).resolve(),
             "rustc": Path(query("rustup", "which", "--toolchain", recipe["rust"], "rustc")).resolve(),
             "bindgen": Path(args.bindgen).resolve(), "cbindgen": Path(args.cbindgen).resolve()}
    tools["libclang"] = clang.parent.parent / "lib/libclang.dylib"
    tools["rust-nm"] = tools["rustc"].parent.parent / "lib/rustlib" / recipe["host"] / "bin/llvm-nm"
    receipt = json.loads((tools["bindgen"].parent.parent / "receipt.json").read_text())
    if receipt["rust"] != recipe["rust"]:
        raise RuntimeError("Generator build toolchain mismatch")
    for name in ("bindgen", "cbindgen"):
        record = receipt["generators"][name]
        if any(record.get(k) != v for k, v in recipe["generators"][name].items()):
            raise RuntimeError(f"Generator source receipt mismatch: {name}")
        if record["binary_sha256"] != digest(tools[name]):
            raise RuntimeError(f"Generator binary receipt mismatch: {name}")
        if query(str(tools[name]), "--version") != f"{name} {recipe['generators'][name]['version']}":
            raise RuntimeError(f"{name} version mismatch")
    sdk = Path(query("xcrun", "--sdk", "macosx", "--show-sdk-path")).resolve()
    return tools, sdk


def check_golden(path, relative):
    golden = ROOT / "tests/golden/c03" / relative
    if not golden.is_file() or path.read_bytes() != golden.read_bytes():
        raise RuntimeError(f"Generated shape differs: {relative}; inspect before an explicit baseline change")


def generate_bindings(run, tools, header, output, cflags):
    run.run([tools["bindgen"], header, "--output", output, "--rust-target", "1.85",
             "--rust-edition", "2024", "--no-include-path-detection", "--allowlist-type", "Madb.*", "--allowlist-function", "madb_.*",
             "--allowlist-var", "MADB_.*", "--no-doc-comments", "--default-macro-constant-type", "signed", "--formatter", "prettyplease", "--", *cflags])


def target_probe(run, tools, sdk, recipe, target):
    output = run.output / target
    output.mkdir()
    arch = "arm64" if target.startswith("aarch64") else "x86_64"
    cflags = ["-target", f"{arch}-apple-macosx{recipe['deployment_target']}",
              "-isysroot", str(sdk), "-resource-dir", run.run([tools["clang"], "--print-resource-dir"]), "-std=c11", "-Wall", "-Wextra", "-Werror"]
    rustflags = ["--target", target, "--edition=2024", "-Dwarnings", "-Cpanic=abort", "-Copt-level=2",
                 "-Clinker=" + str(tools["clang"]), "-Clink-arg=-isysroot", "-Clink-arg=" + str(sdk)]
    for name in ("rust_consumer.rs", "wrong_signature.rs", "c_consumer.c", "wrong_signature.c", "shape_consumer.rs"):
        shutil.copyfile(FIXTURE / name, output / name)
    shutil.copyfile(run.output / "generated/rust_provider.h", output / "rust_provider.h")
    generate_bindings(run, tools, FIXTURE / "c_provider.h", output / "c_provider.rs", cflags)
    # Full C02 ABI shape is generated/compiled separately; it is not advertised
    # as an implemented engine or linked into the stateless runtime fixture.
    generate_bindings(run, tools, ROOT / "docs/examples/first-route/expected.h", output / "expected.rs", cflags)
    run.run([tools["rustc"], *rustflags, "--crate-type=lib", "--emit=metadata", output / "shape_consumer.rs", "-o", output / "expected.rmeta"])
    shape_c = output / "shape_consumer.c"
    shape_c.write_text((ROOT / "docs/examples/first-route/consumer.c").read_text())
    run.run([tools["clang"], *cflags, "-I", run.output / "generated", "-c", shape_c, "-o", output / "shape.o"])
    run.run([tools["rustc"], *rustflags, "--crate-type=staticlib", "--crate-name=madb_probe_rust",
             FIXTURE / "rust_provider.rs", "-o", output / "libmadb_probe_rust.a"])
    run.run([tools["clang"], *cflags, "-c", FIXTURE / "c_provider.c", "-o", output / "c_provider.o"])
    run.run([tools["libtool"], "-static", "-o", output / "libmadb_probe_c.a", output / "c_provider.o"])
    run.run([tools["clang"], *cflags, "-O2", output / "c_consumer.c", output / "libmadb_probe_rust.a",
             "-framework", "Security", "-liconv", "-lresolv", "-o", output / "c_consumer"])
    run.run([tools["rustc"], *rustflags, output / "rust_consumer.rs", "-Lnative=" + str(output),
             "-lstatic=madb_probe_c", "-o", output / "rust_consumer"])
    run.run([tools["clang"], *cflags, "-fsyntax-only", output / "wrong_signature.c"], expected_error="incompatible function pointer")
    run.run([tools["rustc"], *rustflags, "--crate-type=lib", "--emit=metadata", output / "wrong_signature.rs",
             "-o", output / "wrong.rmeta"], expected_error="E0308")
    run.run([tools["clang"], *cflags, "-fpack-struct=1", "-fsyntax-only", output / "c_consumer.c"],
            expected_error="static assertion failed")
    for library, required in [("libmadb_probe_rust.a", ["_madb_probe_rust_checksum", "_madb_probe_rust_echo"]),
                              ("libmadb_probe_c.a", ["_madb_c_fixture_checksum", "_madb_probe_c_echo"])]:
        nm = tools["rust-nm"] if library.endswith("rust.a") else tools["nm"]
        symbols = run.run([nm, "-gUj", output / library]).splitlines()
        actual = sorted(s for s in symbols if s.startswith("_madb_"))
        if actual != sorted(required):
            raise RuntimeError(f"Actual exports differ from provider contract: {actual}")
    statuses = {"layout_and_compile": "passed", "link": "passed", "wrong_signatures": "rejected",
                "wrong_packing": "rejected", "c_to_rust_run": "not_run", "rust_to_c_run": "not_run"}
    if target == recipe["host"]:
        for consumer, marker, key in [("c_consumer", "C_TO_RUST_OK cases=10", "c_to_rust_run"),
                                      ("rust_consumer", "RUST_TO_C_OK cases=10", "rust_to_c_run")]:
            if run.run([output / consumer]) != marker:
                raise RuntimeError(f"Missing runtime assertion marker: {consumer}")
            statuses[key] = "passed"
    # Record the actual transitive headers consumed by Clang/libclang. SDK identity
    # also includes its settings, link stub and selected tool binaries in the lock.
    dependencies = set()
    for header in (FIXTURE / "c_provider.h", run.output / "generated/rust_provider.h",
                   ROOT / "docs/examples/first-route/expected.h"):
        deptext = run.run([tools["clang"], *cflags, "-M", "-x", "c", header]).replace("\\\n", " ")
        for value in shlex.split(deptext.partition(":")[2]):
            dependencies.add(Path(value).resolve())
    headers = [{"path": run.normalize(str(p)), "sha256": digest(p)} for p in sorted(dependencies)]
    write_json(output / "header-inputs.json", headers)
    for name in ("c_provider.rs", "expected.rs"):
        check_golden(output / name, target + "/" + name)
    return statuses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--bindgen", required=True)
    parser.add_argument("--cbindgen", required=True)
    args = parser.parse_args()
    recipe_path = Path(__file__).with_name("toolchain.json")
    recipe = json.loads(recipe_path.read_text())
    tools, sdk = discover(args, recipe)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    # Avoid inherited BINDGEN_EXTRA_CLANG_ARGS, RUSTFLAGS, search-path flags and
    # global compiler settings. SDK/tool identities are resolved explicitly.
    env = {"PATH": os.defpath, "TMPDIR": str(output), "SDKROOT": str(sdk),
           "MACOSX_DEPLOYMENT_TARGET": recipe["deployment_target"],
           "LIBCLANG_PATH": str(tools["libclang"].parent), "LC_ALL": "C"}
    substitutions = [(output, "$OUTPUT"), (ROOT, "$REPO"), (sdk, "$SDK"),
                     (tools["clang"].parent.parent, "$XCODE_TOOLCHAIN"),
                     (tools["rustc"].parent.parent, "$RUST_TOOLCHAIN")]
    substitutions.extend((tools[name], "$" + name.upper()) for name in ("bindgen", "cbindgen"))
    run = Commands(output, env, substitutions)
    source_paths = [p for folder in (FIXTURE, Path(__file__).parent, ROOT / "docs/examples/first-route")
                    for p in sorted(folder.iterdir()) if p.is_file() and p.suffix in (".rs", ".c", ".h", ".toml", ".json", ".py")]
    inputs = [file_ref(p, ROOT) for p in source_paths]
    tool_hashes = {name: digest(path) for name, path in tools.items()}
    try:
        (output / "generated").mkdir()
        for source, name in [(FIXTURE / "rust_provider.rs", "rust_provider.h"),
                             (ROOT / "docs/examples/first-route/expected.rs", "expected.h")]:
            run.run([tools["cbindgen"], "--config", FIXTURE / "cbindgen.toml", source,
                     "--output", output / "generated" / name])
        results = {target: target_probe(run, tools, sdk, recipe, target) for target in recipe["targets"]}
        for name in ("rust_provider.h", "expected.h"):
            check_golden(output / "generated" / name, name)
        tool_lock = {"recipe": recipe, "recipe_sha256": digest(recipe_path),
                     "generator_receipt": json.loads((tools["bindgen"].parent.parent / "receipt.json").read_text()),
                     "tools": [{"id": name, "path": run.normalize(str(path)), "sha256": tool_hashes[name]}
                               for name, path in tools.items()],
                     "sdk_inputs": [{"path": name, "sha256": digest(sdk / name)}
                                    for name in ("SDKSettings.json", "usr/lib/libSystem.tbd")],
                     "rust_version": run.run([tools["rustc"], "-Vv"]),
                     "host_os": platform.mac_ver()[0],
                     "rust_std_inputs": [{"target": target, "files": [{"name": p.name, "sha256": digest(p)}
                         for p in sorted((tools["rustc"].parent.parent / "lib/rustlib" / target / "lib").iterdir()) if p.is_file()]}
                         for target in recipe["targets"]],
                     "inputs": inputs,
                     "outputs": [file_ref(p, output) for p in sorted(output.rglob("*"))
                                 if p.is_file() and p.name not in ("commands.json",)],
                     "command_log": file_ref(output / "commands.json", output)}
        if inputs != [file_ref(p, ROOT) for p in source_paths] or any(digest(p) != tool_hashes[n] for n, p in tools.items()):
            raise RuntimeError("Inputs or tools changed during the probe; discard this evidence")
        write_json(output / "toolchain-lock.json", tool_lock)
        freeze(output)
        write_json(output / "result.json", {"probe": "P02", "status": "passed", "targets": results,
                   "lock": file_ref(output / "madb.lock.json", output), "observations": file_ref(output / "toolchain-lock.json", output), "sdk_certification": False})
        print(json.dumps(results, indent=2))
    except Exception as error:
        write_json(output / "result.json", {"probe": "P02", "status": "failed", "error": run.normalize(str(error)),
                                           "sdk_certification": False})
        raise


if __name__ == "__main__":
    main()
