"""Independent filesystem and real-process helpers for C04 package acceptance."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]


def byte_digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True) + "\n")


def tree_bytes(root):
    """Read only; capture paths, entry kinds, link targets and regular bytes."""
    if not root.exists():
        return None
    entries = {}
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(parent) / name
            relative = path.relative_to(root).as_posix()
            if path.is_symlink():
                entries[relative] = ("link", os.readlink(path))
            elif path.is_dir():
                entries[relative] = ("directory",)
            elif path.is_file():
                entries[relative] = ("file", byte_digest(path.read_bytes()))
            else:
                entries[relative] = ("special",)
    return entries


def build_binary(name, example=False):
    command = ["cargo", "build", "-p", "madb-cli", "--example" if example else "--bin", name, "--locked"]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise AssertionError(result.stdout + result.stderr)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
    target = Path(metadata["target_directory"]) / "debug"
    return target / "examples" / name if example else target / name


def wait_json(path, process=None, timeout=10):
    """The ready file determines order; elapsed time is only a failure watchdog."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            return json.loads(path.read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            if process is not None and process.poll() is not None:
                out, err = process.communicate(timeout=1)
                raise AssertionError(f"process exited before barrier: {process.returncode}\n{out}\n{err}")
            time.sleep(0.01)
    raise AssertionError(f"timed out waiting for explicit barrier {path}")


class PackageFixture(unittest.TestCase):
    """Own every spawned process before cleaning only this test's temporary tree."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-c04-package-")
        self.root = Path(self.temp.name).resolve()
        self.domain = self.root / "domain"
        self.domain.mkdir()
        self.processes = []

    def spawn(self, arguments):
        process = subprocess.Popen(arguments, cwd=self.root, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True)
        self.processes.append(process)
        return process

    def finish(self, process, expected=0, timeout=15):
        out, err = process.communicate(timeout=timeout)
        self.assertEqual(process.returncode, expected, (out, err))
        return out, err

    def tearDown(self):
        for process in self.processes:
            if process.poll() is None:
                process.kill()
            process.communicate(timeout=3)
        for parent, directories, files in os.walk(self.root, followlinks=False):
            Path(parent).chmod(0o700)
            for name in directories + files:
                path = Path(parent) / name
                if not path.is_symlink():
                    path.chmod(0o700 if path.is_dir() else 0o600)
        self.temp.cleanup()
