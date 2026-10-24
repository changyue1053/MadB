"""C02 compile-only ABI shape. This is not P01/P02 or an FFI runtime test."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "docs/examples/first-route"


class AbiShape(unittest.TestCase):
    def test_c_declarations_and_layout_compile(self):
        compiler = shutil.which("clang") or shutil.which("cc")
        self.assertIsNotNone(compiler, "C11 compiler is required; missing tools cannot pass")
        subprocess.run([compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only", str(SOURCE / "consumer.c")], check=True)

    def test_rust_import_shape_compiles_without_linking(self):
        with tempfile.TemporaryDirectory(prefix="madb-abi-shape-") as output:
            subprocess.run(["rustc", "--edition=2024", "--crate-type=lib", "--crate-name=madb_expected_shape",
                            "--emit=metadata", "-Dwarnings", "--out-dir", output, str(SOURCE / "expected.rs")], cwd=ROOT, check=True)


if __name__ == "__main__":
    unittest.main()
