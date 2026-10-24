"""Observe and verify the exact external images used by the four C06 routes."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import tomllib

from .install_tools import RECIPE, digest, inventory, verify as verify_managed
from . import python_runtime


def load_verifier(name):
    """Load the source-frozen adapter, never an ambient adapters package/cache."""
    if name not in ("gobley", "uniffi"):
        raise ValueError("unsupported installation verifier")
    path = Path(__file__).resolve().parents[3] / "adapters/generators" / name / "driver.py"
    identity = hashlib.sha256(str(path).encode()).hexdigest()
    spec = importlib.util.spec_from_file_location("madb_c06_verifier_" + identity, path)
    if spec is None or spec.loader is None:
        raise ValueError("cannot load frozen installation verifier")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def query(*arguments):
    return subprocess.check_output(list(map(str, arguments)), text=True, timeout=30).strip()


def snapshot(root, scope=None):
    """Compact commitment to every file; C04 need not embed 30,000 SDK FileRefs."""
    root = Path(root).resolve(strict=True)
    if not root.is_dir():
        raise ValueError("tool dependency tree is not a directory")
    if scope not in (None, python_runtime.SCOPE):
        raise ValueError("unknown tool inventory scope")
    files = python_runtime.inventory(root) if scope else inventory(root)
    encoded = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    result = dict(root=str(root), sha256="sha256:" + hashlib.sha256(encoded).hexdigest(),
                  entries=len(files), size=sum(item.get("size", 0) for item in files))
    if scope:
        result["scope"] = scope
    return result


def register_image(observation, key, path, version, source):
    # Multi-call tools dispatch on argv[0] (swiftc and swift-symbolgraph-extract).
    # Keep their callable name while pinning the image behind any launcher link.
    launcher = Path(os.path.abspath(path))
    image = launcher.resolve(strict=True)
    if not image.is_file():
        raise ValueError("tool image is not a file: " + str(image))
    observation["tools"][key] = dict(path=str(launcher), image_path=str(image),
                                     invocation_name=launcher.name,
                                     launcher_link=os.readlink(launcher) if launcher.is_symlink() else None,
                                     id="c06-" + key.replace("_", "-"), version=version,
                                     sha256=digest(image), size=image.stat().st_size, source=source)
    return observation["tools"][key]


def register_tree(observation, key, path):
    observation["trees"][key] = snapshot(path)
    return observation["trees"][key]


def register_dependencies(observation, key, root, lock):
    root, lock = Path(root).resolve(strict=True), Path(lock).resolve(strict=True)
    data = json.loads(lock.read_text())
    if data.get("schema") != "madb.maven-files/1":
        raise ValueError("expected reviewed Maven file lock")
    names = {item["path"] for item in data["files"]}
    actual = {path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file() or path.is_symlink()}
    if actual != names or len(names) != len(data["files"]):
        raise ValueError("dependency tree differs from its closed inventory: " + key)
    for item in data["files"]:
        path = root / item["path"]
        if path.is_symlink() or not path.resolve().is_relative_to(root) or digest(path) != "sha256:" + item["sha256"] or path.stat().st_size != item["size"]:
            raise ValueError("dependency bytes differ from reviewed lock: " + item["path"])
    observation["dependency_locks"][key] = dict(path=str(lock), sha256=digest(lock), size=lock.stat().st_size)
    register_tree(observation, key, root)


def discover(repo, managed_root, gobley_root=None, runtime_root=None, maven_root=None, cargo_root=None):
    repo, managed_root = Path(repo).resolve(strict=True), Path(managed_root).resolve(strict=True)
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        raise ValueError("C06 complete profiles currently require macOS Arm64")
    managed = verify_managed(managed_root)
    recipe = json.loads(RECIPE.read_text())
    apple = recipe["xcode"]
    if query("xcodebuild", "-version") != "Xcode " + apple["version"] + "\nBuild version " + apple["build"]:
        raise ValueError("Xcode differs from the reviewed C06 profile")
    if query("xcrun", "--show-sdk-version") != apple["sdk_version"] or query("xcrun", "--show-sdk-build-version") != apple["sdk_build"]:
        raise ValueError("Apple SDK differs from the reviewed C06 profile")
    swiftc = Path(query("xcrun", "--find", "swiftc")).absolute()
    if apple["swift"] not in query(swiftc, "--version"):
        raise ValueError("Swift compiler differs from the reviewed C06 profile")
    rust = tomllib.loads((repo / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    rustc = Path(query("rustup", "which", "--toolchain", rust, "rustc")).resolve(strict=True)
    cargo = Path(query("rustup", "which", "--toolchain", rust, "cargo")).resolve(strict=True)
    if query(rustc, "--version").split()[1] != rust:
        raise ValueError("Rust compiler differs from the repository toolchain")
    target = next(line.split(": ", 1)[1] for line in query(rustc, "-vV").splitlines() if line.startswith("host: "))
    if target != recipe["host"]:
        raise ValueError("Rust host differs from C06 profile")
    sdk = Path(query("xcrun", "--show-sdk-path")).resolve(strict=True)
    developer = Path(query("xcode-select", "-p")).resolve(strict=True)
    result = dict(schema="madb.managed-observation/1", target=target, system=platform.system(),
                  tools={}, trees={}, dependency_locks={}, managed=managed, gobley=None, sdk=str(sdk),
                  xcode=apple, deployment_target="14.0", environment=dict(
                      JAVA_HOME=str(managed_root / "jdk/Contents/Home"),
                      KONAN_DATA_DIR=str(managed_root / "konan"), DEVELOPER_DIR=str(developer),
                      LC_ALL="C", RUSTUP_TOOLCHAIN=rust))
    register_image(result, "python", Path(os.sys.executable).resolve(strict=True), platform.python_version(), "system://observed-python")
    register_image(result, "rustc", rustc, rust, "rustup://" + rust)
    register_image(result, "cargo", cargo, rust, "rustup://" + rust)
    register_image(result, "swiftc", swiftc, apple["swift"], "xcode://" + apple["build"])
    for key, command in [("otool", "otool"), ("swift_frontend", "swift-frontend"), ("swift_symbolgraph_extract", "swift-symbolgraph-extract"), ("clang", "clang"), ("ar", "ar"), ("nm", "nm"), ("ld", "ld")]:
        register_image(result, key, query("xcrun", "--find", command), apple["build"], "xcode://" + apple["build"])
    for key, component, executable in [("java", "jdk", "Contents/Home/bin/java"), ("gradle", "gradle", "bin/gradle"),
                                        ("kotlinc", "kotlin-jvm", "bin/kotlinc"), ("konanc", "kotlin-native", "bin/konanc"),
                                        ("cinterop", "kotlin-native", "bin/cinterop")]:
        if component in managed["paths"]:
            spec = recipe["archives"][component]
            register_image(result, key, Path(managed["paths"][component]) / executable, spec["version"], spec["source"])
    python = result["tools"]["python"]
    python["invocation_arguments"] = list(python_runtime.ISOLATION)
    runtime = python_runtime.describe(python["path"])
    runtime.update(python_runtime.native_dependencies(result["tools"]["otool"]["path"], runtime["root"], python["path"]))
    runtime["system_build"] = query("/usr/bin/sw_vers", "-buildVersion")
    result["python_runtime"] = runtime
    result["trees"]["python-runtime"] = snapshot(runtime["root"], python_runtime.SCOPE)
    roots = set()
    for library in runtime["libraries"]:
        parent = Path(library["image_path"]).parent
        if parent not in roots:
            roots.add(parent)
            register_tree(result, "python-native-" + Path(library["image_path"]).name, parent)
    register_tree(result, "apple-sdk", sdk)
    register_tree(result, "apple-toolchain-libraries", swiftc.parent.parent / "lib")
    register_tree(result, "rust-sysroot-libraries", Path(query(rustc, "--print", "sysroot")) / "lib")
    if gobley_root is not None:
        verify_install = load_verifier("gobley").verify_install
        gobley_root = Path(gobley_root).resolve(strict=True)
        verify_install(gobley_root)
        receipt = gobley_root / "receipt.json"
        result["gobley"] = dict(root=str(gobley_root), receipt_sha256=digest(receipt))
        register_image(result, "gobley", gobley_root / "gobley-patched", "0.3.7-madb", "managed://gobley-source-and-patch")
        register_tree(result, "gobley-runtime", gobley_root / "uniffi_core-0.29.4-madb")
    if runtime_root is not None:
        register_dependencies(result, "runtime-dependencies", runtime_root, RECIPE.with_name("runtime-dependencies.json"))
    if maven_root is not None:
        register_dependencies(result, "skie-maven", maven_root, RECIPE.with_name("skie-dependencies.json"))
    if cargo_root is not None:
        register_tree(result, "cargo-vendor", cargo_root)
    verify(result)
    return result


def verify(observation):
    """Verify before and after each stage, including library and dependency bytes."""
    managed = observation["managed"]
    verify_managed(managed["root"], managed["receipt_sha256"])
    for tool in observation["tools"].values():
        path, image = Path(tool["path"]), Path(tool["image_path"])
        link = os.readlink(path) if path.is_symlink() else None
        if (not path.is_absolute() or path.name != tool["invocation_name"]
                or link != tool["launcher_link"] or not path.is_file()
                or path.resolve(strict=True) != image or image.is_symlink()
                or image.stat().st_size != tool["size"] or digest(image) != tool["sha256"]):
            raise ValueError("observed tool image or launcher changed: " + tool["id"])
    for key, tree in observation["trees"].items():
        if snapshot(tree["root"], tree.get("scope")) != tree:
            raise ValueError("observed tool/dependency tree changed: " + key)
    for key, lock in observation["dependency_locks"].items():
        if digest(lock["path"]) != lock["sha256"] or Path(lock["path"]).stat().st_size != lock["size"]:
            raise ValueError("dependency lock changed: " + key)
    if "python" in observation["tools"]:
        python_runtime.verify(observation)
    if observation["gobley"] is not None:
        verify_install = load_verifier("gobley").verify_install
        installed = observation["gobley"]
        if digest(Path(installed["root"]) / "receipt.json") != installed["receipt_sha256"]:
            raise ValueError("Gobley installation receipt changed")
        verify_install(Path(installed["root"]))
    return observation
