"""Bounded, locked tool invocations for the Rust/C adapter."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


class NativeError(Exception):
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(f"{code}: {message}")


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def semantic(domain, value):
    return digest(domain.encode() + b"\n" + canonical(value))


def load_json(path):
    data = Path(path).read_bytes()
    if len(data) > 1024 * 1024:
        raise NativeError("NATIVE001", f"JSON input exceeds 1 MiB: {path}")
    def unique(items):
        result = {}
        for key, value in items:
            if key in result:
                raise NativeError("NATIVE001", f"duplicate JSON member: {key}")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def input_path(root, name):
    path = Path(name)
    if not name or path.is_absolute() or any(p in ("", ".", "..") for p in name.split("/")):
        raise NativeError("NATIVE001", f"expected logical input path: {name}")
    resolved = (root / path).resolve(strict=True)
    if not resolved.is_relative_to(root.resolve()):
        raise NativeError("NATIVE001", f"input escapes frozen root: {name}")
    return resolved


def verify_tool(tool):
    path = Path(tool["path"])
    if not path.is_absolute() or not path.is_file():
        raise NativeError("NATIVE002", f"tool path is not an installed file: {path}")
    data = path.read_bytes()
    if len(data) != tool["size"] or digest(data) != tool["sha256"]:
        raise NativeError("NATIVE002", f"locked tool bytes changed: {path}")
    return path


def run_tool(config, name, arguments, *, cwd=None, stdin=None):
    try:
        tool = config["tools"][name]
    except KeyError as error:
        raise NativeError("NATIVE001", f"missing locked tool: {name}") from error
    path = verify_tool(tool)
    environment = {"PATH": os.defpath, "LC_ALL": "C"}
    allowed = {"PATH", "LC_ALL", "LANG", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET", "LIBCLANG_PATH", "SOURCE_DATE_EPOCH"}
    for key, value in config.get("environment", {}).items():
        if key not in allowed or not isinstance(value, str) or "\0" in value:
            raise NativeError("NATIVE001", f"unsupported environment name/value: {key}")
        environment[key] = value
    command = [str(path), *map(str, tool.get("arguments", [])), *map(str, arguments)]
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run(command, cwd=cwd, env=environment, input=stdin,
                                    stdout=stdout, stderr=stderr, timeout=90)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise NativeError("NATIVE003", f"{name} did not finish: {error}") from error
        if stdout.tell() > 16 * 1024 * 1024 or stderr.tell() > 1024 * 1024:
            raise NativeError("NATIVE003", f"{name} output exceeded adapter budget")
        stdout.seek(0)
        stderr.seek(0)
        output, errors = stdout.read(), stderr.read()
    verify_tool(tool)
    if result.returncode:
        raise NativeError("NATIVE003", f"{name} exited {result.returncode}: "
                          + errors.decode(errors="replace")[-8192:])
    return output


def file_reference(path, root, sources=None):
    data = path.read_bytes()
    value = {"path": path.relative_to(root).as_posix(), "sha256": digest(data), "size": len(data)}
    if sources is not None:
        value["sources"] = sorted(sources)
    return value
