"""Explicit CPF input closure. Never resolves dependencies during frozen execution."""

from __future__ import annotations
import hashlib
import importlib.util
import json
import os
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_kn_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)

NAMES = {
    "gradle": "gradle-8.14.6",
    "jdk": "jdk-21.0.5+11",
    "compiler": "kotlin-native-prebuilt-macos-aarch64-2.2.21-1.1.0",
    "llvm": "cpf-llvm-19-aarch64-macos-dev-21",
    "ohos": "sysroot-ohos-aarch64-6.0.2.640-04",
    "hms": "sysroot-hms-aarch64-6.0.2.640-02",
    "ffi": "libffi-3.3-1-macos-arm64",
}
RUNTIME = "AKInterop/akinterop-runtime/src/"
SOURCE_PIN = "adapters/generators/akinterop/managed-owned-r6-source.json"
PATCH = "adapters/generators/akinterop/patches/managed-owned-r6.patch"
HEADERS = (
    "napi/native_api.h",
    "napi/common.h",
    "node_api.h",
    "node_api_types.h",
    "js_native_api.h",
    "js_native_api_types.h",
    "info/application_target_sdk_version.h",
    "hilog/log.h",
)


def ref(path):
    path = Path(path)
    return dict(path=str(path), sha256=support.sha(path), size=path.stat().st_size)


def verify_file(item):
    path = Path(item["path"])
    if not path.is_absolute() or ref(path) != {k: item[k] for k in ("path", "sha256", "size")}:
        raise ValueError("Kotlin external input changed: " + str(path))
    return path


def verify_tree(tree):
    """Public extraction manifests preserve symlink targets; they are not vendor signatures."""
    manifest = verify_file(tree["manifest"])
    data = support.read_json(manifest, 16 * support.MAX_JSON)
    root = Path(tree["root"])
    if not root.is_absolute() or root.is_symlink():
        raise ValueError("Kotlin tool tree must have an explicit physical root")
    entries = data.get("files", data.get("entries"))
    paths = set()
    for e in entries:
        name = e["path"]
        if (
            name in paths
            or Path(name).is_absolute()
            or any(s in ("", ".", "..") for s in name.split("/"))
        ):
            raise ValueError("Invalid/duplicate public tool entry")
        paths.add(name)
        p = root / name
        target = e.get("symlink", e.get("target") if e.get("kind") == "symlink" else None)
        if target is not None:
            if (
                not p.is_symlink()
                or os.readlink(p) != target
                or not p.resolve().is_relative_to(root.resolve())
            ):
                raise ValueError("Kotlin tool symlink differs/escapes: " + name)
        elif e.get("kind") == "directory":
            if not p.is_dir() or p.is_symlink():
                raise ValueError("Tool directory changed: " + name)
        else:
            if (
                p.is_symlink()
                or not p.is_file()
                or p.stat().st_size != e.get("size", e.get("bytes"))
                or support.sha(p) != "sha256:" + e["sha256"]
            ):
                raise ValueError("Kotlin public tool bytes differ: " + name)
    actual = {
        p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() or p.is_symlink()
    }
    expected = {e["path"] for e in entries if e.get("kind") != "directory"}
    if actual != expected:
        raise ValueError("Kotlin tool tree contains undeclared files")
    summary = dict(
        sha256=support.sha(manifest),
        entries=len(entries),
        size=sum(e.get("size", e.get("bytes", 0)) for e in entries),
    )
    if tree["summary"] != summary:
        raise ValueError("Kotlin tree summary differs")
    return data


def verify(inputs, config):
    selected = config["kotlin"]
    lockpath = verify_file(selected["dependency_lock"])
    lock = support.read_json(lockpath)
    maven = Path(selected["maven_root"])
    expected = set()
    for e in lock["maven"]:
        name = e["relative_path"]
        expected.add(name)
        p = support.input_path(maven, name)
        if p.stat().st_size != e["bytes"] or support.sha(p) != "sha256:" + e["sha256"]:
            raise ValueError("Kotlin Maven artifact changed: " + name)
    actual = {p.relative_to(maven).as_posix() for p in maven.rglob("*") if p.is_file()}
    if actual != expected:
        raise ValueError("Kotlin Maven closure has missing/extra files")
    for tree in selected["trees"].values():
        verify_tree(tree)
    pin = support.read_json(support.input_path(inputs, SOURCE_PIN))
    if (
        pin["base_commit"] != "75ba50c77afda2845a3a6c51cdcddb03d4fd0ab8"
        or support.sha(support.input_path(inputs, PATCH)) != "sha256:" + pin["patch_sha256"]
    ):
        raise ValueError("Kotlin managed source identity differs")
    support.verify_refs(
        inputs,
        [
            dict(
                path="upstream/akinterop/" + e["path"],
                sha256="sha256:" + e["sha256"],
                size=e["size"],
            )
            for e in pin["base_files"]
        ],
    )
    return lock


def review_configuration(public_root, upstream, compiler_workspaces, *, python_tool):
    """Preflight helper over already acquired original inputs; no download/cache copying."""
    public_root, upstream = Path(public_root), Path(upstream)
    lock = ref(public_root / "offline-input-lock.json")
    trees = {}
    for key, name in NAMES.items():
        p = public_root / (name + "-public-tree.json")
        d = support.read_json(p, 16 * support.MAX_JSON)
        entries = d.get("files", d.get("entries"))
        trees[key] = dict(
            root=str(public_root / "toolchains" / name),
            manifest=ref(p),
            summary=dict(
                sha256=support.sha(p),
                entries=len(entries),
                size=sum(e.get("size", e.get("bytes", 0)) for e in entries),
            ),
        )
    paths = {
        "cpf_java": ("jdk", "Contents/Home/bin/java", "21.0.5+11"),
        "gradle_cli": ("gradle", "lib/gradle-gradle-cli-main-8.14.6.jar", "8.14.6"),
        "cpf_clang": ("llvm", "bin/clang++", "19-dev-21"),
        "cpf_ar": ("llvm", "bin/llvm-ar", "19-dev-21"),
        "cpf_ld": ("llvm", "bin/ld.lld", "19-dev-21"),
        "cpf_readelf": ("llvm", "bin/llvm-readelf", "19-dev-21"),
    }
    tools = {
        k: {
            **ref(Path(trees[tree]["root"]) / name),
            "id": "c07-kotlin-" + k,
            "version": version,
            "source": "independently downloaded public archive; original bytes and extraction manifest",
        }
        for k, (tree, name, version) in paths.items()
    }
    tools["python"] = python_tool
    return dict(
        tools=tools,
        kotlin=dict(
            provider="kotlin-owned",
            version="2.2.21-1.1.0",
            akinterop="1.1.0",
            upstream_root=str(upstream),
            maven_root=str(public_root / "public-maven"),
            dependency_lock=lock,
            trees=trees,
            compiler_workspace=dict(
                strategy="input-addressed-exclusive-cold-build", root=str(compiler_workspaces)
            ),
        ),
    )
