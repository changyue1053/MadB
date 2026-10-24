"""P01 isolated release install/run on the actual CI/local host."""
from pathlib import Path
import json
import os
import subprocess
import tempfile

from common import ROOT, digest


def main():
    with tempfile.TemporaryDirectory(prefix="madb-c03-cli-") as temporary:
        install = Path(temporary) / "installed"
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(ROOT / "target/c03-cli-install")
        for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
            env.pop(key, None)
        subprocess.run(["cargo", "install", "--path", str(ROOT / "crates/madb-cli"), "--locked",
                        "--root", str(install)], cwd=ROOT, env=env, check=True, timeout=300)
        binary = install / "bin/madb"
        # Run outside the source tree, with no Cargo path, tool plugins or cache.
        for args, expected_code, marker in [(["--version"], 0, "madb 0.1.0-dev.0"),
                                             (["--help"], 0, "Read-only snapshot queries"),
                                             (["generate"], 2, "unrecognized subcommand")]:
            result = subprocess.run([str(binary), *args], cwd=temporary,
                                    env={"PATH": os.defpath, "HOME": temporary}, capture_output=True, text=True, timeout=10)
            if result.returncode != expected_code or marker not in result.stdout + result.stderr:
                raise RuntimeError(f"Isolated CLI assertion failed: {args}: {result}")
        print(json.dumps({"probe": "P01-cli", "status": "passed", "binary_sha256": digest(binary),
                          "rust": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
                          "checks": ["isolated-version", "isolated-help", "unimplemented-command-rejected"]}, indent=2))


if __name__ == "__main__":
    main()
