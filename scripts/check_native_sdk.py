"""C05 standalone Rust SDK checks; these unit builds do not certify a candidate."""
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
}
# Deliberately synthetic identities used only for local protocol unit tests.
# They must never be used as a real generated SDK/candidate identity or evidence.
UNIT_INTERFACE_DIGEST = "11" * 32
UNIT_PROFILE_DIGEST = "22" * 32


def inspect_boundaries(root: Path) -> list[str]:
    """Check Cargo's dependency declarations without compiling SDK templates.

    Both approved C05 SDK packages currently have zero Rust package dependencies.
    Rejecting every dependency also prevents aliases, optional/target/build/dev
    edges and transitive private-host imports. The incomplete c-client template
    is deliberately not built here; generated candidate consumption belongs to Lab.
    """
    root = root.resolve()
    errors = []
    manifests = {root / directory / "Cargo.toml" for directory in SDK_PACKAGES}
    sdk_root = root / "sdk" / "rust"
    actual = {path for path in sdk_root.rglob("Cargo.toml")
              if "target" not in path.relative_to(sdk_root).parts}
    if actual != manifests:
        errors.append("NATIVE001: Rust SDK package manifests differ from the two documented standalone packages")
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
            kind = dependency.get("kind") or "normal"
            target = dependency.get("target") or "all targets"
            alias = dependency.get("rename")
            label = dependency["name"] + (f" as {alias}" if alias else "")
            errors.append(
                f"NATIVE003: {name} -> {label} is forbidden ({kind} / {target}); "
                "C05 SDK packages have no Rust package dependencies or tool-host imports"
            )
    return sorted(set(errors))


def check(root: Path, boundaries_only: bool = False) -> None:
    errors = inspect_boundaries(root)
    if errors:
        raise RuntimeError("\n".join(errors))
    print("Native SDK boundaries passed (two standalone workspaces; no Rust package dependencies).", flush=True)
    if boundaries_only:
        return
    environment = os.environ.copy()
    environment["MADB_INTERFACE_DIGEST"] = UNIT_INTERFACE_DIGEST
    environment["MADB_PROFILE_DIGEST"] = UNIT_PROFILE_DIGEST
    print("C05 SDK unit builds use explicit synthetic identities; they do not certify a generated candidate.", flush=True)
    manifest = root / "sdk/rust/engine/Cargo.toml"
    commands = [["cargo", "fmt", "--manifest-path", str(manifest), "--", "--check"]]
    for features in ([], ["--features", "test-support"]):
        commands.extend([
            ["cargo", "clippy", "--manifest-path", str(manifest), "--all-targets", *features,
             "--locked", "--offline", "--", "-D", "warnings"],
            ["cargo", "test", "--manifest-path", str(manifest), "--release", *features,
             "--locked", "--offline"],
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
