"""Cold, content-addressed linking for private producer probes with normal UUIDs."""
from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path

from .worker_support import Commands, canonical, copy_file, input_path, load, sha

WORKSPACE = {"root": "/private/tmp/madb-compiler/c06-smoke", "strategy": "input-addressed-exclusive-cold-build"}


def _workspace_module():
    path = Path(__file__).resolve().parents[1] / "common/compiler_workspace.py"
    name = "madb_compiler_workspace_" + hashlib.sha256(str(path).encode()).hexdigest()
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError("Missing frozen compiler workspace implementation")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class _CompilerCommands(Commands):
    def __init__(self, config, inputs, work, scratch, stage, workspace):
        super().__init__(config, inputs, work, scratch)
        self.stage, self.workspace = stage, workspace

    def run(self, name, args, *, cwd=None, extra=None, executable=None):
        if executable is not None or cwd is not None:
            raise ValueError("Stable compiler stage accepts only observed compiler invocations")
        command = [self.images[name]["path"], *map(str, args)]
        environment = dict(self.environment)
        environment.update(extra or {})
        log = self.stage / "build.log"
        code = self.workspace.execute(command, self.stage / "project", environment, log)
        output = log.read_bytes()
        record = canonical({"command": [self.normalize(value) for value in command], "exit_code": code,
                            "output": self.normalize(output.decode(errors="replace"))}) + b"\n"
        if self.log.stat().st_size + len(record) > 4 * 1024 * 1024:
            raise ValueError("Private compiler evidence exceeds 4 MiB")
        with self.log.open("ab") as stream:
            stream.write(record)
        if code:
            raise RuntimeError(name + " failed: " + output.decode(errors="replace")[-5000:])
        return output


def stable_link(config, inputs, work, commands, callback, extra_sources):
    """Execute the same linking callback under a locked, clean, portable identity."""
    if config.get("smoke_workspace") != WORKSPACE:
        raise ValueError("Native producer probes require the declared stable compiler workspace")
    workspace = _workspace_module()
    inventory = load(input_path(inputs, "sdk-inventory.json"))["files"]
    names = ["sdk-inventory.json", "identity.json", *extra_sources, *["objects/" + name for name in inventory]]
    references = [{"path": name, "sha256": sha(input_path(inputs, name)),
                   "size": input_path(inputs, name).stat().st_size} for name in sorted(names)]
    # Recipe source binds compiler flags and copying behavior without any producer path.
    for directory in ("adapters/toolchains/kotlin_rust", "adapters/toolchains/common"):
        for path in sorted((inputs / directory).rglob("*.py")):
            references.append({"path": path.relative_to(inputs).as_posix(), "sha256": sha(path), "size": path.stat().st_size})
    identity = load(input_path(inputs, "identity.json"))
    key = workspace.identity(references, config["toolchain"], identity["profile_digest"], identity["interface_digest"])
    with workspace.acquire(key, base=Path(WORKSPACE["root"])) as stage:
        project = stage / "project"
        frozen, output, scratch = project / "inputs", project / "output", project / "scratch"
        for directory in (frozen, output, scratch):
            directory.mkdir(parents=True)
        for name in names:
            copy_file(input_path(inputs, name), frozen / name)
        compiler = _CompilerCommands(config, frozen, output, scratch, stage, workspace)
        outputs = callback(config, frozen, output, compiler)
        for name in outputs:
            copy_file(output / name, work / name)
        data = compiler.log.read_bytes()
        if commands.log.stat().st_size + len(data) > 4 * 1024 * 1024:
            raise ValueError("Private compiler evidence exceeds 4 MiB")
        with commands.log.open("ab") as stream:
            stream.write(data)
        return outputs
