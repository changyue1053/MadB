"""Reject missing/cross-route/unpinned Rust inputs before tool preparation."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


class RustEntry(unittest.TestCase):
    def command(self, output, route, extra):
        return [
            sys.executable,
            "-I",
            "-S",
            "-B",
            "-X",
            "pycache_prefix=/dev/null",
            str(ROOT / "scripts/build_arkts_routes.py"),
            "--output",
            str(output),
            "--route",
            route,
            "--stop-after",
            "generate",
            "--allow-dirty",
            "--madb-image",
            "/missing-madb",
            "--madb-sha256",
            "0" * 64,
            "--finalizer-image",
            "/missing-finalizer",
            "--finalizer-sha256",
            "0" * 64,
            "--cli-root",
            "/missing-sdk",
            "--cli-inventory",
            str(output.parent / "inventory.json"),
            "--cli-inventory-sha256",
            "0" * 64,
            "--python-image",
            "/missing-python",
            "--java-home",
            "/missing-java",
            *extra,
        ]

    def test_explicit_backend_configuration_pair_and_route(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve() / "candidate"
            cases = [
                ("rust", [], "Rust requires --rust-configuration"),
                (
                    "rust",
                    ["--rust-configuration", "/missing"],
                    "Rust requires --rust-configuration",
                ),
                ("c", ["--rust-configuration", "/missing"], "only valid for --route rust"),
                (
                    "cpp",
                    ["--rust-configuration-sha256", "0" * 64],
                    "only valid for --route rust",
                ),
                ("kotlin", [], "Kotlin requires --kotlin-configuration"),
                (
                    "kotlin",
                    ["--kotlin-configuration", "/missing"],
                    "Kotlin requires --kotlin-configuration",
                ),
                ("c", ["--kotlin-configuration", "/missing"], "only valid for --route kotlin"),
            ]
            for route, extra, expected in cases:
                with self.subTest(route=route, extra=extra):
                    result = subprocess.run(
                        self.command(output, route, extra), capture_output=True, text=True
                    )
                    self.assertEqual(result.returncode, 2)
                    self.assertIn(expected, result.stderr)
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
