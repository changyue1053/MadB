"""Build C06 Release candidates through the same frozen MadB ports as C05."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.generators.uniffi import driver as uniffi
from adapters.generators.gobley import driver as gobley
from adapters.toolchains.common.documents import ref, sha_file
from adapters.toolchains.kotlin_rust.pipeline import Pipeline
from adapters.toolchains.kotlin_rust.project import prepare, VARIANTS
from adapters.toolchains.kotlin_rust import tools
from scripts.prepare_managed_dependencies import verify as verify_vendor

COMMON_CASES = ["data", "errors", "objects", "async", "cancel", "late-callback", "capacity", "shutdown", "identity", "wrong-profile", "wrong-abi", "distribution"]
EXTRA_CASES = {
    "kotlin-native-rust": ["foreign-future-once", "callback-throw", "cancel-late-result"],
    "swift-kotlin": ["enhanced-api", "missing-cinterop-origin", "generic-lambda"],
}


def revision(repo):
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()


def dirty(repo):
    return bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=repo, text=True).strip())


def require_revision(repo, expected):
    if revision(repo) != expected or dirty(repo):
        raise RuntimeError("producer checkout changed during formal candidate preparation")


def verify_revision_inputs(repo, workspace, prepared, expected_revision):
    """Every author byte is Git-bound; derived compiler inputs have an exact separate set."""
    authors = prepared["author_inputs"] + prepared["lock"]["schemas"]
    author_names = {item["path"] for item in authors}
    derived = {item["path"] for item in prepared["derived_inputs"]}
    allowed = author_names | derived | {"backend.json", "toolchain.json"}
    if any(item["path"] not in allowed for item in prepared["read"]["inputs"]):
        raise RuntimeError("unverified input entered the formal producer closure")
    for item in authors:
        source = subprocess.check_output(["git", "show", expected_revision + ":" + item["path"]], cwd=repo)
        expected = "sha256:" + hashlib.sha256(source).hexdigest()
        actual = Path(workspace) / item["path"]
        if expected != item["sha256"] or sha_file(actual) != expected or actual.stat().st_size != item["size"]:
            raise RuntimeError("frozen author input differs from producer revision: " + item["path"])
    for item in prepared["derived_inputs"]:
        if ref(workspace, item["path"]) != {key: item[key] for key in ("path", "sha256", "size")}:
            raise RuntimeError("compiler-produced declaration changed: " + item["path"])


def _uniffi_installation(path, repo=ROOT):
    path = Path(path).resolve(strict=True)
    result = dict(root=str(path), receipt_sha256=sha_file(path / "receipt.json").removeprefix("sha256:"),
                  bindgen=str(path / "bin/uniffi-bindgen"), metadata=str(path / "bin/madb-uniffi-metadata"))
    receipt = uniffi.verify(result)
    if receipt.get("metadata_inputrefs") != uniffi.metadata_inputrefs(repo) or receipt.get("metadata_features") != []:
        raise ValueError("installed UniFFI declaration helper was built from different author inputs")
    return result


def _cargo_environment(observation, home, profile):
    home.mkdir(parents=True)
    vendor = observation["trees"]["cargo-vendor"]["root"]
    # Cargo reads a private offline source map, never the developer's Cargo config.
    (home / "config.toml").write_text('[source.crates-io]\nreplace-with = "madb-vendor"\n[source.madb-vendor]\ndirectory = ' + json.dumps(vendor) + '\n[net]\noffline = true\n')
    environment = dict(observation["environment"], CARGO_HOME=str(home), CARGO_NET_OFFLINE="true", MADB_PROFILE_DIGEST=profile,
                       HOME=str(home.parent), TMPDIR=str(home.parent), RUSTC=observation["tools"]["rustc"]["path"],
                       SDKROOT=observation["sdk"], MACOSX_DEPLOYMENT_TARGET="14.0")
    environment["PATH"] = str(Path(observation["tools"]["rustc"]["path"]).parent) + os.pathsep + "/usr/bin:/bin:/usr/sbin:/sbin"
    return environment


def _register_gobley_metadata(observation, work, repo):
    """The backend-version-specific parser is built from this fixed source revision."""
    root = Path(observation["gobley"]["root"])
    output = work / "gobley-metadata-build"
    source = work / "gobley-metadata-source"
    for crate in ("boundary-core", "uniffi", "gobley"):
        shutil.copytree(repo / "sdk/rust" / crate, source / "sdk/rust" / crate,
                        ignore=shutil.ignore_patterns("target", "__pycache__", ".DS_Store"))
    helper_inputs = [ref(source, path.relative_to(source).as_posix()) for path in sorted((source / "sdk/rust").rglob("*")) if path.is_file()]
    derivation = gobley.derive_lock(source / "sdk/rust/gobley/Cargo.lock")
    cargo = observation["tools"]["cargo"]["path"]
    environment = _cargo_environment(observation, work / "gobley-metadata-home", "sha256:" + "0" * 64)
    command = [cargo, "build", "--locked", "--offline", "--release", "--bin", "madb-gobley-metadata", "--manifest-path", str(source / "sdk/rust/gobley/Cargo.toml"), "--target-dir", str(output),
               "--config", 'patch.crates-io.uniffi_core.path=' + json.dumps(str(root / "uniffi_core-0.29.4-madb"))]
    tools.verify(observation)
    subprocess.run(command, cwd=repo, env=environment, check=True, timeout=600)
    observation.setdefault("declaration_helpers", {})["metadata_v029"] = dict(inputs=helper_inputs, features=["gobley-029"], lock_derivation=derivation)
    image = work / "images/madb-gobley-metadata"
    image.parent.mkdir(exist_ok=True)
    shutil.copyfile(output / "release/madb-gobley-metadata", image)
    image.chmod(0o500)
    tools.register_image(observation, "metadata_v029", image, "uniffi-0.29.4", "source://sdk/rust/gobley")
    tools.verify(observation)


def observe(args, work, repo=ROOT):
    vendor_receipt = verify_vendor(args.cargo_vendor, repo) if args.cargo_vendor is not None else None
    observation = tools.discover(repo, args.managed_root,
                                 gobley_root=args.gobley_tools, runtime_root=args.runtime_dependencies,
                                 maven_root=args.maven_repository, cargo_root=args.cargo_vendor)
    if vendor_receipt is not None:
        observation["dependency_locks"]["cargo-vendor-receipt"] = vendor_receipt
    if args.uniffi_tools is not None:
        installation = _uniffi_installation(args.uniffi_tools, repo)
        observation["uniffi"] = installation
        observation.setdefault("declaration_helpers", {})["metadata_v032"] = dict(inputs=uniffi.metadata_inputrefs(repo), features=[])
        tools.register_image(observation, "uniffi_bindgen", installation["bindgen"], "0.32.2-madb", "managed://uniffi-0.32.2/source-and-patches")
        tools.register_image(observation, "metadata_v032", installation["metadata"], "uniffi-0.32.2", "managed://uniffi-0.32.2/metadata-parser")
        tools.register_tree(observation, "uniffi-managed-sources", Path(installation["root"]) / "sources")
        observation["dependency_locks"]["uniffi-installation-receipt"] = dict(path=str(Path(installation["root"]) / "receipt.json"), sha256="sha256:" + installation["receipt_sha256"], size=(Path(installation["root"]) / "receipt.json").stat().st_size)
    if "kotlin-native-rust" in args.variants:
        _register_gobley_metadata(observation, work, repo)
    finalizer = work / "images/madb-native-finalize"
    finalizer.parent.mkdir(exist_ok=True)
    shutil.copyfile(repo / "target/debug/madb-native-finalize", finalizer)
    finalizer.chmod(0o500)
    tools.register_image(observation, "finalizer", finalizer, "0.1.0-dev.0", "source://MadB/contracts")
    tools.verify(observation)
    return observation


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new public candidate directory")
    parser.add_argument("--managed-root", type=Path, required=True, help="verified managed tool installation")
    parser.add_argument("--runtime-dependencies", type=Path, required=True, help="exact installed runtime Maven closure")
    parser.add_argument("--maven-repository", type=Path, help="exact SKIE offline Maven closure")
    parser.add_argument("--cargo-vendor", type=Path, help="closed vendor tree covering the selected locked Rust SDKs")
    parser.add_argument("--uniffi-tools", type=Path, help="source-verified patched UniFFI installation")
    parser.add_argument("--gobley-tools", type=Path, help="source-verified patched Gobley installation")
    parser.add_argument("--variants", nargs="+", choices=sorted(VARIANTS), default=list(VARIANTS), help="explicit profiles; omission builds all four")
    parser.add_argument("--allow-dirty", action="store_true", help="development only: formal Lab acceptance rejects the provisional candidates")
    args = parser.parse_args(argv)
    if len(set(args.variants)) != len(args.variants):
        parser.error("--variants cannot contain duplicates")
    if args.output.exists():
        parser.error("--output must not exist")
    if any(v != "swift-kotlin" for v in args.variants) and args.cargo_vendor is None:
        parser.error("selected Rust profiles require --cargo-vendor")
    if any(v in ("kotlin-jvm-rust", "swift-rust") for v in args.variants) and args.uniffi_tools is None:
        parser.error("selected UniFFI 0.32 profiles require --uniffi-tools")
    if "kotlin-native-rust" in args.variants and args.gobley_tools is None:
        parser.error("Kotlin/Native Rust requires --gobley-tools")
    if "swift-kotlin" in args.variants and args.maven_repository is None:
        parser.error("Swift/Kotlin requires --maven-repository")
    return args


def main(argv=None):
    args = arguments(argv)
    if dirty(ROOT) and not args.allow_dirty:
        raise RuntimeError("formal candidates require a clean producer checkout; commit first or explicitly select --allow-dirty for development")
    initial = revision(ROOT)
    subprocess.run(["cargo", "build", "--locked", "-p", "madb-cli", "--bins"], cwd=ROOT, check=True)
    work = Path(tempfile.mkdtemp(prefix="madb-c06-")).resolve()
    print("Verifying managed tool and dependency closures", flush=True)
    observation = observe(args, work)
    host = work / "madb-host"
    shutil.copyfile(ROOT / "target/debug/madb", host)
    host.chmod(0o500)
    output = args.output.absolute()
    output.mkdir(parents=True)
    print("C06 frozen execution workspace: " + str(work), flush=True)
    if args.allow_dirty:
        print("PROVISIONAL development candidates; these are not formal acceptance evidence", flush=True)
    for variant in args.variants:
        if not args.allow_dirty:
            require_revision(ROOT, initial)
        print("Preparing " + variant, flush=True)
        workspace = work / variant
        prepared = prepare(ROOT, workspace, observation, variant)
        if not args.allow_dirty:
            verify_revision_inputs(ROOT, workspace, prepared, initial)
            require_revision(ROOT, initial)
        pipeline = Pipeline(ROOT, workspace, prepared, host)
        print(variant + ": reading checked contract", flush=True)
        pipeline.metadata()
        print(variant + ": generating bindings", flush=True)
        pipeline.generate()
        print(variant + ": building and probing SDK", flush=True)
        pipeline.build()
        required = ["c06." + variant + "." + name for name in COMMON_CASES + EXTRA_CASES.get(variant, [])]
        print(variant + ": packaging public candidate", flush=True)
        manifest = pipeline.package(output / variant, initial, required, provisional=args.allow_dirty)
        print(variant + ": " + manifest["candidate_digest"], flush=True)
    tools.verify(observation)
    if not args.allow_dirty:
        require_revision(ROOT, initial)
    print("Public C06 candidates: " + str(output), flush=True)


if __name__ == "__main__":
    main()
