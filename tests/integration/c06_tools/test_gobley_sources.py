"""Reject undeclared runtime substitutions and mutable Gobley source installs."""
from pathlib import Path
import json
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.gobley import driver


class GobleySourceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_only_reviewed_runtime_is_derived_without_changing_source_lock(self):
        source = ROOT / "sdk/rust/gobley/Cargo.lock"
        original = source.read_bytes()
        copied = self.root / "Cargo.lock"
        copied.write_bytes(original)
        observation = driver.derive_lock(copied)
        self.assertEqual(source.read_bytes(), original)
        self.assertNotEqual(observation["original_sha256"], observation["derived_sha256"])
        self.assertEqual(copied.read_text().count('name = "uniffi_core"'), 1)
        with self.assertRaisesRegex(ValueError, "canonical Gobley runtime lock"):
            driver.derive_lock(copied)

    def test_unreviewed_registry_checksum_is_rejected_without_mutation(self):
        source = (ROOT / "sdk/rust/gobley/Cargo.lock").read_text()
        checksum = driver.read_lock()["runtime"]["sha256"]
        path = self.root / "Cargo.lock"
        path.write_text(source.replace(checksum, "0" * 64))
        before = path.read_bytes()
        with self.assertRaisesRegex(ValueError, "reviewed archive"):
            driver.derive_lock(path)
        self.assertEqual(path.read_bytes(), before)

    def installation(self):
        names = ["gobley-original", "gobley-patched", "gobley-0.3.7.tar.gz", "uniffi_core-0.29.4.crate"]
        for runtime in ("uniffi_core-0.29.4", "uniffi_core-0.29.4-madb"):
            names.extend([runtime + "/src/ffi/foreignfuture.rs", runtime + "/src/ffi/rustfuture/future.rs"])
        for name in names:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("source fixture: " + name)
        (self.root / "receipt.json").write_text(json.dumps({
            "lock_sha256": driver.digest(driver.HERE / "toolchain.json"),
            "files": {name: driver.digest(self.root / name) for name in names},
        }))

    def test_unrelated_runtime_source_tampering_is_detected(self):
        self.installation()
        driver.verify_install(self.root)
        (self.root / "uniffi_core-0.29.4-madb/src/ffi/rustfuture/future.rs").write_text("changed")
        with self.assertRaisesRegex(ValueError, "installed Gobley file changed"):
            driver.verify_install(self.root)

    def test_added_runtime_source_and_symlinks_are_rejected(self):
        self.installation()
        injected = self.root / "uniffi_core-0.29.4-madb/src/injected.rs"
        injected.write_text("extra source")
        with self.assertRaisesRegex(ValueError, "receipt is incomplete"):
            driver.verify_install(self.root)
        injected.unlink()
        injected.symlink_to("ffi/foreignfuture.rs")
        with self.assertRaisesRegex(ValueError, "source contains a symlink"):
            driver.verify_install(self.root)
