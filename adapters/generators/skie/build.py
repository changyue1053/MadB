"""Frozen SKIE producer recipe shared by C06 pre-probe and formal compilation."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import shutil

from .read_api import extract
from . import workspace

EXCLUDED = {"build", ".gradle", ".kotlin", ".git", "__pycache__"}


def sha(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _digest(value: str) -> str:
    value = value.removeprefix("sha256:")
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ValueError("Expected a resolved SHA256 identity")
    return value


def _copy_source(source: Path, destination: Path) -> list[dict]:
    inputs = []
    for path in sorted(source.rglob("*")):
        relative = path.relative_to(source)
        if any(part in EXCLUDED for part in relative.parts) or path.suffix in {".pyc", ".log"}:
            continue
        if path.is_symlink():
            raise ValueError("SKIE author sources cannot contain symlinks: " + str(relative))
        if path.is_file():
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
            inputs.append({"path": relative.as_posix(), "sha256": sha(path), "size": path.stat().st_size})
    return inputs


def materialize_framework(source: Path, destination: Path) -> list[dict]:
    """Publish regular files only; retain module interfaces and DWARF, omit local editor source index."""
    physical = source / "Versions" / "A"
    if not physical.is_dir():
        physical = source
    if not (physical / source.stem).is_file():
        raise ValueError("Compiler did not produce the expected framework binary")
    inventory = []
    for path in sorted(physical.rglob("*")):
        if path.is_symlink():
            raise ValueError("Unexpected link inside the framework's physical version")
        if not path.is_file() or path.suffix == ".swiftsourceinfo":
            continue
        relative = path.relative_to(physical)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        inventory.append({"path": relative.as_posix(), "sha256": sha(target), "size": target.stat().st_size})
    return inventory


def build(repo_inputs, observation, output, profile_digest, interface_digest, test_support=False, *, work=None, commands=None):
    """Build from frozen author files plus the caller-verified managed tool/dependency observation.

    This conformance provider explicitly exports its GC observation hook. The profile must
    acknowledge that role with test_support=True; the hook is never a cleanup dependency.
    """
    if not test_support:
        raise ValueError("The SKIE conformance provider explicitly requires test_support=True")
    repo_inputs, output = Path(repo_inputs).resolve(), Path(output).resolve()
    if output.exists() and any(output.iterdir()):
        raise ValueError("Producer output directory must be empty")
    source = repo_inputs / "sdk/kotlin/skie"
    inputs = []
    for path in sorted(source.rglob("*")):
        relative = path.relative_to(source)
        if any(part in EXCLUDED for part in relative.parts) or path.suffix in {".pyc", ".log"}:
            continue
        if path.is_symlink():
            raise ValueError("SKIE author sources cannot contain symlinks")
        if path.is_file():
            inputs.append({"path": relative.as_posix(), "sha256": sha(path), "size": path.stat().st_size})
    key = workspace.identity(inputs, observation, profile_digest, interface_digest)
    with workspace.acquire(key) as stage:
        return _build_locked(repo_inputs, observation, output, profile_digest, interface_digest, stage, commands, inputs)


def _build_locked(repo_inputs, observation, output, profile_digest, interface_digest, work, commands, inputs):
    project = work / "project"
    if _copy_source(repo_inputs / "sdk/kotlin/skie", project) != inputs:
        raise ValueError("Author inputs changed while acquiring compiler workspace")
    tool = observation["tools"]
    managed = observation["managed"]["paths"]
    gradle = Path(tool["gradle"]["path"])
    command = [str(gradle), "--no-daemon", "--offline", "--dependency-verification=strict",
        "-Pmadb.mavenRepo=" + observation["trees"]["skie-maven"]["root"],
        "-Pkotlin.native.home=" + managed["kotlin-native"],
        "-PmadbProfileDigest=" + _digest(profile_digest),
        "-PmadbInterfaceDigest=" + _digest(interface_digest),
        "linkReleaseFrameworkMacosArm64"]
    environment = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(work / "home"),
        "GRADLE_USER_HOME": str(work / "gradle-home"), "LANG": "en_US.UTF-8", "LC_ALL": "C"}
    environment.update(observation["environment"])
    Path(environment["HOME"]).mkdir()
    log = work / "build.log"
    exit_code = _execute(command, project, environment, log)
    if commands is not None:
        commands.append({"argv": command, "cwd": str(project), "exit_code": exit_code, "stdout": log.read_text(errors="replace")})
    if exit_code:
        raise RuntimeError("SKIE Release producer failed:\n" + log.read_text(errors="replace")[-12000:])
    compiled = project / "build/bin/macosArm64/releaseFramework/MadBSkie.framework"
    framework = output / "framework/MadBSkie.framework"
    inventory = materialize_framework(compiled, framework)
    exports = json.loads((repo_inputs / "sdk/kotlin/skie/runtime-protocol.json").read_text())["public_exports"]
    descriptor = extract(framework, "arm64-apple-macosx14.0", Path(observation["sdk"]), roots=set(exports), tools=tool)
    descriptor_path = output / "public-api.json"
    descriptor_path.write_text(json.dumps(descriptor, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
    facade_source = repo_inputs / "sdk/swift/skie/MadBSkieConcurrency.swift"
    facade = output / "swift/MadBSkieConcurrency.swift"
    facade.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(facade_source, facade)
    receipt = {"schema": "madb.skie-derivation/1", "source_inputs": inputs,
        "profile_digest": "sha256:" + _digest(profile_digest), "interface_digest": "sha256:" + _digest(interface_digest),
        "managed_receipt_sha256": observation["managed"]["receipt_sha256"],
        "framework_files": inventory, "api_sha256": sha(descriptor_path), "facade_sha256": sha(facade),
        "excluded": ["optional .swiftsourceinfo editor source index; generated interfaces and DWARF retained"]}
    (output / "derivation.json").write_text(json.dumps(receipt, sort_keys=True, indent=2) + "\n")
    return {"framework": framework, "api": descriptor_path, "facade": facade, "receipt": output / "derivation.json", "log": log}


_execute = workspace.execute


def build_probes(repo_inputs, observation, output, *, commands=None):
    """Build actual SKIE unavailable declarations from source, never synthetic annotations."""
    repo_inputs, output = Path(repo_inputs).resolve(), Path(output).resolve()
    source = repo_inputs / "adapters/generators/skie/probes"
    inputs = [{"path": str(path.relative_to(source)), "sha256": sha(path), "size": path.stat().st_size}
        for path in sorted(source.rglob("*")) if path.is_file() and not any(p in EXCLUDED for p in path.relative_to(source).parts)]
    verification = repo_inputs / "sdk/kotlin/skie/gradle/verification-metadata.xml"
    inputs.append({"path": "gradle/verification-metadata.xml", "sha256": sha(verification), "size": verification.stat().st_size})
    key = workspace.identity(inputs, observation, "p04-unavailable-declarations", "p04-unavailable-declarations")
    with workspace.acquire(key) as stage:
        project = stage / "project"
        _copy_source(source, project)
        (project / "gradle").mkdir(exist_ok=True)
        shutil.copyfile(verification, project / "gradle/verification-metadata.xml")
        environment = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(stage / "home"),
            "GRADLE_USER_HOME": str(stage / "gradle-home"), "LANG": "en_US.UTF-8", "LC_ALL": "C"}
        environment.update(observation["environment"])
        Path(environment["HOME"]).mkdir()
        clang = [observation["tools"]["clang"]["path"], "-target", "arm64-apple-macosx14.0", "-dynamiclib", "-fobjc-arc",
            "-isysroot", observation["sdk"], "-framework", "Foundation", "native/Custom.m", "-Wl,-reproducible",
            "-install_name", "@rpath/Custom.framework/Custom", "-o", "native/Custom.framework/Custom"]
        gradle = [observation["tools"]["gradle"]["path"], "--no-daemon", "--offline", "--dependency-verification=strict",
            "-Pmadb.mavenRepo=" + observation["trees"]["skie-maven"]["root"],
            "-Pkotlin.native.home=" + observation["managed"]["paths"]["kotlin-native"], "linkReleaseFrameworkMacosArm64"]
        for command in (clang, gradle):
            log = stage / "build.log"
            exit_code = _execute(command, project, environment, log)
            text = log.read_text(errors="replace")
            if commands is not None:
                commands.append({"argv": command, "cwd": str(project), "exit_code": exit_code, "stdout": text})
            if exit_code:
                raise RuntimeError("P04 actual producer failed:\n" + text[-16000:])
        source_framework = project / "build/bin/macosArm64/releaseFramework/MadBSkieGaps.framework"
        materialize_framework(source_framework, output / "MadBSkieGaps.framework")
        materialize_framework(project / "native/Custom.framework", output / "Custom.framework")
        return output
