"""Observe official framework links without accepting missing executable images."""
from pathlib import Path
import errno
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.kotlin_rust import python_runtime
from adapters.toolchains.kotlin_rust import tools


class PythonRuntimeLinkTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.framework = self.root / "Frameworks/Tcl.framework"
        (self.framework / "Versions/8.6").mkdir(parents=True)
        (self.framework / "Versions/Current").symlink_to("8.6")
        self.headers = self.framework / "PrivateHeaders"
        self.headers.symlink_to("Versions/Current/PrivateHeaders")

    def snapshot(self):
        return tools.snapshot(self.root, python_runtime.SCOPE)

    def test_internal_missing_framework_link_is_observed_not_ignored(self):
        entries = {row["path"]: row for row in python_runtime.inventory(self.root)}
        self.assertEqual(entries["Frameworks/Tcl.framework/PrivateHeaders"], {
            "path": "Frameworks/Tcl.framework/PrivateHeaders",
            "symlink": "Versions/Current/PrivateHeaders", "target_missing": True,
        })
        self.assertEqual(entries["Frameworks/Tcl.framework/Versions/Current"], {
            "path": "Frameworks/Tcl.framework/Versions/Current", "symlink": "8.6",
        })

    def test_empty_target_appearance_and_link_retarget_change_inventory(self):
        missing = self.snapshot()
        (self.framework / "Versions/8.6/PrivateHeaders").mkdir()
        self.assertNotEqual(self.snapshot(), missing)
        self.headers.unlink()
        self.headers.symlink_to("Versions/Current/OtherMissingHeaders")
        self.assertNotEqual(self.snapshot(), missing)

    def test_missing_external_target_is_rejected(self):
        self.headers.unlink()
        self.headers.symlink_to(self.root.parent / "missing-outside-runtime")
        with self.assertRaisesRegex(ValueError, "symlink escapes"):
            self.snapshot()

    def test_link_cycle_is_not_mistaken_for_missing_target(self):
        self.headers.unlink()
        self.headers.symlink_to("Loop")
        (self.framework / "Loop").symlink_to("PrivateHeaders")
        with self.assertRaises(OSError) as failure:
            self.snapshot()
        self.assertEqual(failure.exception.errno, errno.ELOOP)

    def test_access_error_does_not_become_missing_target(self):
        realpath = os.path.realpath
        def resolve(path, *, strict=False):
            if Path(path) == self.headers:
                raise PermissionError(errno.EACCES, "access denied", str(path))
            return realpath(path, strict=strict)
        with patch.object(python_runtime.os.path, "realpath", side_effect=resolve):
            with self.assertRaises(PermissionError):
                self.snapshot()

    def test_required_native_image_still_must_exist(self):
        with patch.object(python_runtime.subprocess, "check_output", side_effect=AssertionError("must not run otool")):
            with self.assertRaises(FileNotFoundError):
                python_runtime.native_dependencies("unused", self.root, self.root / "missing-python")


if __name__ == "__main__":
    unittest.main()
