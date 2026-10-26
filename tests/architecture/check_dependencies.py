"""Workspace dependency boundaries, using Cargo's resolved identities and all features."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys


# Change this alongside architecture/02 when a new physical member is approved.
ALLOWED = {
    "madb-model": set(),
    "madb-rules": {"madb-model"},
    "madb-plan": {"madb-model"},
    "madb-protocol": set(),
    "madb-app": {"madb-model", "madb-rules", "madb-plan"},
    "madb-cli": {"madb-app", "madb-contracts", "madb-host", "madb-templates"},
    "madb-contracts": {"madb-app", "madb-model", "madb-plan", "madb-rules", "madb-protocol"},
    "madb-host": {"madb-app"},
    "madb-templates": {"madb-model", "madb-plan", "madb-protocol"},
}


def inspect(root: Path) -> list[str]:
    # Resolve manifest declarations first. Forbidden edges may create a Cargo
    # cycle; detect them without asking the resolver to accept that graph.
    declarations = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--all-features", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, check=True,
    )
    declared = json.loads(declarations.stdout)
    members = {p["id"]: p for p in declared["packages"] if p["id"] in declared["workspace_members"]}
    paths = {Path(p["manifest_path"]).resolve().parent: p["name"] for p in members.values()}
    errors = []
    for package in members.values():
        allowed = ALLOWED.get(package["name"], set())
        for dependency in package["dependencies"]:
            target = paths.get(Path(dependency["path"]).resolve()) if dependency.get("path") else None
            if target and target not in allowed:
                errors.append(f"ARCH003: {package['name']} -> {target} is forbidden (manifest declaration)")
    if errors:
        return sorted(set(errors))

    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, check=True,
    )
    metadata = json.loads(result.stdout)
    packages = {package["id"]: package for package in metadata["packages"]}
    members = {identity: packages[identity] for identity in metadata["workspace_members"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    member_names = {package["name"] for package in members.values()}
    member_directories = {Path(p["manifest_path"]).resolve().parent: p["name"] for p in members.values()}
    errors = []
    if member_names != ALLOWED.keys():
        errors.append(f"ARCH001: workspace members differ from the documented boundaries: {sorted(member_names)}")

    for package in packages.values():
        if package["source"] is None and not Path(package["manifest_path"]).resolve().is_relative_to(root):
            errors.append(f"ARCH004: resolved path package is outside the repository: {package['name']}")

    for identity, package in members.items():
        name = package["name"]
        if name not in ALLOWED:
            continue
        manifest = Path(package["manifest_path"]).resolve()
        if not manifest.is_relative_to(root):
            errors.append(f"ARCH002: {name} is outside the repository")
        # Cargo retains target conditions and dependency kinds when no platform
        # filter is supplied. Use actual package identities, not import aliases.
        for dependency in nodes[identity]["deps"]:
            target = dependency["pkg"]
            if target not in members:
                continue
            target_name = packages[target]["name"]
            if target_name not in ALLOWED[name]:
                kinds = ", ".join(
                    f"{kind['kind'] or 'normal'} / {kind['target'] or 'all targets'}"
                    for kind in dependency["dep_kinds"]
                )
                errors.append(f"ARCH003: {name} -> {target_name} is forbidden ({kinds})")

        # An out-of-tree path dependency can hide a private adapter behind a
        # different package name. Registry sources remain governed by Cargo.lock.
        for dependency in package["dependencies"]:
            # Cargo may omit a dependency edge to a binary-only package; the
            # declaration itself must not bypass the architecture boundary.
            declared = member_directories.get(Path(dependency["path"]).resolve()) if dependency.get("path") else None
            if declared and declared not in ALLOWED[name]:
                errors.append(f"ARCH003: {name} -> {declared} is forbidden (manifest declaration)")
            if dependency.get("path") and not Path(dependency["path"]).resolve().is_relative_to(root):
                errors.append(f"ARCH004: {name} has an external path dependency: {dependency['name']}")

        reachable_allowed = set(ALLOWED[name])
        pending = list(reachable_allowed)
        while pending:
            current = pending.pop()
            for child in ALLOWED[current] - reachable_allowed:
                reachable_allowed.add(child)
                pending.append(child)
        visited = {identity}
        pending_ids = [identity]
        while pending_ids:
            current = pending_ids.pop()
            for dependency in nodes[current]["deps"]:
                target = dependency["pkg"]
                if target in visited:
                    continue
                visited.add(target)
                pending_ids.append(target)
                if target in members and packages[target]["name"] not in reachable_allowed:
                    errors.append(f"ARCH005: {name} reaches forbidden member {packages[target]['name']}")
    return sorted(set(errors))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    try:
        errors = inspect(args.root.resolve())
    except subprocess.CalledProcessError as error:
        print(f"ARCH000: Cargo metadata failed; dependency check not completed\n{error.stderr}", file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print("Architecture dependencies passed (all features; normal/build/dev and target-conditioned edges).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
