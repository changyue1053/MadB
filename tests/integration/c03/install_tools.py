"""Install hash-verified upstream generators into a fresh, task-local directory."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tarfile
import tempfile
import urllib.request

from common import ROOT, digest, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    destination = args.root.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    recipe = json.loads(Path(__file__).with_name("toolchain.json").read_text())
    records = {}
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = recipe["rust"]
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
        env.pop(key, None)
    with tempfile.TemporaryDirectory(prefix="madb-c03-upstream-") as temp:
        for name, spec in recipe["generators"].items():
            archive = Path(temp) / (name + ".crate")
            with urllib.request.urlopen(spec["source"], timeout=60) as response:
                archive.write_bytes(response.read(16 * 1024 * 1024))
            if digest(archive) != "sha256:" + spec["archive_sha256"]:
                raise RuntimeError(f"Source archive mismatch: {name}")
            with tarfile.open(archive) as package:
                package.extractall(temp, filter="data")
            source = Path(temp) / f"{spec['package']}-{spec['version']}"
            if digest(source / "Cargo.lock") != "sha256:" + spec["cargo_lock_sha256"]:
                raise RuntimeError(f"Upstream dependency lock mismatch: {name}")
            subprocess.run(["cargo", "install", "--locked", "--path", str(source), "--root", str(destination)],
                           cwd=ROOT, env=env, check=True, timeout=300)
            records[name] = {**spec, "binary_sha256": digest(destination / "bin" / name)}
    write_json(destination / "receipt.json", {"rust": recipe["rust"], "generators": records})
    print(f"Verified generators installed: {destination}")


if __name__ == "__main__":
    main()
