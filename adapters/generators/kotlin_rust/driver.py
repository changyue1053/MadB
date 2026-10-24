"""Frozen C04 reader/generator for the explicit C06 SampleEngine profiles."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import types

ROOT = Path(__file__).resolve().parents[3]
LIMIT = 1024 * 1024


def load_module(path):
    path = Path(path).resolve(strict=True)
    package = "_madb_c06_" + hashlib.sha256(str(path.parent).encode()).hexdigest()[:16]
    if package not in sys.modules:
        module = types.ModuleType(package)
        module.__path__ = [str(path.parent)]
        sys.modules[package] = module
    name = package + "." + path.stem
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    return sys.modules[name]


documents = load_module(ROOT / "adapters/toolchains/common/documents.py")
lowering = load_module(Path(__file__).with_name("lowering.py"))
policies = lowering.policies


def read_json(path):
    def pairs(values):
        result = {}
        for key, value in values:
            if key in result:
                raise ValueError("duplicate JSON member: " + key)
            result[key] = value
        return result
    data = Path(path).read_bytes()
    if len(data) > LIMIT:
        raise ValueError("managed protocol document exceeds 1 MiB")
    return json.loads(data, object_pairs_hook=pairs)


def input_path(root, name):
    if not isinstance(name, str) or Path(name).is_absolute() or any(p in ("", ".", "..") for p in name.split("/")):
        raise ValueError("invalid frozen logical path")
    path = Path(root) / name
    current = Path(root)
    for part in Path(name).parts:
        current = current / part
        if current.is_symlink():
            raise ValueError("frozen input contains a symbolic link: " + name)
    if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(Path(root).resolve()):
        raise ValueError("frozen input is missing or escapes its root: " + name)
    return path


def verify(request, config, root):
    allowed = {"schema", "variant", "source", "project", "profile", "features", "tools", "toolchain", "rust_target", "deployment_target", "phase_timeout_seconds", "precision", "test_support", "uniffi", "gobley", "derived_framework", "derived_api", "derived_receipt", "compiler_workspace", "smoke_workspace", "rust_compiler_workspace", "phase_termination_grace_ms"}
    if set(config) - allowed or config.get("schema") != "madb.kotlin-rust-backend/1":
        raise ValueError("unknown managed backend configuration")
    if config["variant"] not in ("kotlin-jvm-rust", "kotlin-native-rust", "swift-rust", "swift-kotlin"):
        raise ValueError("unsupported managed variant")
    if request["selector"]["target"] != "aarch64-apple-darwin" or config["rust_target"] != "aarch64-apple-darwin" or config["deployment_target"] != "14.0":
        raise ValueError("managed target differs from the complete C06 profile")
    if config["variant"] == "swift-kotlin" and config.get("compiler_workspace") != {
        "root": "/private/tmp/madb-compiler/c06-skie",
        "strategy": "input-addressed-exclusive-cold-build",
    }:
        raise ValueError("SKIE compilation requires the declared managed cold workspace")
    if config["variant"] != "kotlin-jvm-rust" and config.get("smoke_workspace") != {
        "root": "/private/tmp/madb-compiler/c06-smoke",
        "strategy": "input-addressed-exclusive-cold-build",
    }:
        raise ValueError("native smoke compilation requires the declared managed cold workspace")
    if config["variant"] != "swift-kotlin" and config.get("rust_compiler_workspace") != {
        "root": "/private/tmp/madb-compiler/c06-rust",
        "strategy": "input-addressed-exclusive-cold-build",
    }:
        raise ValueError("Rust provider compilation requires the declared managed cold workspace")
    if config.get("phase_timeout_seconds") != 900 or config.get("phase_termination_grace_ms") != 6000:
        raise ValueError("C06 requires its explicit compilation and cancellation cleanup budgets")
    declared = {}
    for item in request["inputs"]:
        path = input_path(root, item["path"])
        if path.stat().st_size != item["size"] or documents.sha_file(path) != item["sha256"]:
            raise ValueError("frozen managed input changed: " + item["path"])
        if item["path"] in declared:
            raise ValueError("duplicate frozen source input")
        declared[item["path"]] = item
    if config["source"] not in declared:
        raise ValueError("authoritative declaration is not a locked input")
    for name in ("uniffi", "gobley"):
        if name in config and config[name] != config["toolchain"].get(name):
            raise ValueError("backend installation differs from the verified tool observation")
    if config["tools"] != config["toolchain"]["tools"]:
        raise ValueError("managed executable list differs from the verified observation")
    load_module(ROOT / "adapters/toolchains/kotlin_rust/tools.py").verify(config["toolchain"])
    return declared


def environment(config, work):
    work = Path(work)
    home, temporary = work / "home", work / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    temporary.mkdir(parents=True, exist_ok=True)
    observed = config["toolchain"]
    paths = sorted({str(Path(t["path"]).parent) for t in config["tools"].values()})
    env = {"PATH": os.pathsep.join(paths + ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]),
           "HOME": str(home), "TMPDIR": str(temporary), "LC_ALL": "C", "LANG": "C",
           "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1", "CARGO_NET_OFFLINE": "true",
           "CARGO_HOME": str(home / "cargo"), "RUSTC": config["tools"]["rustc"]["path"]}
    env.update(observed["environment"])
    return env


def command(config, arguments, work, stdin=None, max_output=LIMIT, *, cwd=None):
    result = subprocess.run([str(a) for a in arguments], cwd=cwd or work, env=environment(config, work),
                            input=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=config["phase_timeout_seconds"])
    if len(result.stdout) > max_output or len(result.stderr) > 8 * LIMIT:
        raise ValueError("managed helper exceeded its output budget")
    if result.returncode:
        raise RuntimeError("managed tool failed: " + str(arguments[0]) + "\n" + result.stderr.decode(errors="replace")[-16000:])
    return result.stdout


def read_skie_api(config, root, framework, work, output):
    protocol = read_json(input_path(root, "sdk/kotlin/skie/runtime-protocol.json"))
    arguments = [config["tools"]["python"]["path"], *config["tools"]["python"].get("invocation_arguments", []), ROOT / "adapters/generators/skie/read_api.py",
                 framework, "--sdk", config["toolchain"]["sdk"], "--output", output]
    tools_file = Path(work) / "api-tools.json"
    tools_file.write_text(documents.canonical(config["tools"]))
    arguments.extend(["--tools", tools_file])
    for name in protocol["public_exports"]:
        arguments.extend(["--export-root", name])
    command(config, arguments, work)


def _read(request, config, root, work):
    if config["variant"] == "swift-kotlin":
        receipt = read_json(input_path(root, config["derived_receipt"]))
        if receipt["profile_digest"] != request["selector"]["profile_digest"]:
            raise ValueError("compiled declaration belongs to another profile")
        declared = {r["path"]: r for r in request["inputs"]}
        for item in receipt["author_inputs"] + receipt["outputs"]:
            actual = declared.get(item["path"])
            if actual is None or any(actual[k] != item[k] for k in ("sha256", "size")):
                raise ValueError("compiled declaration has stale or undeclared source provenance")
        if receipt["toolchain_digest"] != documents.semantic("madb.compiler-observation/1", config["toolchain"]):
            raise ValueError("compiled declaration toolchain identity changed")
        api_path = input_path(root, config["derived_api"])
        api = read_json(api_path)
        output = Path(work) / "read-skie-api.json"
        read_skie_api(config, root, Path(root) / config["derived_framework"], work, output)
        if read_json(output) != api:
            raise ValueError("actual framework API differs from its frozen compiler descriptor")
        descriptor = load_module(Path(__file__).with_name("skie_lowering.py")).descriptor(api, config["source"])
    else:
        gobley = config["variant"] == "kotlin-native-rust"
        tool = config["tools"]["metadata_v029" if gobley else "metadata_v032"]
        crate = "madb_boundary_gobley" if gobley else "madb_boundary_uniffi"
        data = command(config, [tool["path"], input_path(root, config["source"]), crate], work)
        descriptor = json.loads(data)
    seed = lowering.lower(descriptor, request, config)
    refs = [{key: item[key] for key in ("path", "sha256", "size")} for item in request["inputs"]]
    seed["source_digest"] = documents.semantic("madb.source/1", sorted(refs, key=lambda x: x["path"]))
    return descriptor, seed


def metadata_seed(request, config, input_root, work):
    root, work = Path(input_root), Path(work)
    verify(request, config, root)
    work.mkdir(parents=True, exist_ok=True)
    _, seed = _read(request, config, root, work)
    verify(request, config, root)
    return seed


def finalize(seed, config, root, work):
    envelope = dict(contract=seed, project=read_json(input_path(root, config["project"])), profile=read_json(input_path(root, config["profile"])))
    payload = documents.canonical(envelope).encode()
    if len(payload) > LIMIT:
        raise ValueError("metadata envelope exceeds the shared protocol limit")
    return json.loads(command(config, [config["tools"]["finalizer"]["path"]], work, payload))


def metadata(request, config, input_root, work):
    return finalize(metadata_seed(request, config, input_root, work), config, input_root, work)


def prepare_artifacts(config, workspace, selector, author_inputs):
    """An explicit pre-compilation supplies actual ObjC/Swift declarations to C04."""
    workspace = Path(workspace)
    build = load_module(ROOT / "adapters/generators/skie/build.py")
    derived = workspace / "derived"
    derived.mkdir()
    framework = build.build(workspace, config["toolchain"], workspace / "prepare-skie", selector["profile_digest"], "sha256:" + "0" * 64, test_support=True)
    framework = Path(framework["framework"])
    destination = derived / "framework/MadBSkie.framework"
    for source in sorted(framework.rglob("*")):
        if not source.is_file() or source.name.endswith(".swiftsourceinfo"):
            continue
        if not source.resolve().is_relative_to(framework.resolve()):
            raise ValueError("framework input symlink escapes compiler output")
        target = destination / source.relative_to(framework)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    config.update(derived_framework="derived/framework/MadBSkie.framework", derived_api="derived/skie-api.json", derived_receipt="derived/receipt.json", source="derived/framework/MadBSkie.framework/Headers/MadBSkie.h")
    work = workspace / "prepare-skie-api"
    work.mkdir()
    read_skie_api(config, workspace, destination, work, derived / "skie-api.json")
    refs = [documents.ref(workspace, p.relative_to(workspace).as_posix(), "madb.derived-declaration/1") for p in sorted(derived.rglob("*")) if p.is_file()]
    receipt = dict(schema="madb.derived-declaration/1", profile_digest=selector["profile_digest"], author_inputs=author_inputs, outputs=refs,
                   toolchain_digest=documents.semantic("madb.compiler-observation/1", config["toolchain"]))
    documents.save(workspace, "derived/receipt.json", receipt)
    return refs + [documents.ref(workspace, "derived/receipt.json", "madb.derived-declaration/1")]


def copy_new(source, root, name):
    """Copy only a new regular output below an ordinary parent chain."""
    root = Path(root)
    if root.is_symlink() or not root.is_dir() or Path(name).is_absolute() or any(p in ("", ".", "..") for p in name.split("/")):
        raise ValueError("invalid output root or path")
    parent = root
    for part in Path(name).parts[:-1]:
        parent = parent / part
        if parent.is_symlink() or parent.exists() and not parent.is_dir():
            raise ValueError("output parent is not an ordinary directory")
        parent.mkdir(exist_ok=True)
    target = root / name
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    with Path(source).open("rb") as src, os.fdopen(os.open(target, flags, 0o600), "wb") as dst:
        shutil.copyfileobj(src, dst)


def generated_files(destination, variant):
    """Inspect before copying so a tool cannot launder links into regular output."""
    destination = Path(destination)
    if destination.is_symlink() or not destination.is_dir():
        raise ValueError("generator output root must be an ordinary directory")
    files = []
    allowed = {
        "kotlin-jvm-rust": ("generated/uniffi/", {".kt", ".h"}),
        "kotlin-native-rust": ("generated/gobley/", {".kt", ".h", ".def"}),
        "swift-rust": ("generated/uniffi/", {".swift", ".h", ".modulemap"}),
        "swift-kotlin": ("generated/skie/", {".json"}),
    }
    fixed = {"identity.json", "declarations.json", "generated/facade/MadbBoundaryClient.swift", "generated/facade/MadBSkieConcurrency.swift", "generated/facade/MadbBoundaryClient.kt"}
    total = 0
    for path in sorted(Path(destination).rglob("*")):
        mode = path.lstat().st_mode
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            raise ValueError("generator emitted a link or special file")
        name = path.relative_to(destination).as_posix()
        prefix, extensions = allowed[variant]
        if name not in fixed and not (name.startswith(prefix) and (path.suffix in extensions or path.name == "module.modulemap")):
            raise ValueError("unexpected generated artifact: " + name)
        if not path.resolve().is_relative_to(Path(destination).resolve()):
            raise ValueError("generated artifact escaped its output tree")
        total += path.stat().st_size
        if len(files) >= 1024 or total > 64 * LIMIT:
            raise ValueError("generated source exceeds the C06 output budget")
        path.read_text(encoding="utf-8")
        files.append(path)
    return files


def generate(request, config, root, work):
    root, work = Path(root), Path(work)
    verify(request, config, root)
    descriptor, seed = _read(request, config, root, work)
    contract = finalize(seed, config, root, work)
    expected = read_json(input_path(root, request["contract"]["path"]))
    if contract != expected or request["contract_digest"] != contract["structure_digest"] or request["policy_digest"] != contract["policy_digest"] or request["input_digest"] != contract["source_digest"]:
        raise ValueError("fresh managed declarations/policies differ from the checked contract")
    with tempfile.TemporaryDirectory(prefix="managed-generation-", dir=work) as temporary:
        destination = Path(temporary) / "outputs"
        destination.mkdir()
        variant = config["variant"]
        if variant == "swift-kotlin":
            documents.save(destination, "generated/skie/public-api.json", read_json(input_path(root, config["derived_api"])))
            facade = "sdk/swift/skie/MadBSkieConcurrency.swift"
        elif variant == "kotlin-native-rust":
            output = destination / "generated/gobley"
            command(config, [config["tools"]["gobley"]["path"], input_path(root, config["source"]), "--crate", "madb_boundary_gobley", "--config", input_path(root, "sdk/rust/gobley/uniffi.toml"), "--out-dir", output], work)
            facade = "sdk/kotlin/gobley/MadbBoundaryClient.kt"
        else:
            output = destination / "generated/uniffi"
            language = "kotlin" if variant == "kotlin-jvm-rust" else "swift"
            command(config, [config["tools"]["uniffi_bindgen"]["path"], "generate", input_path(root, config["source"]), "--crate", "madb_boundary_uniffi", "--metadata-no-deps", "--language", language, "--no-format", "--config", input_path(root, "sdk/rust/uniffi/bindgen.toml"), "--out-dir", output], work, cwd=Path(root) / "sdk/rust/uniffi")
            facade = "sdk/swift/uniffi/MadbBoundaryClient.swift" if language == "swift" else "sdk/kotlin/uniffi-jvm/MadbBoundaryClient.kt"
            if language == "kotlin":
                # Publish the same generator's C header for independent ABI probes.
                # The actual API authority remains the version-matched UDL reader.
                header_work = Path(temporary) / "ffi-header"
                command(config, [config["tools"]["uniffi_bindgen"]["path"], "generate", input_path(root, config["source"]), "--crate", "madb_boundary_uniffi", "--metadata-no-deps", "--language", "swift", "--no-format", "--config", input_path(root, "sdk/rust/uniffi/bindgen.toml"), "--out-dir", header_work], work, cwd=Path(root) / "sdk/rust/uniffi")
                headers = list(header_work.glob("*.h"))
                if len(headers) != 1 or headers[0].is_symlink():
                    raise ValueError("UniFFI did not emit exactly one ordinary C header")
                include = output / "include"
                include.mkdir()
                shutil.copyfile(headers[0], include / headers[0].name)
        if facade:
            target = destination / "generated/facade" / Path(facade).name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(input_path(root, facade), target)
        identity = dict(schema="madb.native-identity/1", bootstrap_version=1, interface_digest=documents.semantic("madb.managed-interface/1", descriptor), profile_digest=request["selector"]["profile_digest"])
        documents.save(destination, "identity.json", identity)
        documents.save(destination, "declarations.json", descriptor)
        sources = sorted(symbol["public_id"] for symbol in contract["symbols"])
        files = [dict(documents.ref(destination, p.relative_to(destination).as_posix()), sources=sources) for p in generated_files(destination, variant)]
        for item in files:
            copy_new(destination / item["path"], work, item["path"])
    verify(request, config, root)
    value = dict(schema="madb.output-set/1", input_digest=request["input_digest"], task_digest=request["task_digest"], profile_digest=request["selector"]["profile_digest"], manifest_path="outputs.json", files=files)
    value["generation_id"] = documents.semantic("madb.generation/1", {k: value[k] for k in ("input_digest", "task_digest", "profile_digest", "files")})
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("metadata", "generate"))
    parser.add_argument("--config", type=Path, required=True)
    args = parser.parse_args()
    raw = sys.stdin.buffer.read(LIMIT + 1)
    if len(raw) > LIMIT:
        raise ValueError("managed request exceeds 1 MiB")
    request = json.loads(raw)
    root, work = Path.cwd().parent / "inputs", Path.cwd()
    config = read_json(args.config)
    with tempfile.TemporaryDirectory(prefix="private-driver-", dir=work) as temporary:
        scratch = Path(temporary)
        value = (metadata if args.operation == "metadata" else generate)(request, config, root, scratch)
        if args.operation == "generate":
            for item in value["files"]:
                source = input_path(scratch, item["path"])
                copy_new(source, work, item["path"])
    output = (documents.canonical(value) + "\n").encode()
    if len(output) > LIMIT:
        raise ValueError("managed response exceeds 1 MiB")
    (work / ("contract.json" if args.operation == "metadata" else "outputs.json")).write_bytes(output)
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, KeyError, subprocess.SubprocessError) as error:
        message = "C06 declaration: " + str(error)[-16000:]
        try:
            with (Path.cwd() / "c06-failure.json").open("x") as output:
                json.dump({"schema": "madb.c06-failure/1", "message": message}, output)
        except OSError:
            pass
        print(message, file=sys.stderr)
        raise SystemExit(2) from error
