"""The vendor receipt is anchored to locked registry archives, not local checksums."""
from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from scripts import prepare_managed_dependencies as dependencies


class RegistryVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-vendor-proof-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.package = self.root / "fixture-1.0.0"
        self.package.mkdir()
        self.files = {"Cargo.toml": b'[package]\nname="fixture"\nversion="1.0.0"\nlicense="MIT"\n', "src/lib.rs": b"pub const VALUE: u8 = 1;\n"}
        self.archive = self.root / "fixture-1.0.0.crate"
        with tarfile.open(self.archive, "w:gz") as output:
            for name, data in self.files.items():
                entry = tarfile.TarInfo("fixture-1.0.0/" + name)
                entry.size = len(data)
                output.addfile(entry, io.BytesIO(data))
                path = self.package / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        self.item = dict(name="fixture", version="1.0.0", source="registry+https://github.com/rust-lang/crates.io-index", sha256=dependencies.digest(self.archive))
        self.write_checksums()

    def write_checksums(self):
        checksums = {name: dependencies.digest(self.package / name).removeprefix("sha256:") for name in self.files}
        (self.package / ".cargo-checksum.json").write_text(json.dumps(dict(package=self.item["sha256"].removeprefix("sha256:"), files=checksums)))

    def test_original_registry_bytes_and_license_are_accepted(self):
        result = dependencies._verify_package(self.package, self.item, self.archive)
        self.assertEqual(result["license"], "MIT")

    def test_rewritten_local_checksums_cannot_hide_modified_registry_source(self):
        (self.package / "src/lib.rs").write_bytes(b"pub const VALUE: u8 = 9;\n")
        self.write_checksums()
        with self.assertRaisesRegex(ValueError, "differs from verified registry archive"):
            dependencies._verify_package(self.package, self.item, self.archive)

    def test_partial_checksum_inventory_is_rejected(self):
        checksum = self.package / ".cargo-checksum.json"
        data = json.loads(checksum.read_text())
        del data["files"]["src/lib.rs"]
        checksum.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "inventory is incomplete"):
            dependencies._verify_package(self.package, self.item, self.archive)

    def test_vendor_symlink_is_rejected(self):
        (self.package / "injected.rs").symlink_to(self.package / "src/lib.rs")
        with self.assertRaisesRegex(ValueError, "symlink"):
            dependencies._verify_package(self.package, self.item, self.archive)


if __name__ == "__main__":
    unittest.main()
