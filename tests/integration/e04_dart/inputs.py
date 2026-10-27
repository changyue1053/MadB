"""Untrusted E04 fixture assembly from a public native candidate and real tools.

This file cannot grant admission. The independent finite Rust recipe and public
typed composition checks validate the resulting files and real session output.
"""
from copy import deepcopy
import argparse
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "adapters/generators/dart_ffi"))
import wire

PACKAGE = "org.madb.dart-ffi"
PREFIX = f"packages/{PACKAGE}/0.1.0"
PROFILE = "dart-c-ffi-sync-aarch64-apple-darwin"
BOUNDARY = "dart-c-sync"
ENTRY = "crates/madb-cli/examples/dart_ffi_peer.rs"
RUNTIME = "sdk/dart/ffi/madb_runtime.dart"
FEATURES = ["core.exact-numbers@1", "core.explicit-close@1", "core.resources@1", "core.values@1"]
SOURCE_PATHS = sorted([
    "LICENSE", "NOTICE", "README.md", "declarations.json", "lib/madb_dart_ffi.dart",
    "lib/src/generated.dart", "lib/src/madb_runtime.dart", "lib/src/raw_bindings.dart",
    "native/bridge.c", "native/bridge.h", "native/identity.json", "native/include/madb_rust.h",
    "native/include/madb_rust_client.h", "native/native.dylib", "pubspec.lock", "pubspec.yaml",
])


def save(root, name, value):
    return wire.write_new(root, name, (wire.canonical(value) + "\n").encode())


def copy(source, root, name):
    source = Path(source)
    wire.write_new(root, name, source.read_bytes())


def tool(path, name, version, arguments=()):
    path = Path(path).resolve(strict=True)
    return {"path": str(path), "id": name, "version": version, "sha256": wire.digest(path.read_bytes()),
            "size": path.stat().st_size, "arguments": list(arguments),
            "source": "fixed locally observed E04 tool image"}


def bridge(bridge, operation, root):
    run = subprocess.run([str(bridge), operation, str(root)], capture_output=True, timeout=60)
    if run.returncode:
        raise RuntimeError(run.stderr.decode(errors="replace"))
    return run.stdout.decode().strip()


def claims():
    result = []
    for role in ("bootstrap", "create", "checksum", "request_close", "await_closed", "release_handle"):
        subject = {"kind": "public-symbol", "public_id": f"madb.engine.{role}.v1"}
        for stage, layer, owner in (
            ("type-read", "header-reader", {"kind": "component", "id": PACKAGE + "/reader"}),
            ("abi-lowering", "ffi", {"kind": "component", "id": PACKAGE + "/generator"}),
            ("projection", "facade", {"kind": "sdk-asset", "package": PACKAGE, "asset": "facade"}),
            ("release", "temporary-call-memory", {"kind": "sdk-asset", "package": PACKAGE, "asset": "facade"}),
        ):
            result.append({"boundary": BOUNDARY, "subject": subject, "stage": stage,
                           "scope": {"kind": "call", "anchor": subject, "layer": layer}, "owner": owner})
        if role in ("request_close", "await_closed", "release_handle"):
            result.append({"boundary": BOUNDARY, "subject": subject,
                           "stage": {"request_close": "stop", "await_closed": "drain", "release_handle": "release"}[role],
                           "scope": {"kind": "resource", "anchor": {"kind": "public-symbol", "public_id": "madb.engine.create.v1"}, "layer": "dart-owner"},
                           "owner": {"kind": "sdk-asset", "package": PACKAGE, "asset": "runtime"}})
    return result


def project_digest(project):
    value = deepcopy(project)
    for item in value["policies"]:
        item.pop("origin")
    value["policies"].sort(key=wire.canonical)
    return wire.semantic("madb.project/1", value)


def prepare(args, root):
    root = Path(root).absolute()
    if root.exists() or root.is_symlink():
        raise ValueError("new fixture root required")
    root.mkdir(parents=True)
    source_paths = [ENTRY, "crates/madb-cli/examples/dart_ffi_peer/business.rs", RUNTIME, "extensions/dart-ffi/extension.json"]
    source_paths += [p.relative_to(ROOT).as_posix() for p in sorted((ROOT / "adapters/generators/dart_ffi").rglob("*"))
                     if p.is_file() and "__pycache__" not in p.parts and ".dart_tool" not in p.parts]
    for name in source_paths:
        copy(ROOT / name, root / PREFIX, name)
    dependency_files = []
    if getattr(args, "dart", None):
        for filename in ("ffi-2.2.0.tar.gz", "crypto-3.0.8.tar.gz"):
            name = "dependencies/" + filename
            copy(Path(args.runtime_archives) / filename, root, name)
            copy(Path(args.runtime_archives) / filename, root / PREFIX, name)
            source_paths.append(name)
            dependency_files.append(name)
    provider_files = []
    for file in sorted(Path(args.candidate).resolve(strict=True).rglob("*")):
        if file.is_file():
            name = "provider/" + file.relative_to(Path(args.candidate).resolve()).as_posix()
            copy(file, root, name)
            copy(file, root / PREFIX, name)
            provider_files.append(name)
    clang_version = subprocess.check_output([str(args.clang), "--version"], text=True).splitlines()[0]
    config = {"tools": {"clang": tool(args.clang, "e04-clang", clang_version, ["-isysroot", str(Path(args.sdk).resolve())]),
                        "python": tool(sys.executable, "e04-python", sys.version.split()[0]),
                        "finalizer": tool(args.bridge, "e04-finalizer", "0.1.0"),
                        "libclang": tool(args.libclang, "e04-libclang", clang_version),
                        "ffigen": tool(args.ffigen, "e04-ffigen", "23.0.0")},
              "sdk_root": str(Path(args.sdk).resolve(strict=True))}
    config["tools"]["python"]["invocation_arguments"] = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]
    config["producer_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    config["provisional"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))
    if getattr(args, "dart", None):
        version = subprocess.check_output([str(args.dart), "--version"], text=True).strip()
        if not version.startswith("Dart SDK version: 3.13.7 "):
            raise ValueError("finite recipe requires Dart stable 3.13.7")
        config["tools"]["dart"] = tool(args.dart, "e04-dart", "3.13.7")
        linker = subprocess.check_output([str(args.clang), "-print-prog-name=ld"], text=True).strip()
        linker_version = subprocess.check_output([linker, "-v"], stderr=subprocess.STDOUT, text=True).splitlines()[0]
        config["tools"]["linker"] = tool(linker, "e04-linker", linker_version)
        xcodebuild = Path(args.clang).resolve().parents[4] / "usr/bin/xcodebuild"
        xcode_version = "; ".join(subprocess.check_output([str(xcodebuild), "-version"], text=True).splitlines())
        for key, name in (("install_name_tool", "install_name_tool"), ("codesign", "codesign"), ("otool", "otool")):
            image = Path(args.clang).resolve().parent / name if name != "codesign" else Path("/usr/bin/codesign")
            version = xcode_version if name != "codesign" else "macOS26.6.2 system tool"
            if name == "otool":version = subprocess.check_output([str(image), "--version"], stderr=subprocess.STDOUT, text=True).splitlines()[0]
            config["tools"][key] = tool(image, "e04-" + key.replace("_", "-"), version)
    deps = subprocess.check_output([str(args.clang), "-x", "c", "-std=c11", "-isysroot", str(args.sdk), "-M", "-MT", "declarations", str(root / "provider/include/madb_rust_client.h")], text=True)
    config["compiler_headers"] = []
    for name in sorted(set(shlex.split(deps.replace("\\\n", " ").split(":", 1)[1]))):
        path = Path(name).resolve(strict=True)
        if not path.is_relative_to(root):
            config["compiler_headers"].append({"path": str(path), "sha256": wire.digest(path.read_bytes()), "size": path.stat().st_size})
    if getattr(args, "dart", None):
        with tempfile.TemporaryDirectory(prefix="e04-header-probe-", dir=root.parent) as scratch:
            scratch = Path(scratch)
            for source, name in (("bridge.c.in", "bridge.c"), ("bridge.h.in", "bridge.h")):
                (scratch / name).write_bytes((ROOT / "adapters/generators/dart_ffi/templates" / source).read_bytes())
            command = [str(args.clang), "-x", "c", "-std=c11", "-isysroot", str(args.sdk), "-I", str(root / "provider/include"), "-M", "-MT", "bridge", str(scratch / "bridge.c")]
            deps = subprocess.check_output(command, text=True)
            config["build_headers"] = []
            for name in sorted(set(shlex.split(deps.replace("\\\n", " ").split(":", 1)[1]))):
                path = Path(name).resolve(strict=True)
                if not path.is_relative_to(root) and not path.is_relative_to(scratch):
                    config["build_headers"].append({"path": str(path), "sha256": wire.digest(path.read_bytes()), "size": path.stat().st_size})
    save(root, "backend.json", config)
    manifest = {"schema": "madb.extension/1", "id": PACKAGE, "version": "0.1.0",
                "compatibility": {"protocol": ["1.0"], "ir": ["1.0"]}, "languages": [],
                "components": [{"id": name, "port": port, "supported_features": sorted([*FEATURES, "core.extension-composition@1"]),
                                "entrypoint": {"kind": "process", "path": ENTRY, "args": [operation]}}
                               for name, port, operation in (("reader", "contract-reader", "read"), ("generator", "generator", "generate"))],
                "dependencies": [], "routes": [], "assets": [{"kind": "sdk", "path": "sdk"}, {"kind": "sdk", "path": "provider"}],
                "execution": {"transport": "stdio-jsonl", "network": "none", "env": []}}
    registered = ROOT / "extensions/dart-ffi/extension.json"
    if wire.read_json(registered) != manifest:
        raise ValueError("registered extension differs from the approved two-port finite package")
    copy(registered, root / PREFIX, "extension.json")
    package_names = sorted(set(source_paths + provider_files + ["extension.json"]))
    package_files = [wire.reference(root / PREFIX, name) for name in package_names]
    package_digest = wire.source_digest(package_files)
    components = [{"id": PACKAGE + "/" + name, "version": "0.1.0", "sha256": package_digest} for name in ("reader", "generator")]
    selection = {"profile_id": PROFILE, "route": "org.madb.dart.c-ffi", "direction": "consumer-to-provider", "target": "aarch64-apple-darwin"}
    native_paths = ["provider/contract.json", "provider/identity.json", "provider/include/madb_rust.h", "provider/include/madb_rust_client.h", "provider/lib/libmadb_engine.dylib"]
    composition = {"schema": "madb.composition/1", "id": "dart-ffi-sync-composition", "recipe": "org.madb.dart-ffi/sync-source@1",
                   "selection": selection, "packages": [{"id": PACKAGE, "version": "0.1.0", "manifest": wire.reference(root / PREFIX, "extension.json", "madb.extension/1"),
                                                          "source": {"kind": "file-set", "files": package_files}, "source_digest": package_digest}],
                   "components": components,
                   "assets": [{"id": "facade", "package": PACKAGE, "source": {"kind": "generated", "producer": PACKAGE + "/generator", "output_slot": "sync-source", "paths": SOURCE_PATHS}},
                              {"id": "runtime", "package": PACKAGE, "source": {"kind": "source", "files": [wire.reference(root / PREFIX, RUNTIME)]}},
                              {"id": "native", "package": PACKAGE, "source": {"kind": "source", "files": [wire.reference(root / PREFIX, n) for n in native_paths]}}],
                   "dependencies": [{"from": "facade", "to": target, "scope": scope} for target, scope in (("runtime", "consumer-compile"), ("runtime", "target-runtime"), ("native", "target-runtime"))],
                   "responsibilities": claims()}
    save(root, "composition.json", composition)
    public_tools = [{**{k: t[k] for k in ("id", "version", "sha256", "source")}, "host": "aarch64-apple-darwin", "target": selection["target"], "arguments": t.get("invocation_arguments", t.get("arguments", []))}
                    for t in config["tools"].values()]
    for operation in ("reader", "generator"):
        t = tool(args.peer, "madb-dart-ffi-" + operation, "0.1.0")
        public_tools.append({**{k: t[k] for k in ("id", "version", "sha256", "source")}, "host": "aarch64-apple-darwin", "target": selection["target"], "arguments": ["read" if operation == "reader" else "generate"]})
    profile = {"schema": "madb.profile/2", "id": PROFILE, "route": selection["route"], "provider": {"language": "c", "runtime": "native"},
               "consumer": {"language": "dart", "runtime": "dart-native"}, "direction": selection["direction"], "target": selection["target"],
               "abi": "darwin-c-ffi", "artifact_kind": "dart-package", "build_system": "clang-dart", "deployment_target": "macOS26.6.2",
               "tools": public_tools, "sdk": wire.reference(root, "provider/toolchain.json", "madb.native-toolchain/1"), "macros": [],
               "required_features": sorted([*FEATURES, "core.extension-composition@1"]), "supported_features": sorted([*FEATURES, "core.extension-composition@1"]),
               "restrictions": [], "defaults": {"values": {"ownership": "copy"}, "origin": {"authority": "profile-default", "id": "dart.copy-values", "location": {"path": "profile.json", "line": 1, "column": 1}}},
               "backend_config": None, "composition": {"id": composition["id"], "file": wire.reference(root, "composition.json", "madb.composition/1"), "semantic_digest": bridge(args.bridge, "composition-digest", root)}}
    save(root, "profile.json", profile)
    profile_hash = bridge(args.bridge, "profile-digest", root)
    native_contract = wire.read_json(root / "provider/contract.json")
    selected_ids = {f"madb.engine.{r}.v1" for r in ("bootstrap", "create", "checksum", "request_close", "await_closed", "release_handle")}
    project = {"schema": "madb.project/1", "project": "dart-ffi-sync", "sdk_namespace": "madb",
               "boundaries": [{"id": BOUNDARY, "route": selection["route"], "direction": selection["direction"], "source": {"kind": "c-public-headers", "path": "provider/include/madb_rust_client.h"},
                               "profile": wire.reference(root, "profile.json", "madb.profile/2"), "target": selection["target"], "output": "generated"}],
               "policies": [{"selector": {"kind": "public_id", "id": p["symbol_id"]}, "slot": p["slot"], "families": [], "values": p["values"],
                             "origin": {"authority": "project-policy", "id": f"dart.selected-policy-{i}", "location": {"path": "project.json", "line": 1, "column": 1}}}
                            for i, p in enumerate(native_contract["policies"]) if p["symbol_id"] in selected_ids],
               "policy_families": [], "artifacts": {"package": "madb_dart_ffi", "manifest": "candidate.json"}}
    save(root, "project.json", project)
    names = sorted([*provider_files, *dependency_files, *(PREFIX + "/" + n for n in package_names), "backend.json", "composition.json", "project.json"])
    references = [wire.reference(root, name, "madb.composition/1" if name == "composition.json" else "madb.source-input/1") for name in names]
    schemas = []
    for name in ("common", "project", "lock", "profile-v2", "composition", "extension-manifest", "contract-ir", "read-request", "generate-request", "output-set", "build-task", "build-evidence", "package-request", "candidate"):
        name = "schemas/" + name + ".schema.json"
        copy(ROOT / "docs" / name, root, name)
        schemas.append(wire.reference(root, name))
    lock = {"schema": "madb.lock/1", "project_digest": project_digest(project), "profiles": [{"id": PROFILE, "semantic_digest": profile_hash, "file": wire.reference(root, "profile.json", "madb.profile/2")}],
            "tools": public_tools, "components": components, "inputs": [{k: f[k] for k in ("path", "sha256", "size")} for f in references], "schemas": schemas}
    save(root, "lock.json", lock)
    request = {"schema": "madb.read-request/1", "boundary_id": BOUNDARY, "selector": dict(selection, profile_digest=profile_hash), "inputs": references,
               "component": components[0], "lock": wire.reference(root, "lock.json", "madb.lock/1"), "profile": wire.reference(root, "profile.json", "madb.profile/2"), "output_root": "outputs/read-1"}
    save(root, "read-request.json", request)
    return root


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    for name in ("output", "candidate", "clang", "sdk", "bridge", "peer", "ffigen", "libclang"):
        p.add_argument("--" + name, required=True, type=Path)
    p.add_argument("--dart", type=Path)
    p.add_argument("--runtime-archives", type=Path)
    args = p.parse_args()
    print(prepare(args, args.output))
