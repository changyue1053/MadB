"""Reuse the verified Cargo archive closure; retain original license bytes."""

from __future__ import annotations
import importlib.util
import json
from pathlib import Path
import tomllib

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_rust_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)
host = support.load_module(Path(__file__).with_name("rust_host.py"))
snapshot = support.load_module(
    _ROOT / "adapters/toolchains/kotlin_rust/tools.py"
).snapshot

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
NAPI_PACKAGES = {
    "napi-ohos",
    "napi-sys-ohos",
    "napi-build-ohos",
    "napi-derive-ohos",
    "napi-derive-backend-ohos",
}
NAPI_COMMIT = "e4eb8e1b493d6c3aa98ca220776605963df8135a"
NAPI_LICENSE_SHA = (
    "sha256:0c0e9a8f1b2914d3e31cec973eaf5d04760b2d68272f3abec676a3ada331b0a6"
)


def registry_packages(lock):
    packages = tomllib.loads(Path(lock).read_text())["package"]
    result = {}
    for package in packages:
        if "source" not in package:
            continue
        if package["source"] != REGISTRY or len(package.get("checksum", "")) != 64:
            raise ValueError("Rust dependency requires an exact registry checksum")
        key = package["name"] + "-" + package["version"]
        if key in result:
            raise ValueError("duplicate Rust dependency identity")
        result[key] = dict(
            name=package["name"],
            version=package["version"],
            source=REGISTRY,
            sha256="sha256:" + package["checksum"],
        )
    for name in NAPI_PACKAGES:
        if name + "-1.2.0" not in result:
            raise ValueError("the five reviewed napi crates must stay at 1.2.0")
    return result


def _reference(path):
    path = Path(path)
    return dict(path=str(path), size=path.stat().st_size, sha256=support.sha(path))


def prepare(repo, config, cargo_home, output):
    """Called by dependency preparation, never from metadata or Build replay.

    Cargo uses the provided cache offline. Existing C06 archive verification
    proves each vendor file against the original crate and Cargo.lock checksum.
    """
    repo, output, cargo_home = map(Path, (repo, output, cargo_home))
    output.mkdir(parents=True, exist_ok=False)
    script = repo / "scripts/prepare_managed_dependencies.py"
    spec = importlib.util.spec_from_file_location(
        "_madb_cargo_archive_verifier", script
    )
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    crate = repo / "sdk/rust/arkts-bridge"
    locks = registry_packages(crate / "Cargo.lock")
    before = [_reference(crate / n) for n in ("Cargo.toml", "Cargo.lock")]
    environment = {
        "PATH": str(Path(config["tools"]["cargo"]["path"]).parent) + ":/usr/bin:/bin",
        "CARGO_HOME": str(cargo_home),
        "CARGO_NET_OFFLINE": "true",
        "RUSTC": str(support.verify_tool(config["tools"]["rustc"])),
        "LC_ALL": "C",
    }
    support.command(
        config,
        "cargo",
        [
            "vendor",
            "--offline",
            "--locked",
            "--versioned-dirs",
            "--manifest-path",
            crate / "Cargo.toml",
            output / "vendor",
        ],
        cwd=output,
        extra_environment=environment,
    )
    vendor = output / "vendor"
    if {p.name for p in vendor.iterdir()} != set(locks):
        raise ValueError("vendor contents differ from the locked dependency set")
    packages, licenses = [], []
    supplemental = Path(__file__).with_name("licenses") / "napi-ohos-MIT-LICENSE"
    if support.sha(supplemental) != NAPI_LICENSE_SHA:
        raise ValueError("fixed upstream napi license changed")
    for name, item in sorted(locks.items()):
        candidates = list((cargo_home / "registry/cache").glob("*/" + name + ".crate"))
        archive = next(
            (p for p in candidates if support.sha(p) == item["sha256"]), None
        )
        if archive is None:
            raise ValueError("original offline crate archive unavailable: " + name)
        package = vendor / name
        packages.append(verifier._verify_package(package, item, archive))
        notices = [
            p
            for p in sorted(package.rglob("*"))
            if p.is_file()
            and p.name.lower().startswith(("license", "licence", "notice", "copyright"))
        ]
        if item["name"] in NAPI_PACKAGES:
            vcs = support.read_json(package / ".cargo_vcs_info.json")
            if vcs["git"]["sha1"] != NAPI_COMMIT:
                raise ValueError(
                    "napi supplement does not belong to this upstream revision"
                )
            notices.append(supplemental)
        if not notices:
            raise ValueError("dependency license text is missing: " + name)
        for source in notices:
            relative = (
                source.relative_to(package).as_posix()
                if source.is_relative_to(package)
                else "UPSTREAM-MIT-LICENSE"
            )
            destination = output / "notices" / name / relative
            support.copy_new(source, destination)
            licenses.append(
                dict(
                    package=name,
                    source=_reference(source),
                    output=destination.relative_to(output).as_posix(),
                    sha256=support.sha(destination),
                    size=destination.stat().st_size,
                )
            )
    # Rust's standard library and compiler-builtins are statically included by
    # this route. Keep the installed library copyright inventory plus original
    # license texts; this is not a claim that every listed component is linked.
    toolchain = support.verify_tool(config["tools"]["rustc"]).parent.parent
    if (toolchain / "lib").resolve() != Path(
        config["rust"]["rust_sysroot"]["root"]
    ).resolve():
        raise ValueError(
            "Rust runtime notices must belong to the observed compiler sysroot"
        )
    documentation = toolchain / "share/doc/rust"
    runtime_notices = [
        documentation / "COPYRIGHT-library.html",
        *sorted((documentation / "licenses").glob("*.txt")),
    ]
    if len(runtime_notices) < 3 or not all(
        p.is_file() and not p.is_symlink() for p in runtime_notices
    ):
        raise ValueError(
            "installed Rust standard-library copyright/license originals are missing"
        )
    for source in runtime_notices:
        before_notice = _reference(source)
        relative = source.relative_to(documentation).as_posix()
        destination = output / "notices/rust-std-1.99.1" / relative
        support.copy_new(source, destination)
        if (
            before_notice != _reference(source)
            or support.sha(destination) != before_notice["sha256"]
        ):
            raise ValueError("Rust standard-library notice changed while copying")
        licenses.append(
            dict(
                package="rust-std-1.99.1",
                source=before_notice,
                output=destination.relative_to(output).as_posix(),
                sha256=support.sha(destination),
                size=destination.stat().st_size,
            )
        )
    if before != [_reference(crate / n) for n in ("Cargo.toml", "Cargo.lock")]:
        raise ValueError("Rust source lock changed during preparation")
    receipt = dict(
        schema="madb.cargo-vendor/1",
        scope="arkts-rust",
        rust="1.99.1",
        source_verifier=_reference(script),
        sdk_inputs=before,
        sources=packages,
        licenses=licenses,
        tree=snapshot(vendor),
    )
    (output / "receipt.json").write_text(
        json.dumps(receipt, sort_keys=True, indent=2) + "\n"
    )
    return dict(
        root=str(output),
        receipt=_reference(output / "receipt.json"),
        tree=receipt["tree"],
    )


def verify(inputs, config):
    """No downloads or Cargo cache mutation during frozen execution."""
    selected = config["rust"]["dependencies"]
    root = Path(selected["root"])
    receipt_ref = selected["receipt"]
    if _reference(root / "receipt.json") != receipt_ref:
        raise ValueError("Rust dependency receipt changed")
    receipt = support.read_json(root / "receipt.json", 16 * support.MAX_JSON)
    if (receipt.get("schema"), receipt.get("scope"), receipt.get("rust")) != (
        "madb.cargo-vendor/1",
        "arkts-rust",
        "1.99.1",
    ):
        raise ValueError("unsupported Rust dependency closure")
    actual = snapshot(root / "vendor")
    if actual != receipt["tree"] or actual != selected["tree"]:
        raise ValueError("Rust vendor tree changed")
    expected = registry_packages(
        support.input_path(inputs, config["rust"]["crate"] + "/Cargo.lock")
    )
    identities = {
        p["name"]
        + "-"
        + p["version"]: {k: p[k] for k in ("name", "version", "source", "sha256")}
        for p in receipt["sources"]
    }
    if identities != expected or len(identities) != len(receipt["sources"]):
        raise ValueError("Rust dependency identities differ from frozen Cargo.lock")
    represented = {item["package"] for item in receipt["licenses"]}
    if represented != set(expected) | {"rust-std-1.99.1"}:
        raise ValueError(
            "Rust license closure omits a registry package or standard library"
        )
    standard = {
        item["output"]
        for item in receipt["licenses"]
        if item["package"] == "rust-std-1.99.1"
    }
    if (
        not {
            "notices/rust-std-1.99.1/COPYRIGHT-library.html",
            "notices/rust-std-1.99.1/licenses/MIT.txt",
            "notices/rust-std-1.99.1/licenses/Apache-2.0.txt",
        }
        <= standard
    ):
        raise ValueError(
            "Rust standard-library original copyright/license inventory is incomplete"
        )
    for item in receipt["licenses"]:
        support.verify_refs(
            root, [dict(path=item["output"], sha256=item["sha256"], size=item["size"])]
        )
    if (
        snapshot(config["rust"]["rust_sysroot"]["root"])
        != config["rust"]["rust_sysroot"]
    ):
        raise ValueError("Rust sysroot tree changed")
    host.verify(config)
    return receipt


def notice_inputs(inputs, config):
    receipt = verify(inputs, config)
    root = Path(config["rust"]["dependencies"]["root"])
    return [
        dict(
            path="notices/rust/" + item["output"].removeprefix("notices/"),
            source=str(root / item["output"]),
            sha256=item["sha256"],
            size=item["size"],
        )
        for item in receipt["licenses"]
    ]
