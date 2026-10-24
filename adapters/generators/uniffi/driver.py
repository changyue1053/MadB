"""Install and invoke the pinned UniFFI C06 toolchain without changing Cargo's registry."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tarfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
PATCHES = Path(__file__).with_name("patches")
RUST = "1.99.1"
MAX_ARCHIVE = 16 * 1024 * 1024
RECIPES = {
    "uniffi_core": ["owned-foreign-result-0.32.2", "native-ready-result-0.32.2"],
    "uniffi_bindgen": ["swift-cancellation-0.32.2", "swift-checked-initialization-0.32.2", "swift-foreign-sendable-0.32.2"],
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inventory(root):
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError(f"UniFFI inventory root must be a real directory: {root}")
    result = []
    for path in sorted(root.rglob("*")):
        mode = path.lstat().st_mode
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            raise ValueError(f"UniFFI inventory refuses symlinks/special files: {path}")
        result.append({"path": str(path.relative_to(root)), "sha256": digest(path)})
    return result


def inventory_digest(files):
    return hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _environment(extra=None):
    env = os.environ.copy()
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        env.pop(key, None)
    env["RUSTUP_TOOLCHAIN"] = RUST
    env.update(extra or {})
    return env


def cargo_patches(tools):
    root = Path(tools["root"])
    return [part for name in RECIPES for part in
            ("--config", f'patch.crates-io.{name}.path="{root / "sources" / (name + "-0.32.2")}"')]


def metadata_inputrefs(repo):
    # Bind the complete staged local source closure, including future mod/include
    # files. Only build outputs and Finder metadata are excluded during staging.
    repo = Path(repo)
    paths = []
    for name in ("uniffi", "boundary-core"):
        source = repo / "sdk/rust" / name
        for path in source.rglob("*"):
            if "target" in path.relative_to(source).parts or path.name == ".DS_Store":
                continue
            mode = path.lstat().st_mode
            if stat.S_ISDIR(mode):
                continue
            if not stat.S_ISREG(mode):
                raise ValueError(f"UniFFI author source refuses symlinks/special files: {path}")
            paths.append(path)
    return [{"path": str(path.relative_to(repo)), "sha256": "sha256:" + digest(path),
             "size": path.stat().st_size} for path in sorted(paths)]


def verify(tools):
    root = Path(tools["root"])
    if digest(root / "receipt.json") != tools["receipt_sha256"]:
        raise ValueError("UniFFI toolchain receipt mismatch")
    receipt = json.loads((root / "receipt.json").read_text())
    if set(receipt["sources"]) != set(RECIPES):
        raise ValueError("UniFFI toolchain source set differs from the controlled recipes")
    for name, item in receipt["sources"].items():
        expected_patches = []
        for stem in RECIPES[name]:
            recipe = json.loads((PATCHES / (stem + ".json")).read_text())
            if recipe["source_sha256"] != item["source_sha256"]:
                raise ValueError(f"UniFFI installed upstream source is stale: {name}")
            if digest(PATCHES / (stem + ".patch")) != recipe["patch_sha256"]:
                raise ValueError(f"UniFFI controlled patch bytes mismatch: {stem}")
            expected_patches.append({"name": stem, "sha256": recipe["patch_sha256"]})
        if item["patches"] != expected_patches:
            raise ValueError(f"UniFFI installed source patches are stale: {name}")
        if digest(root / "downloads" / (name + "-0.32.2.crate")) != item["source_sha256"]:
            raise ValueError(f"UniFFI original archive mismatch: {name}")
        actual = inventory(root / "sources" / (name + "-0.32.2"))
        if inventory_digest(actual) != item["inventory_sha256"]:
            raise ValueError(f"UniFFI patched source inventory mismatch: {name}")
    if inventory_digest(inventory(root / "build-source")) != receipt["sdk_source_inventory_sha256"]:
        raise ValueError("UniFFI tool build-source inventory mismatch")
    if not receipt.get("metadata_inputrefs") or receipt.get("metadata_features") != []:
        raise ValueError("UniFFI metadata author provenance missing or unsupported")
    locks = receipt.get("sdk_cargo_locks", {})
    if not {"original_sha256", "derived_sha256", "original_size"} <= locks.keys():
        raise ValueError("UniFFI tool receipt predates complete dependency-lock provenance; reinstall")
    staged_inputs = metadata_inputrefs(root / "build-source")
    for item in staged_inputs:
        if item["path"] == "sdk/rust/uniffi/Cargo.lock":
            if item["sha256"] != "sha256:" + receipt["sdk_cargo_locks"]["derived_sha256"]:
                raise ValueError("UniFFI staged dependency lock mismatch")
            item["sha256"] = "sha256:" + receipt["sdk_cargo_locks"]["original_sha256"]
            item["size"] = receipt["sdk_cargo_locks"]["original_size"]
    if staged_inputs != receipt["metadata_inputrefs"]:
        raise ValueError("UniFFI metadata provenance differs from actual staged source closure")
    for name, item in receipt["binaries"].items():
        if digest(root / item["path"]) != item["sha256"]:
            raise ValueError(f"UniFFI tool image mismatch: {name}")
    return receipt


def _stage_sdk(repo, destination):
    """Rebind only the two verified patched crates; keep every dependency version locked.

    Cargo metadata in the original repository may resolve registry sources. Isolating
    this derived lock prevents tools and provider builds from editing each other's lock.
    """
    metadata_inputrefs(repo)  # Refuse indirect/special author files before copying.
    rust = destination / "sdk/rust"
    for name in ("uniffi", "boundary-core"):
        shutil.copytree(repo / "sdk/rust" / name, rust / name,
                        ignore=shutil.ignore_patterns("target", ".DS_Store"))
    lock = rust / "uniffi/Cargo.lock"
    inputs = metadata_inputrefs(destination)
    return rust / "uniffi", derive_lock(lock), inputs


def derive_lock(lock):
    """Rebind verified patched source identities in a scratch Cargo.lock only."""
    lock = Path(lock)
    original = lock.read_text()
    packages = original.split("\n[[package]]\n")
    rewritten = [packages[0]]
    changed = set()
    for section in packages[1:]:
        package = tomllib.loads(section)
        name = package.get("name")
        if name in RECIPES:
            spec = json.loads((PATCHES / (RECIPES[name][0] + ".json")).read_text())
            if package.get("version") != "0.32.2":
                raise ValueError("Patched runtime version differs from the frozen recipe")
            if package.get("source") != "registry+https://github.com/rust-lang/crates.io-index" or package.get("checksum") != spec["source_sha256"]:
                raise ValueError("Original SDK dependency lock must name the verified upstream archive")
            section = "\n".join(line for line in section.split("\n")
                                if not line.startswith(("source =", "checksum =")))
            changed.add(name)
        rewritten.append(section)
    if changed != set(RECIPES):
        raise ValueError("Patched dependencies missing from SDK lock")
    lock.write_text("\n[[package]]\n".join(rewritten))
    return {"original_sha256": hashlib.sha256(original.encode()).hexdigest(),
                              "derived_sha256": digest(lock), "original_size": len(original.encode())}


def install(destination, repo=ROOT, archive_directory=None):
    """Fresh source extraction + managed patches + locked real generator/helper build.

    An optional explicit archive directory supports offline installation. It must
    contain the two original name-version.crate files; each is still verified.
    """
    root, repo = Path(destination).resolve(), Path(repo).resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / "sources").mkdir()
    (root / "downloads").mkdir()
    sources = {}
    for name, stems in RECIPES.items():
        spec = json.loads((PATCHES / (stems[0] + ".json")).read_text())
        archive = root / "downloads" / (name + "-0.32.2.crate")
        if archive_directory is not None:
            shutil.copyfile(Path(archive_directory) / archive.name, archive)
        else:
            with urllib.request.urlopen(spec["source_url"], timeout=60) as response:
                content = response.read(MAX_ARCHIVE + 1)
            if len(content) > MAX_ARCHIVE:
                raise ValueError(f"UniFFI source archive exceeds byte budget: {name}")
            archive.write_bytes(content)
        if archive.stat().st_size > MAX_ARCHIVE:
            raise ValueError(f"UniFFI source archive exceeds byte budget: {name}")
        if digest(archive) != spec["source_sha256"]:
            raise ValueError(f"UniFFI source archive mismatch: {name}")
        with tarfile.open(archive) as package:
            package.extractall(root / "sources", filter="data")
        source = root / "sources" / (name + "-0.32.2")
        patch_records = []
        for stem in stems:
            patch_spec = json.loads((PATCHES / (stem + ".json")).read_text())
            if patch_spec["source_sha256"] != spec["source_sha256"]:
                raise ValueError(f"UniFFI patch targets another upstream source: {stem}")
            patch = PATCHES / (stem + ".patch")
            if digest(patch) != patch_spec["patch_sha256"]:
                raise ValueError(f"UniFFI controlled patch mismatch: {stem}")
            subprocess.run(["/usr/bin/patch", "-p1", "--batch", "--forward", "-i", str(patch)],
                           cwd=source, check=True, timeout=30)
            patch_records.append({"name": stem, "sha256": patch_spec["patch_sha256"]})
        files = inventory(source)
        sources[name] = {"source_url": spec["source_url"], "source_sha256": spec["source_sha256"],
                         "patches": patch_records, "inventory_sha256": inventory_digest(files),
                         "files": files}
    tools = {"root": str(root)}
    staged, locks, inputs = _stage_sdk(repo, root / "build-source")
    command = ["cargo", "+" + RUST, "build", "--locked", "--release", "--bins", "--manifest-path",
               str(staged / "Cargo.toml"), "--target-dir", str(root / "build"), *cargo_patches(tools)]
    subprocess.run(command, cwd=repo, env=_environment(), check=True, timeout=600)
    (root / "bin").mkdir()
    binaries = {}
    for name in ("uniffi-bindgen", "madb-uniffi-metadata"):
        target = root / "bin" / name
        shutil.copy2(root / "build/release" / name, target)
        binaries[name] = {"path": "bin/" + name, "sha256": digest(target)}
    receipt = {"schema_version": "1.0.0", "rust": RUST, "sources": sources, "binaries": binaries,
               "sdk_cargo_locks": locks, "sdk_source_inventory_sha256": inventory_digest(inventory(root / "build-source")),
               "metadata_inputrefs": inputs, "metadata_features": []}
    (root / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    tools.update({"receipt_sha256": digest(root / "receipt.json"),
                  "bindgen": str(root / "bin/uniffi-bindgen"),
                  "metadata": str(root / "bin/madb-uniffi-metadata")})
    verify(tools)
    return tools


def build_sdk(repo, tools, output, profile_digest, test_support=False, environment=None):
    """Build the Release provider; the caller owns offline/vendor and target gate policy."""
    verify(tools)
    repo, output = Path(repo).resolve(), Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    staged, locks, _inputs = _stage_sdk(repo, output / "source")
    env = _environment(environment)
    env["MADB_PROFILE_DIGEST"] = profile_digest
    env["RUSTFLAGS"] = " ".join([
        "--remap-path-prefix=" + str(repo) + "=/madb/source",
        "--remap-path-prefix=" + str(output) + "=/madb/build",
        "--remap-path-prefix=" + tools["root"] + "=/madb/toolchain",
        "-C", "link-arg=-Wl,-install_name,@rpath/libmadb_boundary_uniffi.dylib",
        "-C", "link-arg=-Wl,-reproducible", "-C", "link-arg=-Wl,-S",
    ])
    command = ["cargo", "+" + RUST, "build", "--locked", "--release", "--lib", "--target", "aarch64-apple-darwin",
               "--manifest-path", str(staged / "Cargo.toml"), "--target-dir", str(output / "cargo"), *cargo_patches(tools)]
    if test_support:
        command.extend(["--features", "test-support"])
    subprocess.run(command, cwd=repo, env=env, check=True, timeout=600)
    native = output / "native"
    native.mkdir()
    for name in ("libmadb_boundary_uniffi.dylib", "libmadb_boundary_uniffi.a"):
        shutil.copy2(output / "cargo/aarch64-apple-darwin/release" / name, native / name)
    return {"native": str(native), "library": str(native / "libmadb_boundary_uniffi.dylib"),
            "profile_digest": profile_digest, "test_support": test_support, "cargo_locks": locks, "files": inventory(native)}


def generate(repo, tools, output, language, source_udl=None, library=None):
    verify(tools)
    repo, output = Path(repo).resolve(), Path(output).resolve()
    source = Path(library or source_udl or repo / "sdk/rust/uniffi/src/boundary.udl").resolve()
    command = [tools["bindgen"], "generate", str(source), "--language", language, "--no-format",
               "--config", str(repo / "sdk/rust/uniffi/bindgen.toml"), "--out-dir", str(output)]
    if library:
        command.append("--metadata-no-deps")
    subprocess.run(command, cwd=repo / "sdk/rust/uniffi", env=_environment({"CARGO_NET_OFFLINE": "true"}), check=True, timeout=60)
    return inventory(output)
