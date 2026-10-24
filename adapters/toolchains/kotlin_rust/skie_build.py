"""C06 SKIE compile/link/run implementation over frozen inputs and explicit managed tools."""
from __future__ import annotations
import hashlib
import importlib
import json
from pathlib import Path
import sys
import tempfile
import types

from . import tools
from .smoke_workspace import stable_link
from .worker_support import Commands, LOG_PATH, canonical, copy_file, input_path, load


def _builder(inputs):
    directory = inputs / "adapters/generators/skie"
    name = "_madb_skie_builder_" + hashlib.sha256(str(directory.resolve()).encode()).hexdigest()[:16]
    if name not in sys.modules:
        package = types.ModuleType(name)
        package.__path__ = [str(directory)]
        sys.modules[name] = package
    return importlib.import_module(name + ".build")


def _record(commands, records):
    with commands.log.open("ab") as stream:
        for record in records:
            if "log" in record:
                raw = Path(record["log"]).read_text(errors="replace")
            else:
                raw = record.get("stdout", "") + record.get("stderr", "")
            normalized = {"command": [commands.normalize(str(value)) for value in record["argv"]],
                "exit_code": record["exit_code"], "output": commands.normalize(raw)}
            data = canonical(normalized) + b"\n"
            if stream.tell() + len(data) > 4 * 1024 * 1024:
                raise ValueError("SKIE command evidence exceeds 4 MiB")
            stream.write(data)


def _compile(config, inputs, work, commands):
    builder = _builder(inputs)
    identity = load(input_path(inputs, "identity.json"))
    public = work / "objects"
    records = []
    result = builder.build(inputs, config["toolchain"], public, identity["profile_digest"], identity["interface_digest"],
        test_support=config["test_support"], commands=records)
    _record(commands, records)
    expected_api = load(input_path(inputs, "generated/skie/public-api.json"))
    if load(result["api"]) != expected_api:
        raise ValueError("Formal compiled SKIE declarations differ from the frozen compiler descriptor")
    copy_file(input_path(inputs, "sdk/kotlin/skie/runtime-protocol.json"), public / "runtime-protocol.json")
    records = []
    builder.build(inputs, config["toolchain"], public / "fixtures/wrong-profile", "sha256:" + "f" * 64,
        identity["interface_digest"], test_support=True, commands=records)
    _record(commands, records)
    records = []
    builder.build_probes(inputs, config["toolchain"], public / "fixtures/p04", commands=records)
    _record(commands, records)
    names = sorted(path.relative_to(public).as_posix() for path in public.rglob("*") if path.is_file())
    (work / "sdk-inventory.json").write_bytes(canonical({"schema": "madb.sdk-inventory/1", "files": names}) + b"\n")
    return ["sdk-inventory.json", *["objects/" + name for name in names]]


def _link_at(config, inputs, work, commands):
    names = load(input_path(inputs, "sdk-inventory.json"))["files"]
    for name in names:
        copy_file(input_path(inputs, "objects/" + name), work / name)
    native = work / "native"
    native.mkdir()
    arguments = ["-swift-version", "6", "-strict-concurrency=complete", "-warnings-as-errors", "-O", "-parse-as-library",
        "-module-name", "MadBSkieSmoke", "-module-cache-path", commands.scratch / "swift-module-cache", "-target", "arm64-apple-macosx14.0", "-sdk", config["toolchain"]["sdk"],
        "-F", work / "framework", "-framework", "MadBSkie", "-Xfrontend", "-disable-incremental-llvm-codegen",
        "-file-prefix-map", str(inputs) + "=/madb/inputs", "-file-prefix-map", str(work) + "=/madb/build",
        work / "swift/MadBSkieConcurrency.swift", input_path(inputs, "sdk/swift/skie/Smoke.swift"),
        "-Xlinker", "-reproducible", "-o", native / "smoke"]
    commands.run("swiftc", arguments)
    return [*names, "native/smoke"]


def _link(config, inputs, work, commands):
    return stable_link(config, inputs, work, commands, _link_at, ["sdk/swift/skie/Smoke.swift"])


def _run(config, inputs, work, commands):
    binary = commands.scratch / "smoke"
    copy_file(input_path(inputs, "native/smoke"), binary)
    binary.chmod(0o700)
    identity = load(input_path(inputs, "identity.json"))
    result = commands.run("smoke", [identity["profile_digest"]], executable=binary)
    if result != b"MADB_C06_SKIE_SMOKE_OK\n":
        raise ValueError("Actual SKIE Swift consumer returned an unexpected result")
    (work / "native").mkdir()
    (work / "native/run.json").write_bytes(canonical({"status": "passed", "exit_code": 0,
        "variant": config["variant"], "target": config["rust_target"]}) + b"\n")
    return ["native/run.json"]


def run_phase(config, phase, inputs, work):
    expected = {"root": "/private/tmp/madb-compiler/c06-skie", "strategy": "input-addressed-exclusive-cold-build"}
    if config.get("compiler_workspace") != expected or not config.get("test_support"):
        raise ValueError("SKIE profile must explicitly admit its managed workspace and conformance-provider test exports")
    tools.verify(config["toolchain"])
    try:
        with tempfile.TemporaryDirectory(prefix="skie-", dir=work) as temporary:
            commands = Commands(config, inputs, work, Path(temporary))
            if phase == "compile": outputs = _compile(config, inputs, work, commands)
            elif phase == "link": outputs = _link(config, inputs, work, commands)
            elif phase == "run": outputs = _run(config, inputs, work, commands)
            else: raise ValueError("Unknown SKIE build phase")
        actual = {path.relative_to(work).as_posix() for path in work.rglob("*") if path.is_file()}
        if actual != set(outputs) | {LOG_PATH}:
            raise ValueError("SKIE phase output closure differs from its declared inventory")
        return sorted(outputs)
    finally:
        tools.verify(config["toolchain"])
