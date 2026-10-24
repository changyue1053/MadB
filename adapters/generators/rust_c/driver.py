"""C04-compatible, stdlib-only real native metadata and generation helper."""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import shlex
import shutil
import sys
import tempfile

from lowering import interface_identity, lower
from native_ast import read_header
from support import NativeError, canonical, digest, file_reference, input_path, load_json, run_tool, semantic, verify_tool


def _compiler_arguments(arguments, expected_target):
    """Admit only the reviewed recipe; later flags cannot override its target."""
    if not isinstance(arguments, list) or any(not isinstance(item, str) or "\0" in item for item in arguments):
        raise NativeError("NATIVE001", "clang_arguments must be a fixed string array")
    seen = set()
    index = 0
    while index < len(arguments):
        flag = arguments[index]
        if flag in seen:
            raise NativeError("NATIVE001", "duplicate native compiler argument: " + flag)
        seen.add(flag)
        if flag in ("-target", "-isysroot", "-resource-dir"):
            index += 1
            if index == len(arguments):
                raise NativeError("NATIVE001", "missing native compiler argument value: " + flag)
            value = arguments[index]
            if flag == "-target":
                if value != expected_target:
                    raise NativeError("NATIVE001", "native compiler target differs from the frozen target/deployment")
            elif not Path(value).is_absolute() or not Path(value).is_dir():
                raise NativeError("NATIVE001", "native compiler search root must be an installed absolute directory")
        elif flag not in ("-std=c11", "-Wall", "-Wextra", "-Werror"):
            if flag != "-mmacosx-version-min=14.0" or "apple" not in expected_target:
                raise NativeError("NATIVE001", "unsupported native compiler argument: " + flag)
        index += 1
    if not {"-target", "-std=c11"} <= seen:
        raise NativeError("NATIVE001", "explicit native target and C11 language standard are required")


def _verify_context(request, config, input_root):
    allowed = {"schema", "provider", "semantics", "source", "project", "profile", "cbindgen_config", "exports", "tools",
               "clang_arguments", "environment", "rust_target", "wrapper", "sdk_inputs", "test_support_header", "test_support"}
    if set(config) - allowed:
        raise NativeError("NATIVE001", "unknown required backend configuration fields: " + str(sorted(set(config) - allowed)))
    if request.get("schema") not in ("madb.read-request/1", "madb.generate-request/1"):
        raise NativeError("NATIVE001", "native helper requires ReadRequest or GenerateRequest")
    if request["selector"]["target"] != config["rust_target"]:
        raise NativeError("NATIVE001", "request and native tool target differ")
    targets = {"aarch64-apple-darwin": "arm64-apple-macosx14.0", "x86_64-apple-darwin": "x86_64-apple-macosx14.0",
               "aarch64-unknown-linux-gnu": "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu": "x86_64-unknown-linux-gnu"}
    if config["rust_target"] not in targets:
        raise NativeError("NATIVE004", "target is outside the explicit 64-bit first-route profile set")
    _compiler_arguments(config["clang_arguments"], targets[config["rust_target"]])
    declared = set()
    for reference in request["inputs"]:
        path = input_path(input_root, reference["path"])
        data = path.read_bytes()
        if len(data) != reference["size"] or digest(data) != reference["sha256"]:
            raise NativeError("NATIVE006", f"frozen source changed: {reference['path']}")
        declared.add(reference["path"])
    required = [config["source"]]
    if config["provider"] == "rust":
        required.append(config["cbindgen_config"])
    else:
        required.extend(config["wrapper"][key] for key in ("manifest", "source", "build_script"))
    if config.get("test_support_header"):
        required.append(config["test_support_header"])
    if "test_support" in config and (not isinstance(config["test_support"], bool) or config["test_support"] != bool(config.get("test_support_header"))):
        raise NativeError("NATIVE001", "test_support must agree with the explicit test-support header")
    if not set(required) <= declared:
        raise NativeError("NATIVE006", "source, generator config and wrapper must be explicit source inputs")
    for tool in config["tools"].values():
        if set(tool) - {"path", "sha256", "size", "id", "version"}:
            raise NativeError("NATIVE001", "native tool images cannot contain hidden invocation arguments")
        verify_tool(tool)
    for sdk_input in config.get("sdk_inputs", []):
        verify_tool(sdk_input)
    return declared


def _rust_locations(surface, request, input_root):
    """Location-only source scan; ABI structure remains artifact-derived."""
    for name, fn in surface["functions"].items():
        found = []
        pattern = re.compile(rb"(?m)^\s*pub\s+(?:unsafe\s+)?extern\s+\"C\"\s+fn\s+(" + name.encode() + rb")\b")
        for reference in request["inputs"]:
            if not reference["path"].endswith(".rs"):
                continue
            data = input_path(input_root, reference["path"]).read_bytes()
            for match in pattern.finditer(data):
                prefix = data[:match.start(1)]
                found.append({"path": reference["path"], "line": prefix.count(b"\n") + 1,
                              "column": len(prefix.rsplit(b"\n", 1)[-1]) + 1})
        if len(found) != 1:
            raise NativeError("NATIVE006", f"Rust declaration source location is absent or ambiguous: {name}")
        fn["location"] = found[0]
        fn["logical_source_path"] = found[0]["path"]


def _header(request, config, input_root, scratch):
    source = input_path(input_root, config["source"])
    if config["provider"] == "c":
        return source
    header, depfile = scratch / "madb_rust.h", scratch / "cbindgen.d"
    run_tool(config, "cbindgen", ["--config", input_path(input_root, config["cbindgen_config"]), source,
                                 "--output", header, "--depfile", depfile], cwd=scratch)
    dependencies = shlex.split(depfile.read_text().replace("\\\n", " ").partition(":")[2])
    declared = {input_path(input_root, item["path"]) for item in request["inputs"]}
    if not dependencies or any(Path(name).resolve() not in declared for name in dependencies):
        raise NativeError("NATIVE006", "cbindgen read undeclared source/config dependencies")
    return header


def _read(request, config, input_root, scratch):
    header = _header(request, config, input_root, scratch)
    _header_dependencies(header, request, config, input_root)
    surface = read_header(header, config)
    if config["provider"] == "rust":
        _rust_locations(surface, request, input_root)
    project = load_json(input_path(input_root, config["project"]))
    seed = lower(surface, config, request, project)
    return header, surface, seed


def _header_dependencies(header, request, config, input_root):
    dependencies = run_tool(config, "clang", [*config["clang_arguments"], "-M", "-x", "c", header]).decode()
    paths = shlex.split(dependencies.replace("\\\n", " ").partition(":")[2])
    declared = {input_path(input_root, item["path"]) for item in request["inputs"]}
    sdk = {Path(item["path"]).resolve() for item in config.get("sdk_inputs", [])}
    for name in paths:
        path = Path(name).resolve()
        if path == header.resolve():
            continue
        if path in declared or path.is_relative_to(input_root.resolve()):
            raise NativeError("NATIVE004", f"indirect user header requires a separate supported reader: {path}")
        if path not in sdk:
            raise NativeError("NATIVE006", f"Clang read an unlocked SDK header: {path}")


def metadata_seed(request, config, input_root: Path, work: Path):
    """Preparation may inspect this seed; formal metadata always rereads the source."""
    input_root, work = Path(input_root), Path(work)
    _verify_context(request, config, input_root)
    with tempfile.TemporaryDirectory(prefix="native-reader-", dir=work) as temporary:
        _, _, seed = _read(request, config, input_root, Path(temporary))
    _verify_context(request, config, input_root)
    return seed


read_metadata_seed = metadata_seed


def _finalize(seed, config, input_root):
    envelope = {"contract": seed, "project": load_json(input_path(input_root, config["project"])),
                "profile": load_json(input_path(input_root, config["profile"]))}
    data = canonical(envelope)
    if len(data) > 1024 * 1024:
        raise NativeError("NATIVE001", "metadata finalizer envelope exceeds 1 MiB")
    output = run_tool(config, "finalizer", [], stdin=data)
    import json
    return json.loads(output)


def metadata(request, config, input_root, work):
    return _finalize(metadata_seed(request, config, input_root, work), config, input_root)


def _identity_files(identity, provider, destination):
    interface = list(bytes.fromhex(identity["interface_digest"].removeprefix("sha256:")))
    profile = list(bytes.fromhex(identity["profile_digest"].removeprefix("sha256:")))
    (destination / "identity.json").write_bytes(canonical(identity) + b"\n")
    if provider == "rust":
        (destination / "identity.rs").write_text(f"// Generated native identity; do not edit.\npub const INTERFACE: [u8; 32] = {interface};\npub const PROFILE: [u8; 32] = {profile};\n")
    else:
        c_array = lambda values: "{" + ", ".join(map(str, values)) + "}"
        (destination / "identity.h").write_text("/* Generated native identity; do not edit. */\n#include <stdint.h>\n"
            + f"static const uint8_t MADB_C_INTERFACE_DIGEST[32] = {c_array(interface)};\n"
            + f"static const uint8_t MADB_C_PROFILE_DIGEST[32] = {c_array(profile)};\n")
        source = destination / "rust/madb-c-client/src"
        source.mkdir(parents=True, exist_ok=True)
        (source / "identity.rs").write_text(f"// Generated native identity; do not edit.\npub const INTERFACE_DIGEST: [u8; 32] = {interface};\npub const PROFILE_DIGEST: [u8; 32] = {profile};\n")


def _rust_bindings(header, config, destination):
    libclang = config["tools"].get("libclang")
    if not libclang or Path(config.get("environment", {}).get("LIBCLANG_PATH", "")).resolve() != Path(libclang["path"]).parent.resolve():
        raise NativeError("NATIVE002", "bindgen must use the exact locked libclang directory")
    output = destination / "rust/madb-c-client/src/raw.rs"
    output.parent.mkdir(parents=True, exist_ok=True)
    run_tool(config, "bindgen", [header, "--output", output, "--rust-target", "1.85", "--rust-edition", "2024",
        "--no-include-path-detection", "--allowlist-type", "Madb.*", "--allowlist-function", "madb_.*",
        "--allowlist-var", "MADB_.*", "--no-doc-comments", "--default-macro-constant-type", "signed",
        "--formatter", "prettyplease", "--", *config["clang_arguments"]])


def _copy_wrapper(config, input_root, destination):
    base = destination / "rust/madb-c-client"
    for key, name in [("manifest", "Cargo.toml"), ("source", "src/lib.rs"), ("build_script", "build.rs")]:
        target = base / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(input_path(input_root, config["wrapper"][key]), target)


def _declaration_check(config, destination, scratch):
    header = destination / ("include/madb_rust.h" if config["provider"] == "rust" else "include/madb_c.h")
    run_tool(config, "clang", [*config["clang_arguments"], "-fsyntax-only", "-x", "c", header])
    if config.get("test_support_header"):
        probe = scratch / "test-support-check.c"
        probe.write_text('#include "madb_rust.h"\n#include "test-support.h"\n')
        run_tool(config, "clang", [*config["clang_arguments"], "-I", destination / "include", "-fsyntax-only", probe])
    if config["provider"] == "c":
        run_tool(config, "rustc", ["--target", config["rust_target"], "--edition=2024", "-Dwarnings",
            "--crate-name=madb_c_client", "--crate-type=lib", "--emit=metadata", destination / "rust/madb-c-client/src/lib.rs",
            "-o", scratch / "wrapper.rmeta"])


def generate(request, config, input_root: Path, work: Path):
    input_root, work = Path(input_root), Path(work)
    _verify_context(request, config, input_root)
    with tempfile.TemporaryDirectory(prefix="native-generator-", dir=work) as temporary:
        scratch = Path(temporary)
        header, surface, seed = _read(request, config, input_root, scratch)
        checked = _finalize(seed, config, input_root)
        contract = load_json(input_path(input_root, request["contract"]["path"]))
        if checked != contract or checked["structure_digest"] != request["contract_digest"] or checked["policy_digest"] != request["policy_digest"] or checked["source_digest"] != request["input_digest"]:
            raise NativeError("NATIVE006", "fresh native metadata differs from the requested checked contract")
        destination = scratch / "outputs"
        (destination / "include").mkdir(parents=True)
        target = destination / ("include/madb_rust.h" if config["provider"] == "rust" else "include/madb_c.h")
        shutil.copyfile(header, target)
        identity = interface_identity(surface, config, request["selector"]["profile_digest"])
        _identity_files(identity, config["provider"], destination)
        if config["provider"] == "c":
            _rust_bindings(header, config, destination)
            _copy_wrapper(config, input_root, destination)
        elif config.get("test_support_header"):
            target = destination / "include/test-support.h"
            shutil.copyfile(input_path(input_root, config["test_support_header"]), target)
        _declaration_check(config, destination, scratch)
        _verify_context(request, config, input_root)
        sources = [s["public_id"] for s in checked["symbols"]]
        files = [file_reference(path, destination, sources) for path in sorted(destination.rglob("*")) if path.is_file()]
        for reference in files:
            target = work / reference["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                raise NativeError("NATIVE001", "generation output already exists")
            shutil.copyfile(destination / reference["path"], target)
    value = {"schema": "madb.output-set/1", "input_digest": request["input_digest"], "task_digest": request["task_digest"],
             "profile_digest": request["selector"]["profile_digest"], "manifest_path": "outputs.json", "files": files}
    value["generation_id"] = semantic("madb.generation/1", {key: value[key] for key in ("input_digest", "task_digest", "profile_digest", "files")})
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("metadata", "generate"))
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    try:
        import json
        data = sys.stdin.buffer.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024:
            raise NativeError("NATIVE001", "request exceeds 1 MiB")
        request, config = json.loads(data), load_json(args.config)
        work, input_root = Path.cwd(), Path.cwd().parent / "inputs"
        value = metadata(request, config, input_root, work) if args.operation == "metadata" else generate(request, config, input_root, work)
        output = canonical(value) + b"\n"
        (work / ("contract.json" if args.operation == "metadata" else "outputs.json")).write_bytes(output)
        sys.stdout.buffer.write(output)
    except (NativeError, ValueError, KeyError, OSError) as error:
        sys.stderr.write(str(error) + "\n")
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
