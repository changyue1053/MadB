"""Cargo artifact naming is distinct from ELF SONAME and delivery filename."""

from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts.rust import compiled_library


class ArtifactName(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.manifest = self.root / "author/sdk/rust/arkts-bridge/Cargo.toml"
        self.manifest.parent.mkdir(parents=True)
        self.dist = self.root / "dist/arm64-v8a"
        self.dist.mkdir(parents=True)

    def manifest_text(self, name="madb_arkts_bridge", kind="cdylib"):
        self.manifest.write_text(
            '[lib]\nname = "' + name + '"\ncrate-type = ["' + kind + '"]\n'
        )

    def test_cargo_filename_not_route_soname_selects_the_artifact(self):
        self.manifest_text()
        actual = self.dist / "libmadb_arkts_bridge.so"
        actual.write_bytes(b"synthetic path fixture; not an ELF or target claim")
        self.assertEqual(compiled_library(self.root), actual)
        self.assertFalse((self.dist / "libmadbc07sdkrust.so").exists())

    def test_missing_or_symlinked_cargo_artifact_rejects(self):
        self.manifest_text()
        with self.assertRaisesRegex(ValueError, "regular file"):
            compiled_library(self.root)
        other = self.root / "foreign.so"
        other.write_bytes(b"foreign path fixture")
        (self.dist / "libmadb_arkts_bridge.so").symlink_to(other)
        with self.assertRaisesRegex(ValueError, "symlink"):
            compiled_library(self.root)

    def test_library_name_cannot_select_another_directory_or_non_cdylib(self):
        for name, kind in (
            ("../../foreign", "cdylib"),
            ("bridge", "rlib"),
            ("", "cdylib"),
        ):
            self.manifest_text(name, kind)
            with self.subTest(name=name, kind=kind), self.assertRaisesRegex(
                ValueError, "named Cargo cdylib"
            ):
                compiled_library(self.root)


if __name__ == "__main__":
    unittest.main()
