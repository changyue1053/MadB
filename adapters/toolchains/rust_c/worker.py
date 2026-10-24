"""Locked C05 compile/link/run worker; BuildTask evidence is emitted only on stdout."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess
import sys
import tempfile
import time

MAX_JSON = 1024 * 1024
MAX_LOG = 1024 * 1024
MAX_STREAM = 256 * 1024
LOG_PATH = "logs/command.log"
RUST_SYMBOLS = (
    "madb_fixture_bootstrap", "madb_fixture_create", "madb_fixture_checksum",
    "madb_fixture_request_close", "madb_fixture_await_closed",
    "madb_fixture_release_handle", "madb_fixture_observe",
)
TEST_SYMBOLS = (
    "madb_fixture_test_arm", "madb_fixture_test_wait", "madb_fixture_test_resume",
    "madb_fixture_test_snapshot", "madb_fixture_test_emit",
    "madb_fixture_test_set_fault", "madb_fixture_test_set_next_token",
)
C_SYMBOLS = ("madb_c_fixture_bootstrap", "madb_c_fixture_checksum")


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode()


def load_json(path):
    with Path(path).open("rb") as source:
        data = source.read(MAX_JSON + 1)
    if len(data) > MAX_JSON:
        raise ValueError("native build JSON exceeds 1 MiB")
    return decode_json(data)


def decode_json(data):
    def unique(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON member: " + key)
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def sha_file(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return "sha256:" + value.hexdigest()


def file_reference(path, root):
    return {"path": path.relative_to(root).as_posix(), "sha256": sha_file(path), "size": path.stat().st_size}


def input_path(root, name):
    if not isinstance(name, str) or not name or Path(name).is_absolute() or any(part in ("", ".", "..") for part in name.split("/")):
        raise ValueError("expected a logical native input path")
    path = root
    for part in name.split("/"):
        path = path / part
        if path.is_symlink():
            raise ValueError("native input symlink is not supported: " + name)
    if not path.is_file() or not path.resolve().is_relative_to(root):
        raise ValueError("native input is missing or escapes its frozen root: " + name)
    return path


def verify_tool(tool):
    path = Path(tool["path"])
    if not path.is_absolute() or not path.is_file() or path.stat().st_size != tool["size"] or sha_file(path) != tool["sha256"]:
        raise ValueError("locked native tool or SDK input changed: " + str(path))
    return path


def verify_tools(config):
    for tool in config["tools"].values():
        verify_tool(tool)
    for sdk_input in config.get("sdk_inputs", []):
        verify_tool(sdk_input)


def environment(config):
    result = {"PATH": os.defpath, "LC_ALL": "C", "ZERO_AR_DATE": "1"}
    allowed = {"PATH", "LC_ALL", "LANG", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET", "LIBCLANG_PATH", "SOURCE_DATE_EPOCH"}
    for key, value in config.get("environment", {}).items():
        if key not in allowed or not isinstance(value, str) or "\0" in value:
            raise ValueError("unsupported native build environment name/value: " + key)
        result[key] = value
    return result


class Commands:
    def __init__(self, config, input_root, work, scratch):
        self.config = config
        self.environment = environment(config)
        self.environment["TMPDIR"] = str(scratch)
        self.work = work
        self.paths = [(str(scratch), "$SCRATCH"), (str(input_root), "$INPUTS"), (str(work), "$WORK")]
        self.log = work / LOG_PATH
        self.log.parent.mkdir(parents=True)
        self.log.write_bytes(b"")

    def normalize(self, text):
        for path, replacement in self.paths:
            text = text.replace(path, replacement)
        return text

    def run(self, name, arguments, *, cwd=None, extra_environment=None, executable=None):
        tool = None if executable is not None else self.config["tools"][name]
        path = Path(executable) if executable is not None else verify_tool(tool)
        if tool is not None and tool.get("arguments"):
            raise ValueError("native build tools do not accept command-prefix arguments")
        command = [str(path), *map(str, arguments)]
        env = dict(self.environment)
        env.update(extra_environment or {})
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        process = None
        failure = None
        returncode = -1
        try:
            process = subprocess.Popen(command, cwd=cwd or self.work, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            deadline = time.monotonic() + (30 if executable is not None else 90)
            with selectors.DefaultSelector() as selected:
                selected.register(process.stdout, selectors.EVENT_READ, "stdout")
                selected.register(process.stderr, selectors.EVENT_READ, "stderr")
                while selected.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise RuntimeError(name + " exceeded its time budget")
                    for key, _ in selected.select(min(remaining, 0.1)):
                        block = os.read(key.fileobj.fileno(), 65536)
                        if not block:
                            selected.unregister(key.fileobj)
                            continue
                        if len(buffers[key.data]) + len(block) > MAX_STREAM:
                            raise RuntimeError(name + " exceeded its output budget")
                        buffers[key.data].extend(block)
            returncode = process.wait(timeout=max(0.1, deadline - time.monotonic()))
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
            failure = error
        finally:
            if process is not None:
                if process.poll() is None:
                    process.kill()
                process.stdout.close()
                process.stderr.close()
                returncode = process.wait(timeout=5)
            if tool is not None:
                verify_tool(tool)
        record = {"command": [self.normalize(item) for item in command], "exit_code": returncode,
                  "stdout": self.normalize(buffers["stdout"].decode(errors="replace")),
                  "stderr": self.normalize(buffers["stderr"].decode(errors="replace"))}
        if failure is not None:
            record["error"] = str(failure)
        data = canonical(record) + b"\n"
        if self.log.stat().st_size + len(data) > MAX_LOG:
            raise RuntimeError("native command log exceeded 1 MiB")
        with self.log.open("ab") as log:
            log.write(data)
        if failure is not None:
            raise RuntimeError(str(failure)) from failure
        if returncode:
            raise RuntimeError(f"{name} exited {returncode}: " + record["stderr"][-4096:])
        return bytes(buffers["stdout"])


def platform_kind(config):
    target = config["rust_target"]
    if target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
        return "apple"
    if target in ("aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"):
        return "linux"
    raise ValueError("unsupported first-route native target: " + str(target))


def phase_outputs(config, phase):
    provider = config["provider"]
    if provider not in ("rust", "c") or not isinstance(config.get("test_support", False), bool):
        raise ValueError("native provider/test-support configuration is invalid")
    if provider == "c" and config.get("test_support", False):
        raise ValueError("the stateless C provider has no test-support feature")
    platform = platform_kind(config)
    if phase == "compile":
        return ["objects/libengine.a" if provider == "rust" else "objects/provider.o"]
    if phase == "link":
        library = ("libmadb_engine.dylib" if platform == "apple" else "libmadb_engine.so") if provider == "rust" else "libmadb_c.a"
        return ["lib/" + library, "native/smoke"]
    if phase == "run":
        return ["native/run.json"]
    raise ValueError("native build phase must be compile, link or run")


def clang_flags(config, input_root, work):
    flags = config["clang_arguments"]
    if not isinstance(flags, list) or not all(isinstance(flag, str) and "\0" not in flag for flag in flags):
        raise ValueError("invalid locked Clang arguments")
    if any(flag in ("-o", "-c", "-S", "-E", "-emit-llvm", "-shared", "-dynamiclib") or flag.startswith("@") for flag in flags):
        raise ValueError("Clang phase/output flags are owned by the worker")
    return [*flags, "-fPIC", "-O2", "-g0", "-Werror=date-time",
            f"-ffile-prefix-map={input_root}=/madb/inputs", f"-ffile-prefix-map={work}=/madb/work"]


def compile_provider(config, input_root, work, commands):
    output = work / phase_outputs(config, "compile")[0]
    if config["provider"] == "rust":
        source = input_path(input_root, config.get("implementation", config.get("source", "sdk/rust/engine/src/lib.rs")))
        input_path(input_root, "identity.rs")
        arguments = ["--target", config["rust_target"], "--edition=2024", "--crate-name=madb_engine",
                     "--crate-type=staticlib", "-Dwarnings", "-Copt-level=2", "-Cpanic=unwind",
                     "-Cdebuginfo=0", "-Crelocation-model=pic", "-Ccodegen-units=1",
                     "--remap-path-prefix", f"{input_root}=/madb/inputs",
                     "--remap-path-prefix", f"{work}=/madb/work"]
        if config.get("test_support", False):
            arguments.extend(["--cfg", 'feature="test-support"'])
        commands.run("rustc", [*arguments, source, "-o", output], extra_environment={"OUT_DIR": str(input_root)})
    else:
        source = input_path(input_root, config.get("implementation", "sdk/c/first-route/madb_c.c"))
        input_path(input_root, "identity.h")
        input_path(input_root, source.with_suffix(".h").relative_to(input_root).as_posix())
        commands.run("clang", [*clang_flags(config, input_root, work), "-I", input_root, "-I", source.parent,
                               "-c", source, "-o", output])


def expected_header(input_root, scratch):
    identity = load_json(input_path(input_root, "identity.json"))
    values = []
    for name in ("interface_digest", "profile_digest"):
        value = identity[name]
        if not isinstance(value, str) or len(value) != 71 or not value.startswith("sha256:"):
            raise ValueError("native bootstrap identity must be a SHA-256 digest")
        value = bytes.fromhex(value[7:])
        if len(value) != 32:
            raise ValueError("native bootstrap identity must contain 32 bytes")
        values.append(value)
    header = scratch / "expected_identity.h"
    body = "/* Derived solely from the accepted generated identity document. */\n#include <stdint.h>\n"
    for name, value in zip(("EXPECTED_INTERFACE", "EXPECTED_PROFILE"), values):
        body += f"static const uint8_t {name}[32] = {{" + ",".join(map(str, value)) + "};\n"
    header.write_text(body)


def check_symbols(config, commands, library, symbols, platform):
    if platform == "apple":
        output = commands.run("nm", ["-gUj", library]).decode()
        actual = {line.strip().removeprefix("_") for line in output.splitlines() if line.strip() and not line.endswith(":")}
    else:
        flags = ["--defined-only", "--extern-only", "--format=posix"]
        if library.suffix == ".so":
            flags.append("--dynamic")
        output = commands.run("nm", [*flags, library]).decode()
        actual = {line.split()[0] for line in output.splitlines() if line.strip() and not line.endswith(":")}
    if actual != set(symbols):
        raise RuntimeError("native exported symbols differ: expected " + repr(sorted(symbols)) + "; actual " + repr(sorted(actual)))


def link_provider(config, input_root, work, scratch, commands):
    platform = platform_kind(config)
    provider = config["provider"]
    library = work / phase_outputs(config, "link")[0]
    flags = clang_flags(config, input_root, work)
    if provider == "rust":
        archive = input_path(input_root, "objects/libengine.a")
        symbols = (*RUST_SYMBOLS, *TEST_SYMBOLS) if config.get("test_support", False) else RUST_SYMBOLS
        exports = scratch / "exports.txt"
        if platform == "apple":
            # Current dyld requires LC_UUID; ld uses a content hash, not a random UUID.
            exports.write_text("".join("_" + symbol + "\n" for symbol in sorted(symbols)))
            link = ["-dynamiclib", "-Wl,-force_load," + str(archive), "-Wl,-exported_symbols_list," + str(exports),
                    "-Wl,-install_name,@rpath/libmadb_engine.dylib", "-Wl,-reproducible", "-Wl,-S", "-Wl,-dead_strip",
                    "-framework", "Security", "-liconv", "-lresolv"]
        else:
            exports.write_text("{ global: " + " ".join(symbol + ";" for symbol in sorted(symbols)) + " local: *; };\n")
            link = ["-shared", "-Wl,--whole-archive", archive, "-Wl,--no-whole-archive",
                    "-Wl,--version-script=" + str(exports), "-Wl,-soname,libmadb_engine.so", "-Wl,--build-id=none",
                    "-lpthread", "-ldl", "-lm", "-lrt", "-lutil"]
        commands.run("clang", [*flags, *link, "-o", library])
        check_symbols(config, commands, library, symbols, platform)
    else:
        # Archive member mode is part of the output bytes. C04 freezes inputs
        # read-only, while preparation inputs are ordinary files; normalize a
        # private copy without changing the immutable input or its basename.
        object_file = scratch / "provider.o"
        shutil.copyfile(input_path(input_root, "objects/provider.o"), object_file)
        object_file.chmod(0o644)
        commands.run("ar", ["rcs" if platform == "apple" else "rcsD", library, object_file])
        check_symbols(config, commands, library, C_SYMBOLS, platform)
    expected_header(input_root, scratch)
    smoke = input_path(input_root, config.get("smoke_source", "adapters/toolchains/rust_c/smoke.c"))
    input_path(input_root, "include/madb_rust.h" if provider == "rust" else "include/madb_c.h")
    link = ["-L", work / "lib", "-lmadb_engine"] if provider == "rust" else [library]
    if platform == "apple":
        link.extend(["-Wl,-reproducible", "-Wl,-S", "-Wl,-rpath,@loader_path/../lib"])
    else:
        link.extend(["-Wl,--build-id=none", "-Wl,-rpath,$ORIGIN/../lib"])
    commands.run("clang", [*flags, "-I", input_root / "include", "-I", scratch,
                           "-DMADB_PROVIDER_" + provider.upper(), smoke, *link, "-o", work / "native/smoke"])


def run_smoke(config, input_root, work, scratch, commands):
    executable = scratch / "native/smoke"
    executable.parent.mkdir()
    shutil.copyfile(input_path(input_root, "native/smoke"), executable)
    executable.chmod(0o700)
    if config["provider"] == "rust":
        library_name = phase_outputs(config, "link")[0]
        library = scratch / library_name
        library.parent.mkdir()
        shutil.copyfile(input_path(input_root, library_name), library)
    result = commands.run("smoke", [], executable=executable, cwd=scratch)
    expected = f"MADB_NATIVE_SMOKE_OK provider={config['provider']}\n".encode()
    if result != expected:
        raise RuntimeError("native smoke did not return its exact success marker")
    (work / "native/run.json").write_bytes(canonical({"exit_code": 0, "provider": config["provider"],
        "status": "passed", "target": config["rust_target"]}) + b"\n")


def run_phase(config, phase, input_root: Path, work: Path) -> list[str]:
    input_root, work = Path(input_root).resolve(strict=True), Path(work).resolve(strict=True)
    if input_root == work or input_root.is_relative_to(work) or work.is_relative_to(input_root):
        raise ValueError("frozen inputs and native work directories must be disjoint")
    outputs = phase_outputs(config, phase)
    if any(work.iterdir()):
        raise ValueError("native phase work directory must initially be empty")
    verify_tools(config)
    try:
        for name in outputs:
            (work / name).parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="native-build-", dir=work) as temporary:
            scratch = Path(temporary)
            commands = Commands(config, input_root, work, scratch)
            if phase == "compile":
                compile_provider(config, input_root, work, commands)
            elif phase == "link":
                link_provider(config, input_root, work, scratch, commands)
            else:
                run_smoke(config, input_root, work, scratch, commands)
        actual = {path.relative_to(work).as_posix() for path in work.rglob("*") if path.is_file()}
        if actual != {*outputs, LOG_PATH}:
            raise RuntimeError("native phase produced an unexpected file set")
        return outputs
    finally:
        verify_tools(config)


def verify_task_inputs(task, root):
    for reference in task["inputs"]:
        path = input_path(root, reference["path"])
        if path.stat().st_size != reference["size"] or sha_file(path) != reference["sha256"]:
            raise ValueError("frozen build input changed: " + reference["path"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    try:
        data = sys.stdin.buffer.read(MAX_JSON + 1)
        if len(data) > MAX_JSON:
            raise ValueError("BuildTask exceeds 1 MiB")
        task = decode_json(data)
        work, input_root = Path.cwd().resolve(), (Path.cwd().parent / "inputs").resolve(strict=True)
        config_path = args.config.resolve(strict=True)
        script_path = Path(__file__).resolve(strict=True)
        if not config_path.is_relative_to(input_root) or not script_path.is_relative_to(input_root):
            raise ValueError("worker and backend configuration must be explicit frozen inputs")
        expected_arguments = [os.path.relpath(script_path, work), "--config", os.path.relpath(config_path, work)]
        if task.get("schema") != "madb.build-task/1" or task.get("arguments") != expected_arguments:
            raise ValueError("BuildTask requires the standard native worker arguments")
        config = load_json(config_path)
        if task["selector"]["target"] != config["rust_target"] or sorted(task["outputs"]) != sorted(phase_outputs(config, task["phase"])):
            raise ValueError("BuildTask target or declared output set differs from the native phase")
        declared = {reference["path"] for reference in task["inputs"]}
        if not {config_path.relative_to(input_root).as_posix(), script_path.relative_to(input_root).as_posix()} <= declared:
            raise ValueError("BuildTask must declare the worker and backend configuration")
        verify_task_inputs(task, input_root)
        outputs = run_phase(config, task["phase"], input_root, work)
        verify_task_inputs(task, input_root)
        evidence = {"schema": "madb.build-evidence/1", "task_id": task["id"], "task_digest": task["task_digest"],
                    "input_digest": task["input_digest"], "profile_digest": task["selector"]["profile_digest"],
                    "generation_id": task["generation_id"], "tool": task["tool"], "phase": task["phase"],
                    "target": task["selector"]["target"], "status": "passed", "exit_code": 0,
                    "outputs": [file_reference(work / name, work) for name in outputs],
                    "logs": [file_reference(work / LOG_PATH, work)]}
        sys.stdout.buffer.write(canonical(evidence) + b"\n")
    except (KeyError, OSError, RuntimeError, TypeError, ValueError) as error:
        sys.stderr.write("native build: " + str(error) + "\n")
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
