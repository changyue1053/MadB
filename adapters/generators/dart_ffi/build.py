"""Finite compile/link/run port over actual frozen Source and prior results.

stdin/stdout use the existing BuildTask/BuildEvidence schemas. No task output
identity is guessed: the host observes each completed step before binding next.
"""
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wire

OUTPUTS = {
    "compile": ["build/bridge.o", "build/compile.commands.json"],
    "link": ["build/native/bridge.dylib", "build/native/install.json", "build/lib/src/package_pins.dart", "build/provenance.json", "build/link.commands.json"],
    "run": ["build/run.observation.json", "build/run.commands.json"],
}
ARCHIVES = {
    "ffi-2.2.0": "6d7fd89431262d8f3125e81b50d3847a091d846eafcd4fdb88dd06f36d705a45",
    "crypto-3.0.8": "e3626ae6270383c9e3a549f34c9e4cd26c09ae6a9adaab2bf4737a00340efdc4",
}


def build(task, root, work):
    phase = task["phase"]
    if task["schema"] != "madb.build-task/1" or phase not in OUTPUTS or sorted(task["outputs"]) != sorted(OUTPUTS[phase]):
        raise ValueError("task differs from finite build phase")
    wire.verify(root, task["inputs"])
    config = wire.read_json(wire.confined(root, "backend.json"))
    commands = []

    def run(key, arguments, cwd=work, environment=None):
        tool = config["tools"][key]
        image = wire.check_tool(tool)
        argv = [str(image), *map(str, arguments)]
        env = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "CI": "true"}
        if environment:
            env.update(environment)
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=120)
        wire.check_tool(tool)
        if len(result.stdout) + len(result.stderr) > 8 * wire.MAX_DOCUMENT:
            raise ValueError("build command output bound exceeded")
        commands.append({"argv": argv, "tool": tool, "environment": env, "exit_code": result.returncode,
                         "stdout": result.stdout.decode(errors="strict"), "stderr": result.stderr.decode(errors="strict")})
        diagnostic = work / "partial-commands.json"
        diagnostic.write_text(wire.canonical(commands) + "\n")
        if result.returncode:
            raise ValueError("build command failed: " + commands[-1]["stdout"] + commands[-1]["stderr"])
        return result.stdout.decode()

    for tool in config["tools"].values():
        wire.check_tool(tool)
    (work / "build").mkdir()
    if phase == "compile":
        source = wire.confined(root, "native/bridge.c")
        include = wire.confined(root, "native/include/madb_rust_client.h").parent
        flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-isysroot", config["sdk_root"], "-I", str(include)]
        deps = run("clang", [*flags, "-M", "-MT", "bridge", source])
        external = []
        for name in sorted(set(shlex.split(deps.replace("\\\n", " ").split(":", 1)[1]))):
            path = Path(name).resolve(strict=True)
            if path.is_relative_to(root):
                relative = path.relative_to(root).as_posix()
                if relative not in {f["path"] for f in task["inputs"]}:
                    raise ValueError("undeclared source header")
            else:
                external.append({"path": str(path), "sha256": wire.digest(path.read_bytes()), "size": path.stat().st_size})
        if external != config["build_headers"]:
            raise ValueError("actual bridge system header closure differs")
        run("clang", [*flags, "-c", source, "-o", work / "build/bridge.o"])
        for item in external:
            if wire.digest(Path(item["path"]).read_bytes()) != item["sha256"] or Path(item["path"]).stat().st_size != item["size"]:
                raise ValueError("compiler header changed during compilation")
    elif phase == "link":
        (work / "build/native").mkdir()
        bridge = work / "build/native/bridge.dylib"
        run("clang", ["-dynamiclib", "-Wl,-headerpad_max_install_names", "--ld-path=" + config["tools"]["linker"]["path"],
                      "-isysroot", config["sdk_root"], wire.confined(root, "build/bridge.o"), wire.confined(root, "native/native.dylib"),
                      "-Wl,-install_name,@loader_path/bridge.dylib", "-o", bridge])
        run("install_name_tool", ["-change", "@rpath/libmadb_engine.dylib", "@loader_path/native.dylib", bridge])
        run("codesign", ["--force", "--sign", "-", bridge])
        graph = run("otool", ["-L", bridge])
        libraries = [line.strip().split(" (", 1)[0] for line in graph.splitlines()[1:]]
        if libraries != ["@loader_path/bridge.dylib", "@loader_path/native.dylib", "/usr/lib/libSystem.B.dylib"]:
            raise ValueError("bridge contains an unexpected native loading graph")
        run("codesign", ["--verify", "--strict", bridge])
        files = []
        for name, source in (("bridge.dylib", bridge), ("native.dylib", wire.confined(root, "native/native.dylib")), ("identity.json", wire.confined(root, "native/identity.json"))):
            data = source.read_bytes()
            files.append({"path": name, "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)})
        install = {"bridge": "bridge.dylib", "native": "native.dylib", "identity": "identity.json", "files": sorted(files, key=lambda f: f["path"])}
        raw = (wire.canonical(install) + "\n").encode()
        wire.write_new(work, "build/native/install.json", raw)
        identity = wire.read_json(wire.confined(root, "native/identity.json"))
        pins = "part of '../madb_dart_ffi.dart';\n" + f"const _installDigest = '{hashlib.sha256(raw).hexdigest()}';\n" + f"const _interfaceDigest = '{identity['interface_digest']}';\n" + f"const _profileDigest = '{identity['profile_digest']}';\n"
        wire.write_new(work, "build/lib/src/package_pins.dart", pins.encode())
        profile = wire.read_json(wire.confined(root, "profile.json"))
        generator = next(t for t in profile["tools"] if t["id"] == "madb-dart-ffi-generator")
        provenance = {"schema":"madb.native-provenance/1", "producer_revision":config["producer_revision"], "provisional":config["provisional"], "input_digest":task["input_digest"], "tool_digest":generator["sha256"]}
        wire.write_new(work, "build/provenance.json", (wire.canonical(provenance) + "\n").encode())
    else:
        scratch = Path(tempfile.mkdtemp(prefix="dart-run-", dir=work)); package = scratch / "package"; cache = scratch / "pub-cache"
        package.mkdir(); cache.mkdir(); (scratch / "temp").mkdir()
        for ref in task["inputs"]:
            name = ref["path"]
            destination = None
            if name.startswith("lib/") or name in ("pubspec.yaml", "pubspec.lock", "native/native.dylib", "native/identity.json"):
                destination = name
            elif name in ("build/native/bridge.dylib", "build/native/install.json", "build/lib/src/package_pins.dart"):
                destination = name.removeprefix("build/")
            if destination:
                wire.write_new(package, destination, wire.confined(root, name).read_bytes())
        for name, digest in ARCHIVES.items():
            archive = wire.confined(root, "dependencies/" + name + ".tar.gz")
            if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
                raise ValueError("runtime package archive differs from approved lock")
            target = cache / "hosted/pub.dev" / name; target.mkdir(parents=True)
            with tarfile.open(archive, "r:gz") as stream:
                if any(not (m.isfile() or m.isdir()) for m in stream.getmembers()):
                    raise ValueError("package archive contains symbolic or special entries")
                stream.extractall(target, filter="data")
            hashes = cache / "hosted-hashes/pub.dev"; hashes.mkdir(parents=True, exist_ok=True)
            (hashes / (name + ".sha256")).write_text(digest)
        smoke = (Path(__file__).parent / "templates/smoke.dart").read_bytes()
        wire.write_new(package, "bin/smoke.dart", smoke)
        env = {"PUB_CACHE": str(cache), "TMPDIR": str(scratch / "temp")}
        original_lock = (package / "pubspec.lock").read_bytes()
        run("dart", ["--suppress-analytics", "pub", "get", "--offline", "--enforce-lockfile"], package, env)
        if (package / "pubspec.lock").read_bytes() != original_lock:
            raise ValueError("pub changed the frozen runtime dependency lock")
        run("dart", ["--suppress-analytics", "analyze"], package, env)
        jit = run("dart", ["--suppress-analytics", "run", "bin/smoke.dart", str(package / "native")], package, env)
        run("dart", ["--suppress-analytics", "compile", "exe", "bin/smoke.dart", "-o", "smoke"], package, env)
        executable = package / "smoke"
        result = subprocess.run([str(executable), str(package / "native")], cwd=package, capture_output=True, timeout=30, env={"PATH":"/usr/bin:/bin", "CI":"true", "TMPDIR":str(scratch / "temp")})
        commands.append({"argv":[str(executable),str(package / "native")],"image":wire.reference(package,"smoke"),"exit_code":result.returncode,"stdout":result.stdout.decode(),"stderr":result.stderr.decode()})
        expected = {"checksum": 510, "state": "closed", "closed_rejected": True}
        if result.returncode or json.loads(jit) != expected or json.loads(result.stdout) != expected:
            raise ValueError("generated ordinary JIT/AOT smoke differs")
        staging = sorted(str(p) for p in (scratch / "temp").iterdir())
        # Both fixed consumers have exited. Only this invocation owns scratch.
        # Any exception/timeout above intentionally retains it for host cleanup.
        shutil.rmtree(scratch)
        if scratch.exists(): raise ValueError("owned scratch cleanup incomplete")
        wire.write_new(work, "build/run.observation.json", (wire.canonical({"jit":expected,"aot":expected,"owned_temporary_root":str(scratch),"native_staging_directories":staging,"owned_temporary_removed_after_consumer_exit":True}) + "\n").encode())
    record = (wire.canonical(commands) + "\n").encode()
    (work / "partial-commands.json").unlink()
    wire.write_new(work, f"build/{phase}.commands.json", record)
    log = f"logs/{phase}.json"; wire.write_new(work, log, record)
    wire.verify(root, task["inputs"])
    for tool in config["tools"].values():
        wire.check_tool(tool)
    return {"schema":"madb.build-evidence/1", "task_id":task["id"], "task_digest":task["task_digest"], "input_digest":task["input_digest"], "profile_digest":task["selector"]["profile_digest"], "generation_id":task["generation_id"], "tool":task["tool"], "phase":phase, "target":task["selector"]["target"], "status":"passed", "exit_code":0, "outputs":[wire.reference(work,n) for n in OUTPUTS[phase]], "logs":[wire.reference(work,log)]}


if __name__ == "__main__":
    data = sys.stdin.buffer.read(wire.MAX_DOCUMENT + 1)
    if len(data) > wire.MAX_DOCUMENT: raise ValueError("task exceeds document bound")
    task = json.loads(data)
    work = Path.cwd(); root = work.parent / "inputs"
    sys.stdout.write(wire.canonical(build(task, root, work)) + "\n")
