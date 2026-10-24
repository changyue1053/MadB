"""Standalone SDK dependency boundaries and native unit gates; no candidate certification."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
SDK_PACKAGES = {
    "sdk/rust/engine": "madb-first-route-engine",
    "sdk/rust/c-client": "madb-c-client",
    "sdk/rust/boundary-core": "madb-boundary-core",
    "sdk/rust/uniffi": "madb-boundary-uniffi",
    "sdk/rust/gobley": "madb-boundary-gobley",
}
# These SDK build/parser packages use reviewed external dependencies, never host crates.
SDK_REGISTRY = {
    "madb-boundary-uniffi": {("uniffi", "normal"): ("=0.32.2", {"cli"}), ("uniffi", "build"): ("=0.32.2", {"build"}),
                             ("async-trait", "normal"): ("=0.1.89", set()), ("uniffi_bindgen", "normal"): ("=0.32.2", set()), ("serde_json", "normal"): ("=1.0.154", set())},
    "madb-boundary-gobley": {("uniffi", "normal"): ("=0.29.4", set()), ("uniffi", "build"): ("=0.29.4", {"build"}),
                             ("async-trait", "normal"): ("=0.1.89", set()), ("uniffi_bindgen", "normal"): ("=0.29.4", set()), ("serde_json", "normal"): ("=1.0.154", set())},
}


def approved_dependency(root, name, dependency):
    kind = dependency.get("kind") or "normal"
    if dependency.get("rename") or dependency.get("target") or dependency.get("optional") or dependency.get("uses_default_features") is not True:
        return False
    if name in SDK_REGISTRY and dependency["name"] == "madb-boundary-core":
        return (kind == "normal" and dependency.get("source") is None and not dependency.get("features")
                and Path(dependency.get("path", "")).resolve() == (root / "sdk/rust/boundary-core").resolve())
    expected = SDK_REGISTRY.get(name, {}).get((dependency["name"], kind))
    return (expected is not None and dependency.get("path") is None
            and dependency.get("source") == "registry+https://github.com/rust-lang/crates.io-index"
            and dependency["req"] == expected[0] and set(dependency["features"]) == expected[1])


# Deliberately synthetic identities used only for local protocol unit tests.
# They must never be used as a real generated SDK/candidate identity or evidence.
UNIT_INTERFACE_DIGEST = "11" * 32
UNIT_PROFILE_DIGEST = "22" * 32


def inspect_boundaries(root: Path) -> list[str]:
    """Check Cargo's dependency declarations without compiling SDK templates.

    C05 packages and the shared C06 runtime are dependency-free. C06 binding/parser
    packages may use only the exact reviewed registry dependencies and shared SDK
    runtime. Every local edge remains in this independently checked set; aliases,
    target/optional/dev edges cannot hide tool-host imports. Candidate builds
    additionally verify the complete source vendor closure.
    """
    root = root.resolve()
    errors = []
    manifests = {root / directory / "Cargo.toml" for directory in SDK_PACKAGES}
    sdk_root = root / "sdk" / "rust"
    actual = {path for path in sdk_root.rglob("Cargo.toml")
              if "target" not in path.relative_to(sdk_root).parts}
    if actual != manifests:
        errors.append("NATIVE001: Rust SDK package manifests differ from the documented standalone package set")
    for directory, name in SDK_PACKAGES.items():
        package_root = root / directory
        manifest = package_root / "Cargo.toml"
        try:
            document = tomllib.loads(manifest.read_text())
        except (OSError, tomllib.TOMLDecodeError) as error:
            errors.append(f"NATIVE001: cannot read SDK manifest {directory}: {error}")
            continue
        if document.get("package", {}).get("name") != name:
            errors.append(f"NATIVE002: {directory} must retain package identity {name}")
            continue
        if document.get("workspace") != {} or "workspace" in document["package"]:
            errors.append(f"NATIVE002: {name} must declare its own empty [workspace]")
            continue
        result = subprocess.run(
            ["cargo", "metadata", "--manifest-path", str(manifest), "--no-deps",
             "--all-features", "--format-version", "1", "--locked", "--offline"],
            cwd=root, capture_output=True, text=True,
        )
        if result.returncode:
            errors.append(f"NATIVE000: Cargo metadata failed for {name}; isolation not checked\n{result.stderr}")
            continue
        try:
            metadata = json.loads(result.stdout)
            packages = {package["id"]: package for package in metadata["packages"]}
            members = [packages[identity] for identity in metadata["workspace_members"]]
        except (json.JSONDecodeError, KeyError, TypeError) as error:
            errors.append(f"NATIVE000: invalid Cargo metadata for {name}: {error}")
            continue
        if (Path(metadata["workspace_root"]).resolve() != package_root.resolve()
                or len(members) != 1 or members[0]["name"] != name
                or Path(members[0]["manifest_path"]).resolve() != manifest.resolve()):
            errors.append(f"NATIVE002: {name} is not its own standalone Cargo workspace")
            continue
        for dependency in members[0]["dependencies"]:
            if approved_dependency(root, name, dependency):
                continue
            kind = dependency.get("kind") or "normal"
            target = dependency.get("target") or "all targets"
            alias = dependency.get("rename")
            label = dependency["name"] + (f" as {alias}" if alias else "")
            errors.append(
                f"NATIVE003: {name} -> {label} is forbidden ({kind} / {target}); "
                "dependency is outside the explicit SDK allowlist; tool-host imports are forbidden"
            )
    return sorted(set(errors))


def check(root: Path, boundaries_only: bool = False) -> None:
    errors = inspect_boundaries(root)
    if errors:
        raise RuntimeError("\n".join(errors))
    print("Native SDK boundaries passed (five standalone workspaces; explicit dependency allowlist, no tool-host imports).", flush=True)
    if boundaries_only:
        return
    environment = os.environ.copy()
    environment["MADB_INTERFACE_DIGEST"] = UNIT_INTERFACE_DIGEST
    environment["MADB_PROFILE_DIGEST"] = UNIT_PROFILE_DIGEST
    print("C05 SDK unit builds use explicit synthetic identities; they do not certify a generated candidate.", flush=True)
    manifest = root / "sdk/rust/engine/Cargo.toml"
    commands = [["cargo", "fmt", "--manifest-path", str(root / directory / "Cargo.toml"), "--", "--check"]
                for directory in SDK_PACKAGES]
    for features in ([], ["--features", "test-support"]):
        commands.extend([
            ["cargo", "clippy", "--manifest-path", str(manifest), "--all-targets", *features,
             "--locked", "--offline", "--", "-D", "warnings"],
            ["cargo", "test", "--manifest-path", str(manifest), "--release", *features,
             "--locked", "--offline"],
        ])
    core = root / "sdk/rust/boundary-core/Cargo.toml"
    commands.extend([
        ["cargo", "clippy", "--manifest-path", str(core), "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
        ["cargo", "test", "--manifest-path", str(core), "--release", "--locked", "--offline"],
    ])
    for command in commands:
        print("+ " + " ".join(command), flush=True)
        subprocess.run(command, cwd=root, env=environment, check=True)
    print("Native SDK unit gates passed (default Release and test-support); candidate/Lab verification is separate.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--boundaries-only", action="store_true")
    args = parser.parse_args()
    try:
        check(args.root.resolve(), args.boundaries_only)
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f"Native SDK checks failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
