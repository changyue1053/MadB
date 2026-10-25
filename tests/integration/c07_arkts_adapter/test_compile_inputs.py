from pathlib import Path
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from adapters.toolchains.arkts.worker import checked_dependencies
from adapters.generators.arkts.support import sha


class Dependencies(unittest.TestCase):
    def test_original_identity_and_closed_roots(self):
        with tempfile.TemporaryDirectory(prefix="c07-dep-") as tmp:
            root = Path(tmp).resolve()
            inputs = root / "inputs"
            native = root / "sdk"
            inputs.mkdir()
            native.mkdir()
            source = inputs / "unit.c"
            source.write_text("int value;")
            sdk = native / "sdk.h"
            sdk.write_text("typedef int S;")
            original = {
                "unit.c": {"path": "unit.c", "sha256": sha(source), "size": source.stat().st_size}
            }
            dep = root / "unit.d"
            dep.write_text("madb-object: " + str(source) + " \\\n " + str(sdk) + "\n")
            self.assertEqual(len(checked_dependencies(dep, inputs, native, original)), 2)
            source.write_text("int other;")
            with self.assertRaisesRegex(ValueError, "changed"):
                checked_dependencies(dep, inputs, native, original)

    def test_external_header_and_unlisted_generated_header_rejected(self):
        with tempfile.TemporaryDirectory(prefix="c07-dep-") as tmp:
            root = Path(tmp).resolve()
            inputs = root / "inputs"
            native = root / "sdk"
            inputs.mkdir()
            native.mkdir()
            for path in (root / "external.h", inputs / "undeclared.h"):
                path.write_text("int value;")
                dep = root / "unit.d"
                dep.write_text("madb-object: " + str(path) + "\n")
                with self.subTest(path=path), self.assertRaises(ValueError):
                    checked_dependencies(dep, inputs, native, {})


if __name__ == "__main__":
    unittest.main()
