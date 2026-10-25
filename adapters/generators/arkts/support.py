"""Small private port helpers. Public host orchestration stays FrozenPipeline."""

from __future__ import annotations
import hashlib
import importlib.util
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import types

MAX_JSON = 1024 * 1024


def load_module(path):
    path = Path(path).resolve(strict=True)
    package = "_madb_c07_" + hashlib.sha256(str(path.parent).encode()).hexdigest()[:20]
    if package not in sys.modules:
        pkg = types.ModuleType(package)
        pkg.__path__ = [str(path.parent)]
        sys.modules[package] = pkg
    name = package + "." + path.stem
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, path)
        m = importlib.util.module_from_spec(spec)
        sys.modules[name] = m
        spec.loader.exec_module(m)
    return sys.modules[name]


def read_json(path, maximum=MAX_JSON):
    data = Path(path).read_bytes()
    if len(data) > maximum:
        raise ValueError("private JSON exceeds budget")

    def pairs(items):
        out = {}
        for k, v in items:
            if k in out:
                raise ValueError("duplicate JSON field: " + k)
            out[k] = v
        return out

    return json.loads(data.decode("utf-8"), object_pairs_hook=pairs)


def input_path(root, name):
    root = Path(root)
    if (
        not isinstance(name, str)
        or not name
        or "\\" in name
        or "\0" in name
        or Path(name).is_absolute()
        or any(p in ("", ".", "..") for p in name.split("/"))
    ):
        raise ValueError("invalid frozen logical input")
    p = root
    for part in name.split("/"):
        p = p / part
        if p.is_symlink():
            raise ValueError("frozen input contains symlink")
    if not p.is_file():
        raise ValueError("frozen input must be a regular file: " + name)
    return p


def sha(path):
    d = hashlib.sha256()
    with Path(path).open("rb") as f:
        while b := f.read(1024 * 1024):
            d.update(b)
    return "sha256:" + d.hexdigest()


def verify_refs(root, refs):
    found = set()
    for r in refs:
        name = r["path"]
        if name in found:
            raise ValueError("duplicate frozen reference: " + name)
        found.add(name)
        p = input_path(root, name)
        if type(r["size"]) is not int or p.stat().st_size != r["size"] or sha(p) != r["sha256"]:
            raise ValueError("frozen input changed: " + name)
    return found


def verify_tool(tool):
    p = Path(tool["path"])
    if (
        not p.is_absolute()
        or not p.is_file()
        or p.stat().st_size != tool["size"]
        or sha(p) != tool["sha256"]
    ):
        raise ValueError("locked executable/module changed: " + str(p))
    return p


def clean_environment(extra=None):
    """Only explicit task settings are inherited by a tool, never caller env."""
    env = {
        "PATH": os.defpath,
        "LC_ALL": "C",
        "LANG": "C",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONNOUSERSITE": "1",
    }
    if extra is not None:
        if not isinstance(extra, dict):
            raise ValueError("explicit tool environment must be a dictionary")
        for key, value in extra.items():
            if (
                not isinstance(key, str)
                or not key
                or not key.replace("_", "a").isalnum()
                or key[0].isdigit()
                or not isinstance(value, str)
                or "\0" in value
            ):
                raise ValueError("invalid explicit tool environment entry")
        env.update(extra)
    return env


def command(
    config,
    tool_name,
    args,
    *,
    cwd,
    stdin=None,
    maximum=16 * MAX_JSON,
    capture=None,
    extra_environment=None,
):
    tool = config["tools"][tool_name]
    p = verify_tool(tool)
    env = clean_environment(extra_environment)
    arguments = [str(p), *tool.get("invocation_arguments", []), *map(str, args)]

    def save(entry):
        if capture is not None:
            capture.append(entry)
        else:
            logdir = Path(cwd) / "port-commands"
            logdir.mkdir(exist_ok=True)
            index = 0
            while (logdir / f"{index:04}.json").exists():
                index += 1
            with (logdir / f"{index:04}.json").open("x") as f:
                json.dump(entry, f, indent=2)

    entry = {
        "tool": tool_name,
        "tool_sha256": tool["sha256"],
        "arguments": arguments,
        "environment": dict(sorted((extra_environment or {}).items())),
        "exit_code": None,
    }
    try:
        result = subprocess.run(
            arguments,
            cwd=cwd,
            env=env,
            input=stdin,
            capture_output=True,
            timeout=config.get("phase_timeout_seconds", 900),
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        entry.update(
            error=str(error),
            stdout=(getattr(error, "stdout", None) or b"")[:maximum].decode(errors="replace"),
            stderr=(getattr(error, "stderr", None) or b"")[:MAX_JSON].decode(errors="replace"),
        )
        save(entry)
        raise RuntimeError(tool_name + " could not complete: " + str(error)) from error
    entry.update(
        exit_code=result.returncode,
        stdout=result.stdout[:maximum].decode(errors="replace"),
        stderr=result.stderr[:MAX_JSON].decode(errors="replace"),
    )
    over = len(result.stdout) > maximum or len(result.stderr) > MAX_JSON
    if over:
        entry.update(
            output_truncated=True, stdout_bytes=len(result.stdout), stderr_bytes=len(result.stderr)
        )
    save(entry)
    verify_tool(tool)
    if over:
        raise ValueError("tool output exceeded budget")
    if result.returncode:
        raise RuntimeError(
            tool_name + " failed: " + result.stderr.decode(errors="replace")[-12000:]
        )
    return result.stdout


def publish_diagnostics(scratch, extra=()):
    """Emit bounded diagnostics; host decides whether to persist successful stderr."""
    paths = sorted((Path(scratch) / "port-commands").glob("*.json"))
    paths += [Path(scratch) / n for n in extra]
    size = sum(p.stat().st_size for p in paths)
    if size > MAX_JSON:
        raise ValueError("port diagnostics exceed 1 MiB; retained in failed scratch")
    for p in paths:
        sys.stderr.buffer.write(
            ("C07 private diagnostic " + str(p.relative_to(scratch)) + "\n").encode()
        )
        sys.stderr.buffer.write(p.read_bytes())
        sys.stderr.buffer.write(b"\n")


def copy_new(source, destination):
    destination = Path(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError("output already exists")
    destination.parent.mkdir(parents=True, exist_ok=True)
    current = destination.parent
    while current != current.parent:
        if current.is_symlink():
            raise ValueError("output parent symlink")
        current = current.parent
    with Path(source).open("rb") as src, destination.open("xb") as dst:
        shutil.copyfileobj(src, dst)
