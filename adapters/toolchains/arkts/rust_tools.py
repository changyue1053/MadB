"""Install the exact public ohrs source with a verified offline Cargo closure."""

from __future__ import annotations
import importlib.util
import json
from pathlib import Path
import shutil
import tarfile
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
SOURCE = "https://static.crates.io/crates/ohrs/ohrs-1.5.0.crate"
ARCHIVE_SHA = "sha256:6ce994723b481c06af38af61d8650cb238a8de0783866a84d7f82016eafa835f"
LOCK_SHA = "sha256:f54eb24c1ce9f3e803bc56cff727e71ba1152f1fc23a2baa792d96d69f79c3d1"


def _ref(p):
    p = Path(p)
    return dict(path=str(p), sha256=support.sha(p), size=p.stat().st_size)


def install(repo, config, archive, cargo_home, output):
    """Preparation only. All registry transport happens before this offline call."""
    host.verify(config)
    repo, archive, cargo_home, output = map(Path, (repo, archive, cargo_home, output))
    if archive.is_symlink() or support.sha(archive) != ARCHIVE_SHA:
        raise ValueError("ohrs official source archive differs from the 1.5.0 pin")
    output.mkdir(parents=True, exist_ok=False)
    source = output / "source"
    source.mkdir()
    seen = set()
    with tarfile.open(archive) as package:
        for member in package:
            parts = member.name.split("/")
            if parts[0] != "ohrs-1.5.0" or any(p in ("", ".", "..") for p in parts):
                raise ValueError("invalid ohrs source archive path")
            if member.name in seen or not (member.isdir() or member.isfile()):
                raise ValueError("duplicate or nonordinary ohrs source member")
            seen.add(member.name)
            if member.isfile():
                if member.size > 128 * 1024 * 1024:
                    raise ValueError("ohrs source member exceeds budget")
                destination = source / member.name
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open("xb") as out, package.extractfile(
                    member
                ) as stream:
                    shutil.copyfileobj(stream, out)
    crate = source / "ohrs-1.5.0"
    if support.sha(crate / "Cargo.lock") != LOCK_SHA:
        raise ValueError("published ohrs lock changed")
    original = snapshot(source)
    # Never normalize archive names to accommodate a case-insensitive volume.
    # Some locked registry inputs contain both README.md and readme.md.
    probe = output / "MadB-Case-Probe"
    probe.write_bytes(b"case-sensitive source closure")
    try:
        case_sensitive = not (output / "madb-case-probe").exists()
    finally:
        probe.unlink()
    if not case_sensitive:
        raise ValueError(
            "ohrs 1.5.0 locked windows-targets source contains README.md and readme.md; "
            "use a case-sensitive filesystem and fresh Cargo registry/src there. "
            "Keep original archive bytes; do not normalize inventory names."
        )
    script = repo / "scripts/prepare_managed_dependencies.py"
    spec = importlib.util.spec_from_file_location("_madb_ohrs_source_verifier", script)
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    env = {
        **host.environment(config),
        "PATH": host.environment(config)["PATH"],
        "RUSTC": str(support.verify_tool(config["tools"]["rustc"])),
        "CARGO_HOME": str(cargo_home),
        "CARGO_NET_OFFLINE": "true",
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
        extra_environment=env,
    )
    records = []
    for p in tomllib.loads((crate / "Cargo.lock").read_text())["package"]:
        if "source" not in p:
            continue
        if p["source"] != "registry+https://github.com/rust-lang/crates.io-index":
            raise ValueError("ohrs dependency is not an exact reviewed registry source")
        item = {k: p[k] for k in ("name", "version", "source")}
        item["sha256"] = "sha256:" + p["checksum"]
        name = p["name"] + "-" + p["version"]
        candidates = list((cargo_home / "registry/cache").glob("*/" + name + ".crate"))
        original_archive = next(
            (c for c in candidates if support.sha(c) == item["sha256"]), None
        )
        if original_archive is None:
            raise ValueError(
                "ohrs original offline dependency archive missing: " + name
            )
        records.append(
            verifier._verify_package(output / "vendor" / name, item, original_archive)
        )
    if {p.name for p in (output / "vendor").iterdir()} != {
        p["name"] + "-" + p["version"] for p in records
    }:
        raise ValueError("ohrs source closure has extra/missing packages")
    home = output / "cargo-home"
    home.mkdir()
    (home / "config.toml").write_text(
        '[source.crates-io]\nreplace-with="madb-vendor"\n[source.madb-vendor]\ndirectory='
        + json.dumps(str(output / "vendor"))
        + "\n"
    )
    env["CARGO_HOME"] = str(home)
    support.command(
        config,
        "cargo",
        [
            "build",
            "--offline",
            "--locked",
            "--release",
            "--manifest-path",
            crate / "Cargo.toml",
            "--target-dir",
            output / "target",
        ],
        cwd=output,
        extra_environment=env,
    )
    if snapshot(source) != original:
        raise ValueError("ohrs source changed while building")
    binary = output / "bin/ohrs"
    support.copy_new(output / "target/release/ohrs", binary)
    binary.chmod(0o755)
    host.verify(config)
    receipt = dict(
        schema="madb.private.ohrs-install/1",
        version="1.5.0",
        host=config["rust"]["host"],
        source_url=SOURCE,
        archive=_ref(archive),
        lock_sha256=LOCK_SHA,
        original_source=original,
        vendor=snapshot(output / "vendor"),
        registry_sources=records,
        source_verifier=_ref(script),
        binary=_ref(binary),
        tools={n: config["tools"][n] for n in ("cargo", "rustc")},
        logs=[_ref(p) for p in sorted((output / "port-commands").glob("*.json"))],
    )
    (output / "receipt.json").write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n"
    )
    return dict(root=str(output), receipt=_ref(output / "receipt.json"))


def verify(installed, config):
    root = Path(installed["root"])
    if _ref(root / "receipt.json") != installed["receipt"]:
        raise ValueError("ohrs installation receipt changed")
    r = support.read_json(root / "receipt.json", 16 * support.MAX_JSON)
    if (
        r.get("schema"),
        r.get("version"),
        r.get("source_url"),
        r.get("lock_sha256"),
    ) != ("madb.private.ohrs-install/1", "1.5.0", SOURCE, LOCK_SHA):
        raise ValueError("ohrs installation has wrong public source identity")
    if r.get("host") != config["rust"]["host"]:
        raise ValueError(
            "ohrs build host closure differs from the selected verified host"
        )
    if r["archive"]["sha256"] != ARCHIVE_SHA or _ref(root / "bin/ohrs") != r["binary"]:
        raise ValueError(
            "ohrs installed executable differs from the locked source build"
        )
    if (
        snapshot(root / "source") != r["original_source"]
        or snapshot(root / "vendor") != r["vendor"]
    ):
        raise ValueError("ohrs source/vendor installation changed")
    return r
