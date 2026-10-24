"""Public installer delegates real backends and never promotes partial work."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from scripts import install_managed_backends as installer


class BackendInstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-backend-entry-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / "new-installation"

    def test_existing_root_is_never_reused(self):
        self.root.mkdir()
        (self.root / "user-file").write_text("preserve")
        with patch.object(installer.gobley, "install") as implementation:
            with self.assertRaisesRegex(ValueError, "must not exist"):
                installer.install(self.root, ["gobley"])
            implementation.assert_not_called()
        self.assertEqual((self.root / "user-file").read_text(), "preserve")

    def test_duplicate_or_unknown_backend_fails_before_creating_root(self):
        for selected in ([], ["gobley", "gobley"], ["unknown"]):
            with self.assertRaisesRegex(ValueError, "supported backend"):
                installer.install(self.root, selected)
            self.assertFalse(self.root.exists())

    def test_backend_failure_retains_diagnostic_state_without_success_record(self):
        def failed(destination):
            destination.mkdir()
            (destination / "build.log").write_text("compiler diagnostics")
            raise RuntimeError("compile failed")
        with patch.object(installer.gobley, "install", side_effect=failed):
            with self.assertRaisesRegex(RuntimeError, "compile failed"):
                installer.install(self.root, ["gobley"])
        self.assertEqual(json.loads((self.root / "installation.json").read_text())["status"], "failed")
        self.assertFalse((self.root / "gobley-tools.json").exists())
        self.assertEqual((self.root / "gobley/build.log").read_text(), "compiler diagnostics")

    def test_one_selected_backend_delegates_install_and_verification(self):
        def installed(destination):
            destination.mkdir()
            (destination / "receipt.json").write_text('{"source":"fixture"}\n')
        with patch.object(installer.gobley, "install", side_effect=installed) as implementation, patch.object(installer.gobley, "verify_install") as verify, patch.object(installer.uniffi, "install") as other:
            result = installer.install(self.root, ["gobley"])
            implementation.assert_called_once_with(self.root / "gobley")
            verify.assert_called_once_with(self.root / "gobley")
            other.assert_not_called()
        self.assertEqual(set(result), {"gobley"})
        self.assertEqual(json.loads((self.root / "gobley-tools.json").read_text()), result["gobley"])
        self.assertEqual(json.loads((self.root / "installation.json").read_text())["status"], "complete")


if __name__ == "__main__":
    unittest.main()
