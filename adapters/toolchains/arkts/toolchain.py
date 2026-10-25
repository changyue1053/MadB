"""Observe already installed tools; no download, installation or license action."""

from pathlib import Path
import hashlib
import importlib.util
import json
import os
import platform
import re
import subprocess
import sys

_spec = importlib.util.spec_from_file_location(
    "_c07_port", Path(__file__).resolve().parents[3] / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)


def check_distribution(metadata):
    suite = metadata["command_line_tools"]
    expected = {
        "version": "26.0.0.851",
        "distribution_platform": "mac-arm64",
        "releaseType": "release",
        "hvigor_version": "6.26.8",
        "ohpm_version": "26.0.0.630",
    }
    if any(suite.get(key) != value for key, value in expected.items()):
        raise ValueError("installed CLI distribution differs from the fixed C07 profile")
    product = metadata["sdk"]["product"]
    if not metadata["sdk"]["is_formal"] or any(
        product.get(k) != v
        for k, v in {"version": "26.0.0.105", "apiVersion": "26", "releaseType": "Release"}.items()
    ):
        raise ValueError("installed target SDK differs from the formal C07 profile")


def check_program_versions(config):
    def query(name, arguments):
        tool = config["tools"][name]
        support.verify_tool(tool)
        value = subprocess.check_output(
            [tool["path"], *arguments], text=True, timeout=30, env=support.clean_environment()
        ).strip()
        support.verify_tool(tool)
        return value

    node = query("node", ["--version"])
    clang = query("clang", ["--version"])
    ts = query(
        "node",
        [
            "-e",
            "process.stdout.write(require(process.argv[1]).version)",
            config["tools"]["typescript"]["path"],
        ],
    )
    package = support.read_json(
        Path(config["tools"]["typescript"]["path"]).parents[1] / "package.json"
    )
    if node != "v24.14.1" or ts != "4.9.5" or package.get("version") != "4.9.5-r4":
        raise ValueError("actual Node/TypeScript version differs from fixed C07 profile")
    if re.search(r"clang version 15\.0\.4(?: |\n|$)", clang) is None:
        raise ValueError("actual Clang version differs from fixed C07 profile")
    return {
        "node": node,
        "clang": clang,
        "typescript_api": ts,
        "typescript_package": package["version"],
    }


def observe(repo, cli_root, inventory_ref, python_image, java_home, finalizer_image, madb_image):
    repo = Path(repo)
    cli = Path(cli_root)
    native = cli / "sdk/default/openharmony/native"
    java_home = Path(java_home)
    inventory_path = support.verify_tool(inventory_ref)
    inventory = support.read_json(inventory_path, 64 * support.MAX_JSON)
    checked = support.load_module(repo / "adapters/toolchains/arkts/tools.py").verify_inventory(
        inventory, cli
    )
    check_distribution(checked["metadata"])
    trees = support.load_module(repo / "adapters/toolchains/kotlin_rust/tools.py")
    python_runtime = support.load_module(repo / "adapters/toolchains/kotlin_rust/python_runtime.py")
    config = {"tools": {}}

    def register(name, path, version, source):
        path = Path(path)
        config["tools"][name] = {
            "path": str(path),
            "sha256": support.sha(path),
            "size": path.stat().st_size,
            "id": "c07-" + name,
            "version": version,
            "source": source,
        }

    source = "caller-reviewed local archive; independently verified installed tree; not a vendor signature"
    for name, relative, version in [
        ("node", "tool/node/bin/node", "24.14.1"),
        (
            "typescript",
            "sdk/default/openharmony/ets/build-tools/ets-loader/node_modules/typescript/lib/typescript.js",
            "4.9.5-r4",
        ),
        ("clang", "sdk/default/openharmony/native/llvm/bin/clang", "15.0.4"),
        ("clangxx", "sdk/default/openharmony/native/llvm/bin/clang++", "15.0.4"),
        ("readelf", "sdk/default/openharmony/native/llvm/bin/llvm-readelf", "15.0.4"),
        ("ohpm", "ohpm/bin/ohpm", "26.0.0.630"),
        ("hvigor", "hvigor/hvigor/bin/hvigor.js", "6.26.8"),
    ]:
        register(name, cli / relative, version, source)
    program_versions = check_program_versions(config)
    image = Path(python_image).resolve(strict=True)
    runtime = python_runtime.describe(image, environment=support.clean_environment())
    register(
        "python",
        image,
        runtime["version"],
        "existing isolated local Python runtime; separately closed tree",
    )
    config["tools"]["python"]["invocation_arguments"] = python_runtime.ISOLATION
    register(
        "finalizer",
        Path(finalizer_image),
        "0.1.0",
        "caller-reviewed MadB finalizer image; exact bytes, no source attestation",
    )
    register(
        "madb-host",
        Path(madb_image),
        "0.1.0",
        "caller-reviewed MadB owner image; exact bytes, no source attestation",
    )
    otool = Path(
        subprocess.check_output(
            ["/usr/bin/xcrun", "--find", "otool"],
            text=True,
            timeout=30,
            env=support.clean_environment(),
        ).strip()
    ).resolve(strict=True)
    register("otool", otool, "host-xcode-tool", "local platform linkage reader; exact image hash")
    register(
        "java",
        java_home / "bin/java",
        "host-jbr",
        "existing DevEco host JBR; separate from Harmony target SDK",
    )
    runtime.update(
        python_runtime.native_dependencies(
            otool, runtime["root"], image, environment=support.clean_environment()
        )
    )
    runtime["system_build"] = subprocess.check_output(
        ["/usr/bin/sw_vers", "-buildVersion"],
        text=True,
        timeout=30,
        env=support.clean_environment(),
    ).strip()
    closure = {
        "python_runtime": runtime,
        "trees": {
            "python-runtime": trees.snapshot(runtime["root"], python_runtime.SCOPE),
            "host-jbr": trees.snapshot(java_home),
        },
    }
    config.update(
        cli_root=str(cli),
        native_sdk=str(native),
        java_home=str(java_home),
        runtime_closure=closure,
        seam_arguments=[
            "--target=aarch64-linux-ohos23.0.0",
            "--sysroot=" + str(native / "sysroot"),
            "-std=c17",
            "-O2",
            "-fPIC",
            "-fvisibility=hidden",
        ],
    )
    command = [
        config["tools"]["clangxx"]["path"],
        *config["seam_arguments"][:2],
        "-print-file-name=libc++_shared.so",
    ]
    runtime_lib = Path(
        subprocess.check_output(
            command, text=True, timeout=30, env=support.clean_environment()
        ).strip()
    ).resolve(strict=True)
    if not runtime_lib.is_relative_to(native):
        raise ValueError("C++ runtime resolved outside fixed Native SDK")
    native_runtime = {
        "soname": "libc++_shared.so",
        "path": str(runtime_lib),
        "sha256": support.sha(runtime_lib),
        "size": runtime_lib.stat().st_size,
    }
    public = {
        "scope": "exact installed host/tool observations; no C07 or target certification",
        "archive": checked["archive"],
        "cli_entries_sha256": checked["entries_sha256"],
        "metadata": checked["metadata"],
        "program_versions": program_versions,
        "tools": [
            {k: t[k] for k in ("id", "version", "sha256", "size", "source")}
            for _, t in sorted(config["tools"].items())
        ],
        "runtime_trees": [
            {k: v for k, v in tree.items() if k != "root"} | {"name": name}
            for name, tree in sorted(closure["trees"].items())
        ],
        "python_runtime": {k: runtime[k] for k in ("version", "system_build", "system_libraries")},
        "system": {"platform": platform.platform(), "build": runtime["system_build"]},
        "native_runtime": {k: native_runtime[k] for k in ("soname", "sha256", "size")},
    }
    return {
        "inventory": inventory_ref,
        "config": config,
        "public": public,
        "native_runtime": native_runtime,
    }


def verify_runtime(config, root):
    root = Path(root)
    trees = support.load_module(root / "adapters/toolchains/kotlin_rust/tools.py")
    py = support.load_module(root / "adapters/toolchains/kotlin_rust/python_runtime.py")
    closure = config["runtime_closure"]
    for name, tree in closure["trees"].items():
        if trees.snapshot(tree["root"], tree.get("scope")) != tree:
            raise ValueError("host runtime tree changed: " + name)
    py.verify({"tools": config["tools"], "python_runtime": closure["python_runtime"]})
    if (
        subprocess.check_output(
            ["/usr/bin/sw_vers", "-buildVersion"],
            text=True,
            timeout=30,
            env=support.clean_environment(),
        ).strip()
        != closure["python_runtime"]["system_build"]
    ):
        raise ValueError("host OS shared-cache build changed")
