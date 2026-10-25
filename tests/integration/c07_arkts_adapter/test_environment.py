"""Real preload/startup/owner sentinels; these are host isolation tests."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts.support import clean_environment, command, sha
from adapters.toolchains.arkts.pipeline import Pipeline
from adapters.toolchains.common.pipeline import FrozenPipeline

ISOLATION = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]
INJECTORS = {
    "NODE_OPTIONS": "--require=/not-an-input.cjs",
    "JAVA_TOOL_OPTIONS": "-Dunreviewed=true",
    "JDK_JAVA_OPTIONS": "-Dunreviewed=true",
    "DYLD_INSERT_LIBRARIES": "/not-an-input.dylib",
    "LD_PRELOAD": "/not-an-input.so",
    "RUSTFLAGS": "--cfg unreviewed",
    "RUSTC_WRAPPER": "/not-an-input-wrapper",
    "PYTHONPATH": "/not-an-input-package",
}


class EnvironmentBoundary(unittest.TestCase):
    def test_real_node_preload_is_omitted_by_tool_launch(self):
        node = shutil.which("node")
        self.assertIsNotNone(node, "Node is a required repository check tool")
        node = Path(node).resolve(strict=True)
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp).resolve()
            marker = work / "injected"
            preload = work / "preload.cjs"
            preload.write_text(
                "require('node:fs').writeFileSync(" + json.dumps(str(marker)) + ", 'injected');"
            )
            poisoned = clean_environment({"NODE_OPTIONS": "--require=" + str(preload)})
            subprocess.run(
                [str(node), "-e", "process.stdout.write('control')"],
                env=poisoned,
                check=True,
                capture_output=True,
            )
            self.assertEqual(marker.read_text(), "injected")
            marker.unlink()
            tool = {"path": str(node), "size": node.stat().st_size, "sha256": sha(node)}
            with patch.dict(os.environ, {**INJECTORS, "NODE_OPTIONS": poisoned["NODE_OPTIONS"]}):
                value = command(
                    {"tools": {"node": tool}},
                    "node",
                    ["-e", "process.stdout.write('isolated')"],
                    cwd=work,
                )
            self.assertEqual(value, b"isolated")
            self.assertFalse(marker.exists())

    def test_real_owner_process_receives_only_declared_environment(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            image = root / "owner"
            image.write_text(
                "#!"
                + str(Path(sys.executable).resolve())
                + "\nimport json,os\nprint(json.dumps({k:os.environ[k] for k in "
                + repr(list(INJECTORS))
                + " if k in os.environ}))\n"
            )
            image.chmod(0o700)
            pipeline = Pipeline.__new__(Pipeline)
            pipeline.root = root
            pipeline.binary = image
            pipeline.host_size = image.stat().st_size
            pipeline.host_digest = sha(image)
            pipeline.config = {"phase_timeout_seconds": 2}
            with patch.dict(os.environ, INJECTORS):
                result = pipeline.execute("metadata", {"tasks": []})
            self.assertEqual(json.loads(result), {})
            self.assertIsNone(FrozenPipeline.__new__(FrozenPipeline).owner_environment())

    def test_python_startup_sentinel_is_excluded_before_public_imports(self):
        with tempfile.TemporaryDirectory() as temp:
            poison = Path(temp).resolve()
            marker = poison / "startup"
            (poison / "sitecustomize.py").write_text(
                "from pathlib import Path\nPath(" + repr(str(marker)) + ").write_text('injected')\n"
            )
            env = clean_environment({"PYTHONPATH": str(poison)})
            subprocess.run([sys.executable, "-c", "pass"], env=env, check=True, capture_output=True)
            self.assertEqual(marker.read_text(), "injected")
            marker.unlink()
            result = subprocess.run(
                [sys.executable, *ISOLATION, str(ROOT / "scripts/build_arkts_routes.py"), "--help"],
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("--cli-inventory-sha256", result.stdout)
            self.assertFalse(marker.exists())

    def test_public_entry_rejects_unisolated_startup_before_tools(self):
        result = subprocess.run(
            [sys.executable, "-B", str(ROOT / "scripts/build_arkts_routes.py"), "--help"],
            env=clean_environment(),
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Use Python -I -S -B", result.stderr)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
