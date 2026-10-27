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
import tomllib

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


def _copy_built_image(source, destination):
    """Freeze exactly the Cargo-reported regular image, not a conventional path."""
    source, destination = Path(source), Path(destination)
    if not source.is_absolute() or source.is_symlink() or not source.is_file():
        raise RuntimeError("Cargo executable is not an absolute regular image")
    expected = dict(sha256=sha_file(source), size=source.stat().st_size)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with source.open("rb") as incoming, destination.open("xb") as outgoing:
        shutil.copyfileobj(incoming, outgoing)
    actual = dict(sha256=sha_file(destination), size=destination.stat().st_size)
    if actual != expected or sha_file(source) != expected["sha256"] or source.stat().st_size != expected["size"]:
        raise RuntimeError("MadB tool image changed while freezing the build output")
    destination.chmod(0o500)
    return dict(path=str(destination), **actual)


def _build_madb_images(repo, work, expected_revision, provisional=False):
    """Internal same-build evidence; this is not a loadable public receipt format.

    Cargo's exact package/artifact messages bind version to the fresh output.
    A private target directory prevents another task's target/debug image from
    being substituted by a conventional-path lookup after an unrelated build.
    Formal source provenance still requires the clean-revision checks below.
    """
    repo, work = Path(repo).resolve(strict=True), Path(work).resolve(strict=True)
    inputs = [ref(repo, name) for name in ("Cargo.toml", "Cargo.lock", "crates/madb-cli/Cargo.toml", "rust-toolchain.toml")]
    declared = tomllib.loads((repo / "Cargo.toml").read_text())["workspace"]["package"]
    target = work / "tool-build"
    target.mkdir()
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target))
    # These overrides could replace the compiler or redirect output away from
    # the source/target pair described by this build record.
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "RUSTUP_TOOLCHAIN"):
        environment.pop(key, None)
    metadata_command = ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"]
    metadata_text = subprocess.check_output(metadata_command, cwd=repo, env=environment, text=True)
    metadata = json.loads(metadata_text)
    packages = [package for package in metadata["packages"]
                if package["id"] in metadata["workspace_members"] and package["name"] == "madb-cli"
                and Path(package["manifest_path"]).resolve() == repo / "crates/madb-cli/Cargo.toml"]
    if len(packages) != 1 or packages[0]["version"] != declared["version"] or packages[0]["publish"] != [] or declared["publish"] is not False:
        raise RuntimeError("MadB build package/version/publish state differs from the workspace")
    package = packages[0]
    if Path(metadata["target_directory"]).resolve() != target:
        raise RuntimeError("Cargo metadata does not name this private tool build directory")
    command = ["cargo", "build", "--locked", "-p", "madb-cli", "--bins", "--target-dir", str(target), "--message-format=json"]
    build = subprocess.run(command, cwd=repo, env=environment, stdout=subprocess.PIPE, text=True, check=True)
    messages = [json.loads(line) for line in build.stdout.splitlines() if line.strip()]
    finished = [message for message in messages if message.get("reason") == "build-finished"]
    if len(finished) != 1 or finished[0].get("success") is not True:
        raise RuntimeError("Cargo did not report a successful complete tool build")
    executables = {}
    for message in messages:
        if message.get("reason") != "compiler-artifact" or message.get("package_id") != package["id"]:
            continue
        name = message["target"]["name"]
        if name not in ("madb", "madb-native-finalize"):
            continue
        raw = message.get("executable")
        if name in executables or message["target"]["kind"] != ["bin"] or not isinstance(raw, str):
            raise RuntimeError("Cargo reported duplicate or invalid MadB executable artifacts")
        image = Path(raw)
        if not image.is_absolute() or image.is_symlink() or not image.resolve(strict=True).is_relative_to(target):
            raise RuntimeError("Cargo executable is outside this private tool build directory")
        executables[name] = image
    if set(executables) != {"madb", "madb-native-finalize"}:
        raise RuntimeError("Cargo omitted a required MadB tool artifact")
    if inputs != [ref(repo, item["path"]) for item in inputs]:
        raise RuntimeError("MadB build manifests or lock changed during compilation")
    if not provisional:
        require_revision(repo, expected_revision)
    images = {name: _copy_built_image(image, work / "images" / name) for name, image in sorted(executables.items())}
    host = subprocess.run([images["madb"]["path"], "--version"], cwd=work,
                          env={"PATH": os.defpath, "HOME": str(work)}, capture_output=True, text=True, check=True, timeout=30)
    if host.stdout != "madb " + package["version"] + "\n" or host.stderr:
        raise RuntimeError("Built MadB reports a different version from its Cargo artifact package")
    for image in images.values():
        if sha_file(image["path"]) != image["sha256"] or Path(image["path"]).stat().st_size != image["size"]:
            raise RuntimeError("Frozen MadB tool image changed during its version observation")
    # Preserve exact Cargo output as private evidence. No candidate schema or
    # helper stdin protocol is extended by this bookkeeping.
    records = work / "tool-build-evidence"
    records.mkdir()
    (records / "cargo-metadata.json").write_text(metadata_text)
    (records / "cargo-build.jsonl").write_text(build.stdout)
    receipt = records / "receipt.json"
    receipt.write_text(json.dumps(dict(source_revision=expected_revision, provisional=provisional,
        package_id=package["id"], version=package["version"], inputs=inputs, images=images,
        commands=[metadata_command, command],
        cargo_metadata=ref(records, "cargo-metadata.json"), cargo_build=ref(records, "cargo-build.jsonl")),
        sort_keys=True, indent=2) + "\n")
    return dict(version=package["version"], images=images,
                receipt=dict(path=str(receipt), sha256=sha_file(receipt), size=receipt.stat().st_size))


def _verify_built_tools(built_tools):
    for image in [*built_tools["images"].values(), built_tools["receipt"]]:
        path = Path(image["path"])
        if path.is_symlink() or not path.is_file() or path.stat().st_size != image["size"] or sha_file(path) != image["sha256"]:
            raise RuntimeError("MadB tool or receipt differs from the same-build frozen identity")


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


def observe(args, work, built_tools, repo=ROOT):
    _verify_built_tools(built_tools)
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
    finalizer = built_tools["images"]["madb-native-finalize"]
    if sha_file(finalizer["path"]) != finalizer["sha256"] or Path(finalizer["path"]).stat().st_size != finalizer["size"]:
        raise RuntimeError("Finalizer differs from the same-build frozen image")
    tools.register_image(observation, "finalizer", finalizer["path"], built_tools["version"], "source://MadB/contracts")
    observation["dependency_locks"]["madb-tool-build"] = built_tools["receipt"]
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
    work = Path(tempfile.mkdtemp(prefix="madb-c06-")).resolve()
    built_tools = _build_madb_images(ROOT, work, initial, provisional=args.allow_dirty)
    print("Verifying managed tool and dependency closures", flush=True)
    observation = observe(args, work, built_tools)
    host = Path(built_tools["images"]["madb"]["path"])
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
        _verify_built_tools(built_tools)
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
    _verify_built_tools(built_tools)
    if not args.allow_dirty:
        require_revision(ROOT, initial)
    print("Public C06 candidates: " + str(output), flush=True)


if __name__ == "__main__":
    main()
