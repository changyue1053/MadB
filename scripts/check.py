"""Run repository checks, including frozen execution and C05/C06 SDK invariants; failures never become skipped passes."""
from pathlib import Path
import importlib.metadata
import os
import platform
import shutil
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def require_environment():
    expected_python = (ROOT / ".python-version").read_text().strip()
    if platform.python_version() != expected_python:
        raise RuntimeError(f"Use Python {expected_python}; got {platform.python_version()}")
    node = shutil.which("node")
    if not node:
        raise RuntimeError("Node.js is required; see README")
    expected_node = (ROOT / ".node-version").read_text().strip()
    actual_node = subprocess.check_output([node, "--version"], text=True).strip().removeprefix("v")
    if actual_node != expected_node:
        raise RuntimeError(f"Use Node.js {expected_node}; got {actual_node}")
    expected_rust = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    actual_rust = subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).split()[1]
    if actual_rust != expected_rust:
        raise RuntimeError(f"Use project Rust {expected_rust}; got {actual_rust}")
    for line in (ROOT / "requirements-dev.lock").read_text().splitlines():
        if "==" not in line or line.startswith("#"):
            continue
        name, expected = line.split()[0].split("==", 1)
        actual = importlib.metadata.version(name)
        if actual != expected:
            raise RuntimeError(f"Install requirements-dev.lock: {name} needs {expected}, got {actual}")
    return node


def main():
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    node = require_environment()
    commands = [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "check", "--workspace", "--locked"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"],
        ["cargo", "test", "--workspace", "--all-features", "--locked"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/contract"],
        [node, "tests/contract/canonical_vectors.cjs"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c03"],
        [sys.executable, "tests/integration/c03/cli_install.py"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_process"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_inputs"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_tasks"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_dag"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_observations"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c04_package"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c05_pipeline"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c06_tools"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c06_project"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration/c06_reader"],
        [sys.executable, "scripts/check_native_sdk.py"],
        [sys.executable, "tests/architecture/check_dependencies.py"],
        [sys.executable, "-m", "unittest", "discover", "-s", "tests/architecture"],
        [sys.executable, "docs/tools/check_docs.py", "--node", node],
        [sys.executable, "-m", "unittest", "discover", "-s", "docs/tools/tests"],
        [node, "docs/tools/tests/panorama_smoke.cjs"],
    ]
    for command in commands:
        print("+ " + " ".join(command), flush=True)
        subprocess.run(command, cwd=ROOT, check=True)
    print("Repository checks passed, including C05/C06 SDK invariants. Native candidate and independent Lab certification remain separate.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, importlib.metadata.PackageNotFoundError, subprocess.CalledProcessError) as error:
        print(f"Repository checks failed: {error}", file=sys.stderr)
        raise SystemExit(1)
