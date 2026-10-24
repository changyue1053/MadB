"""Pinned Gobley adapter; the host owns inputs, processes and candidate publication."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import tomllib
import tempfile
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request

HERE = Path(__file__).resolve().parent


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_lock() -> dict:
    lock = json.loads((HERE / "toolchain.json").read_text())
    for name in ("generator", "runtime"):
        item = lock[name]
        for patch in [item, *item.get("additional_patches", [])]:
            if digest(HERE / patch["patch"]) != patch["patch_sha256"]:
                raise ValueError(f"{name} patch differs from the tool lock")
    if digest(HERE / "generator.Cargo.lock") != lock["generator"]["cargo_lock_sha256"]:
        raise ValueError("generator Cargo.lock differs from the tool lock")
    return lock


def run(arguments: list[str], *, cwd: Path, env: dict | None = None) -> None:
    subprocess.run(arguments, cwd=cwd, env=env, check=True)


def fetch(item: dict, archive: Path) -> None:
    if not archive.exists():
        archive.parent.mkdir(parents=True, exist_ok=True)
        request = urllib.request.Request(item["source"], headers={"User-Agent": "MadB-C06/1"})
        with urllib.request.urlopen(request, timeout=90) as response:
            archive.write_bytes(response.read())
    if digest(archive) != item["sha256"]:
        raise ValueError(f"source archive checksum mismatch: {archive.name}")


def install(destination: Path) -> dict:
    """Install both baseline and patched tools. Network/source build is explicit."""
    lock = read_lock()
    destination = destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    receipt = destination / "receipt.json"
    if receipt.exists():
        return verify_install(destination)
    for key, archive_name in (("generator", "gobley-0.3.7.tar.gz"), ("runtime", "uniffi_core-0.29.4.crate")):
        archive = destination / archive_name
        fetch(lock[key], archive)
        with tarfile.open(archive) as source:
            source.extractall(destination, filter="data")
    generator = destination / "gobley-0.3.7"
    # Upstream workspace includes unrelated BoringSSL examples. The exact pinned
    # CLI-only lock retains all generator dependency versions without those builds.
    manifest = generator / "Cargo.toml"
    source = manifest.read_text()
    start = source.index("members = [")
    end = source.index("]", start)
    manifest.write_text(source[:start] + 'members = ["crates/gobley-uniffi-bindgen"]' + source[end + 1:])
    shutil.copyfile(HERE / "generator.Cargo.lock", generator / "Cargo.lock")
    command = ["cargo", f"+{lock['rust']}", "build", "--locked", "--release", "-p", "gobley-uniffi-bindgen", "--bin", "gobley-uniffi-bindgen"]
    run(command, cwd=generator)
    binary = generator / "target/release/gobley-uniffi-bindgen"
    shutil.copy2(binary, destination / "gobley-original")
    for patch in [lock["generator"], *lock["generator"].get("additional_patches", [])]:
        run(["patch", "-p1", "--forward", "-i", str(HERE / patch["patch"])], cwd=generator)
    run(command, cwd=generator)
    shutil.copy2(binary, destination / "gobley-patched")
    original_runtime = destination / "uniffi_core-0.29.4"
    patched_runtime = destination / "uniffi_core-0.29.4-madb"
    shutil.copytree(original_runtime, patched_runtime)
    for patch in [lock["runtime"], *lock["runtime"].get("additional_patches", [])]:
        run(["patch", "-p1", "--forward", "-i", str(HERE / patch["patch"])], cwd=patched_runtime)
    files = ["gobley-original", "gobley-patched", "gobley-0.3.7.tar.gz", "uniffi_core-0.29.4.crate"]
    for tree in (original_runtime, patched_runtime):
        files.extend(str(path.relative_to(destination)) for path in sorted(tree.rglob("*")) if path.is_file())
    result = {"lock_sha256": digest(HERE / "toolchain.json"), "files": {p: digest(destination / p) for p in files}}
    receipt.write_text(json.dumps(result, indent=2) + "\n")
    return verify_install(destination)


def verify_install(destination: Path) -> dict:
    read_lock()
    receipt = json.loads((destination / "receipt.json").read_text())
    if receipt["lock_sha256"] != digest(HERE / "toolchain.json"):
        raise ValueError("installed Gobley tool lock is stale")
    expected = {"gobley-original", "gobley-patched", "gobley-0.3.7.tar.gz", "uniffi_core-0.29.4.crate"}
    for name in ("uniffi_core-0.29.4", "uniffi_core-0.29.4-madb"):
        tree = destination / name
        if not (tree / "src/ffi/foreignfuture.rs").is_file():
            raise ValueError("installed runtime source tree is missing")
        if any(path.is_symlink() for path in tree.rglob("*")):
            raise ValueError("installed runtime source contains a symlink")
        expected.update(str(path.relative_to(destination)) for path in tree.rglob("*") if path.is_file())
    if set(receipt["files"]) != expected:
        raise ValueError("installed Gobley receipt is incomplete")
    for name, checksum in receipt["files"].items():
        if digest(destination / name) != checksum:
            raise ValueError(f"installed Gobley file changed: {name}")
    return receipt



def derive_lock(path: Path) -> dict:
    """Select only the reviewed runtime patch in an isolated source-copy lock."""
    original = path.read_bytes()
    checksum = read_lock()["runtime"]["sha256"]
    chunks = re.split(r"(?m)(?=^\[\[package\]\])", original.decode())
    found = 0
    for index, chunk in enumerate(chunks):
        packages = tomllib.loads(chunk).get("package", [])
        if not packages or packages[0]["name"] != "uniffi_core":
            continue
        package = packages[0]
        if package["version"] != "0.29.4" or package.get("source") != "registry+https://github.com/rust-lang/crates.io-index" or package.get("checksum") != checksum:
            raise ValueError("canonical Gobley runtime lock does not match the reviewed archive")
        chunks[index] = "\n".join(line for line in chunk.split("\n") if not line.startswith(("source =", "checksum =")))
        found += 1
    if found != 1:
        raise ValueError("canonical Gobley lock must contain exactly one runtime")
    path.write_text("".join(chunks))
    return {"original_sha256": hashlib.sha256(original).hexdigest(), "derived_sha256": digest(path)}


def build_sdk(repository: Path, tools: Path, output: Path, profile_digest: str, *, test_support: bool = False) -> dict:
    verify_install(tools)
    lock = read_lock()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("KN-RUST-GOBLEY requires the locked macOS arm64 host")
    output.mkdir(parents=True, exist_ok=True)
    runtime = tools / "uniffi_core-0.29.4-madb"
    with tempfile.TemporaryDirectory(prefix="gobley-source-", dir=output.parent) as temporary:
        source = Path(temporary)
        for crate in ("boundary-core", "uniffi", "gobley"):
            shutil.copytree(repository / "sdk/rust" / crate, source / "sdk/rust" / crate,
                            ignore=shutil.ignore_patterns("target", "__pycache__"))
        lock_derivation = derive_lock(source / "sdk/rust/gobley/Cargo.lock")
        command = ["cargo", f"+{lock['rust']}", "build", "--locked", "--release", "--manifest-path",
                   str(source / "sdk/rust/gobley/Cargo.toml"), "--target-dir", str(output),
                   "--config", f'patch.crates-io.uniffi_core.path="{runtime}"']
        if test_support:
            command.extend(["--features", "test-support"])
        environment = dict(os.environ, MADB_PROFILE_DIGEST=profile_digest, MACOSX_DEPLOYMENT_TARGET="14.0",
                           RUSTFLAGS="", CARGO_ENCODED_RUSTFLAGS="\x1f".join([
                               "--remap-path-prefix="+str(source)+"=/madb/source",
                               "--remap-path-prefix="+str(output)+"=/madb/build",
                               "--remap-path-prefix="+str(runtime)+"=/madb/runtime"]))
        run(command, cwd=source, env=environment)
    return {"library": str(output / "release/libmadb_boundary_gobley.dylib"),
            "static_library": str(output / "release/libmadb_boundary_gobley.a"),
            "profile_digest": profile_digest, "toolchain": lock, "lock_derivation": lock_derivation}


def generate(repository: Path, tools: Path, library: Path, output: Path, *, baseline: bool = False) -> None:
    verify_install(tools)
    binary = tools / ("gobley-original" if baseline else "gobley-patched")
    run([str(binary), "--library", str(library), "--out-dir", str(output), "--config", str(repository / "sdk/rust/gobley/uniffi.toml"), "--crate-paths", f"madb_boundary_gobley={repository / 'sdk/rust/uniffi'}"], cwd=repository)



def generate_udl(repository: Path, tools: Path, output: Path) -> None:
    """Production contract authority; identical output to the matching built library."""
    verify_install(tools)
    run([str(tools / "gobley-patched"), str(repository / "sdk/rust/uniffi/src/boundary.udl"),
         "--crate", "madb_boundary_gobley", "--config", str(repository / "sdk/rust/gobley/uniffi.toml"),
         "--out-dir", str(output)], cwd=repository)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["install", "generate", "generate-udl", "build"])
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=HERE.parents[2])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--profile-digest")
    parser.add_argument("--baseline", action="store_true")
    parser.add_argument("--test-support", action="store_true")
    args = parser.parse_args()
    tools = args.tools.resolve()
    if args.command == "install":
        print(json.dumps(install(tools)))
    elif args.command == "generate-udl":
        if not args.output:
            parser.error("generate-udl requires --output")
        generate_udl(args.repository.resolve(), tools, args.output.resolve())
    elif args.command == "generate":
        if not args.output or not args.library:
            parser.error("generate requires --library and --output")
        generate(args.repository.resolve(), tools, args.library.resolve(), args.output.resolve(), baseline=args.baseline)
    else:
        if not args.output or not args.profile_digest:
            parser.error("build requires --output and --profile-digest")
        print(json.dumps(build_sdk(args.repository.resolve(), tools, args.output.resolve(), args.profile_digest, test_support=args.test_support)))


if __name__ == "__main__":
    main()
