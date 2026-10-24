"""Install the locked native generators into a fresh directory and record their images."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tarfile
import tempfile
import urllib.request

from .common import sha_file

ROOT = Path(__file__).resolve().parents[3]
RECIPE = Path(__file__).with_name("toolchain.json")
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024


def install(destination):
    """Verify archives and upstream dependency locks before running cargo install.

    The destination must be new. A receipt is written only after both locked
    generators install successfully; its existing C03 shape remains consumable
    by the native toolchain discovery path.
    """
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=False)
    recipe = json.loads(RECIPE.read_text())
    records = {}
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = recipe["rust"]
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        env.pop(key, None)
    with tempfile.TemporaryDirectory(prefix="madb-native-upstream-") as temp:
        for name, spec in recipe["generators"].items():
            archive = Path(temp) / (name + ".crate")
            with urllib.request.urlopen(spec["source"], timeout=60) as response:
                content = response.read(MAX_ARCHIVE_BYTES + 1)
            if len(content) > MAX_ARCHIVE_BYTES:
                raise RuntimeError(f"Source archive exceeds byte budget: {name}")
            archive.write_bytes(content)
            if sha_file(archive) != "sha256:" + spec["archive_sha256"]:
                raise RuntimeError(f"Source archive mismatch: {name}")
            with tarfile.open(archive) as package:
                package.extractall(temp, filter="data")
            source = Path(temp) / f"{spec['package']}-{spec['version']}"
            if sha_file(source / "Cargo.lock") != "sha256:" + spec["cargo_lock_sha256"]:
                raise RuntimeError(f"Upstream dependency lock mismatch: {name}")
            subprocess.run(["cargo", "install", "--locked", "--path", str(source), "--root", str(destination)],
                           cwd=ROOT, env=env, check=True, timeout=300)
            records[name] = {**spec, "binary_sha256": sha_file(destination / "bin" / name)}
    (destination / "receipt.json").write_text(
        json.dumps({"rust": recipe["rust"], "generators": records}, ensure_ascii=False, indent=2) + "\n")
    print(f"Verified generators installed: {destination}")
    return destination


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True,
                        help="new installation directory (existing directories are rejected)")
    args = parser.parse_args(argv)
    install(args.root)


if __name__ == "__main__":
    main()
