"""Prepare a verified offline Cargo source closure for the C06 Rust SDKs."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.generators.uniffi import driver as uniffi
from adapters.generators.gobley import driver as gobley
from adapters.toolchains.kotlin_rust.tools import snapshot

RUST = "1.99.1"
REGISTRY_SOURCES = {"registry+https://github.com/rust-lang/crates.io-index", "registry+sparse+https://index.crates.io/"}
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
PATCHED_PACKAGES = {"uniffi_core-0.32.2", "uniffi_bindgen-0.32.2", "uniffi_core-0.29.4"}


def digest(path):
    hash_ = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            hash_.update(block)
    return "sha256:" + hash_.hexdigest()


def reference(repo, name):
    path = repo / name
    return dict(path=name, sha256=digest(path), size=path.stat().st_size)


def sources(repo):
    result = {}
    for backend in ("uniffi", "gobley"):
        lock = tomllib.loads((repo / f"sdk/rust/{backend}/Cargo.lock").read_text())
        for package in lock["package"]:
            source = package.get("source")
            if source is None:
                continue
            if source not in REGISTRY_SOURCES or not isinstance(package.get("checksum"), str):
                raise ValueError("C06 requires exact registry source checksums: " + package["name"])
            key = package["name"] + "-" + package["version"]
            item = dict(name=package["name"], version=package["version"], source=source, sha256="sha256:" + package["checksum"])
            if key in result and result[key] != item:
                raise ValueError("two SDK locks disagree on registry identity: " + key)
            result[key] = item
    return result


def _files(root):
    result = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError("vendored source must not contain a symlink: " + str(path))
        if path.is_file():
            result[path.relative_to(root).as_posix()] = digest(path)
    return result


def _merge(first, second, destination):
    destination.mkdir()
    for tree in (first, second):
        for package in sorted(tree.iterdir()):
            if package.is_symlink() or not package.is_dir():
                raise ValueError("unexpected Cargo vendor root entry: " + str(package))
            target = destination / package.name
            if target.exists():
                if _files(target) != _files(package):
                    raise ValueError("duplicate Cargo package bytes disagree: " + package.name)
            else:
                shutil.copytree(package, target)


def _archive(item, cache, cargo_home):
    name = item["name"] + "-" + item["version"] + ".crate"
    target = cache / name
    if not target.exists():
        candidates = sorted((cargo_home / "registry/cache").glob("*/" + name)) if cargo_home.exists() else []
        cached = next((path for path in candidates if digest(path) == item["sha256"]), None)
        if cached is not None:
            shutil.copyfile(cached, target)
        else:
            url = f"https://static.crates.io/crates/{item['name']}/{name}"
            temporary = target.with_suffix(".partial")
            try:
                with urllib.request.urlopen(url, timeout=90) as response, temporary.open("xb") as output:
                    size = 0
                    while chunk := response.read(1024 * 1024):
                        size += len(chunk)
                        if size > MAX_ARCHIVE_BYTES:
                            raise ValueError("registry source archive exceeds limit: " + name)
                        output.write(chunk)
                if digest(temporary) != item["sha256"]:
                    raise ValueError("registry source archive checksum mismatch: " + name)
                temporary.replace(target)
            finally:
                temporary.unlink(missing_ok=True)
    if target.is_symlink() or digest(target) != item["sha256"]:
        raise ValueError("registry archive cache checksum mismatch: " + name)
    return target


def _verify_package(package, item, archive):
    checksum = json.loads((package / ".cargo-checksum.json").read_text())
    if checksum["package"] != item["sha256"].removeprefix("sha256:"):
        raise ValueError("vendored package does not match Cargo.lock: " + package.name)
    expected = {}
    prefix = item["name"] + "-" + item["version"] + "/"
    with tarfile.open(archive) as source:
        for member in source:
            if member.isdir():
                continue
            if not member.isfile() or not member.name.startswith(prefix):
                raise ValueError("unexpected registry archive entry: " + member.name)
            relative = member.name[len(prefix):]
            if not relative or any(part in ("", ".", "..") for part in relative.split("/")) or relative in expected:
                raise ValueError("unsafe or duplicate registry archive member")
            stream = source.extractfile(member)
            hash_ = hashlib.sha256()
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                hash_.update(chunk)
            # Cargo's vendor_this filter omits VCS control and unpack markers.
            # https://doc.rust-lang.org/stable/nightly-rustc/src/cargo/ops/cargo_vendor.rs.html
            parts = relative.split("/")
            if ".git" not in parts and parts[-1] not in (".gitignore", ".gitattributes") and relative != ".cargo-ok":
                expected[relative] = "sha256:" + hash_.hexdigest()
    actual = _files(package)
    actual.pop(".cargo-checksum.json", None)
    # Registry archives include the normalized manifest Cargo consumes. Vendor
    # output must preserve those source bytes, not merely invent new checksums.
    if actual != expected:
        mismatch = sorted(name for name in set(actual) | set(expected) if actual.get(name) != expected.get(name))
        raise ValueError("vendored source differs from verified registry archive: " + package.name + " " + ", ".join(mismatch[:8]))
    checksums = {name: "sha256:" + value for name, value in checksum["files"].items()}
    if checksums != actual:
        raise ValueError("Cargo vendor file checksum inventory is incomplete: " + package.name)
    manifest = tomllib.loads((package / "Cargo.toml").read_text())["package"]
    if manifest["name"] != item["name"] or manifest["version"] != item["version"]:
        raise ValueError("vendored Cargo manifest identity mismatch")
    if not manifest.get("license") and not manifest.get("license-file"):
        raise ValueError("registry package has no declared redistribution license: " + package.name)
    return dict(**item, archive_url=f"https://static.crates.io/crates/{item['name']}/{archive.name}", license=manifest.get("license"), license_file=manifest.get("license-file"))


def _uniffi_tools(root):
    value = dict(root=str(root), receipt_sha256=digest(root / "receipt.json").removeprefix("sha256:"), bindgen=str(root / "bin/uniffi-bindgen"), metadata=str(root / "bin/madb-uniffi-metadata"))
    uniffi.verify(value)
    return value


def prepare(output, uniffi_root, gobley_root, archive_cache=None, repo=ROOT):
    output, repo = Path(output).resolve(), Path(repo).resolve(strict=True)
    uniffi_root, gobley_root = Path(uniffi_root).resolve(strict=True), Path(gobley_root).resolve(strict=True)
    if output.exists():
        raise ValueError("dependency output must not exist")
    source_names = [f"sdk/rust/{backend}/{name}" for backend in ("uniffi", "gobley") for name in ("Cargo.toml", "Cargo.lock")]
    before = [reference(repo, name) for name in source_names]
    canonical_sources = sources(repo)
    selected = {name: item for name, item in canonical_sources.items() if name not in PATCHED_PACKAGES}
    uniffi_tools = _uniffi_tools(uniffi_root)
    gobley.verify_install(gobley_root)
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))).expanduser().resolve()
    output.mkdir(parents=True)
    cache = Path(archive_cache).expanduser().resolve() if archive_cache else output / "archives"
    cache.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, RUSTUP_TOOLCHAIN=RUST)
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        environment.pop(name, None)
    with tempfile.TemporaryDirectory(prefix="madb-c06-vendor-") as temporary:
        temporary = Path(temporary)
        trees = []
        derivations = {}
        for crate in ("boundary-core", "uniffi", "gobley"):
            shutil.copytree(repo / "sdk/rust" / crate, temporary / "source/sdk/rust" / crate,
                            ignore=shutil.ignore_patterns("target", "__pycache__", ".DS_Store"))
        for backend in ("uniffi", "gobley"):
            patches = uniffi.cargo_patches(uniffi_tools) if backend == "uniffi" else ["--config", 'patch.crates-io.uniffi_core.path=' + json.dumps(str(gobley_root / "uniffi_core-0.29.4-madb"))]
            lock = temporary / f"source/sdk/rust/{backend}/Cargo.lock"
            derivations[backend] = (uniffi if backend == "uniffi" else gobley).derive_lock(lock)
            tree = temporary / backend
            command = ["cargo", "+" + RUST, *patches, "vendor", "--locked", "--versioned-dirs", "--manifest-path", str(temporary / f"source/sdk/rust/{backend}/Cargo.toml"), str(tree)]
            result = subprocess.run(command, cwd=repo, env=environment, text=True, capture_output=True, timeout=900)
            (output / (backend + "-vendor.log")).write_text(result.stdout + result.stderr)
            if result.returncode:
                raise RuntimeError("locked Cargo vendor failed for " + backend + ":\n" + result.stderr[-8000:])
            trees.append(tree)
        _merge(*trees, output / "vendor")
    packages = []
    actual_names = {path.name for path in (output / "vendor").iterdir()}
    if actual_names != set(selected):
        raise ValueError("vendor package set differs from the union of both SDK locks")
    for name, item in sorted(selected.items()):
        archive = _archive(item, cache, cargo_home)
        packages.append(_verify_package(output / "vendor" / name, item, archive))
    after = [reference(repo, name) for name in source_names]
    if before != after:
        raise RuntimeError("SDK manifests or Cargo.lock changed during dependency preparation")
    uniffi.verify(uniffi_tools)
    gobley.verify_install(gobley_root)
    tree = snapshot(output / "vendor")
    tree.pop("root")
    receipt = dict(schema="madb.cargo-vendor/1", rust=RUST, sdk_inputs=before, sources=packages, tree=tree, lock_derivations=derivations,
                   patched_packages=[canonical_sources[name] for name in sorted(PATCHED_PACKAGES)],
                   patched_sources=dict(uniffi_receipt_sha256=digest(uniffi_root / "receipt.json"), gobley_receipt_sha256=digest(gobley_root / "receipt.json")))
    (output / "receipt.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    verify(output / "vendor", repo)
    return receipt


def verify(vendor, repo=ROOT):
    vendor, repo = Path(vendor).resolve(strict=True), Path(repo).resolve(strict=True)
    receipt_path = vendor.parent / "receipt.json"
    receipt = json.loads(receipt_path.read_text())
    if receipt.get("schema") != "madb.cargo-vendor/1" or receipt.get("rust") != RUST:
        raise ValueError("unsupported Cargo vendor receipt")
    expected_names = {f"sdk/rust/{backend}/{name}" for backend in ("uniffi", "gobley") for name in ("Cargo.toml", "Cargo.lock")}
    if {item["path"] for item in receipt["sdk_inputs"]} != expected_names:
        raise ValueError("Cargo vendor receipt does not bind both SDK source locks")
    for item in receipt["sdk_inputs"]:
        if reference(repo, item["path"]) != item:
            raise ValueError("Cargo vendor receipt is stale for " + item["path"])
    actual = snapshot(vendor)
    actual.pop("root")
    if actual != receipt["tree"]:
        raise ValueError("Cargo vendor tree changed after source verification")
    expected_sources = {name: item for name, item in sources(repo).items() if name not in PATCHED_PACKAGES}
    actual_sources = {p["name"] + "-" + p["version"]: {key: p[key] for key in ("name", "version", "source", "sha256")} for p in receipt["sources"]}
    if actual_sources != expected_sources or len(actual_sources) != len(receipt["sources"]):
        raise ValueError("Cargo vendor registry identities differ from SDK locks")
    return dict(path=str(receipt_path), sha256=digest(receipt_path), size=receipt_path.stat().st_size)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--uniffi-tools", type=Path, required=True)
    parser.add_argument("--gobley-tools", type=Path, required=True)
    parser.add_argument("--archive-cache", type=Path, help="optional cache; every archive is rehashed")
    args = parser.parse_args(argv)
    receipt = prepare(args.output, args.uniffi_tools, args.gobley_tools, args.archive_cache)
    print(f"Verified {len(receipt['sources'])} registry packages: {args.output / 'vendor'}")


if __name__ == "__main__":
    main()
