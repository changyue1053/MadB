"""Independent host integration oracle for captured inputs and retained runs."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[3]
PYTHON = Path(sys.executable).resolve()
VALUE = b"captured original bytes"


def reference(path, name):
    data = path.read_bytes()
    return {"path": name, "sha256": "sha256:" + hashlib.sha256(data).hexdigest(), "size": len(data)}


class FrozenInputTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        build = subprocess.run(["cargo", "build", "-p", "madb-host", "--example", "frozen_probe", "--locked"],
                               cwd=ROOT, capture_output=True, text=True)
        if build.returncode:
            raise AssertionError(f"frozen_probe build failed:\n{build.stdout}\n{build.stderr}")
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
        cls.driver = Path(metadata["target_directory"]) / "debug/examples/frozen_probe"
        cls.tool = reference(PYTHON, str(PYTHON))

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-frozen-")
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.parent = self.root / "runs"
        self.parent.mkdir()
        self.run_root = self.parent / "run"
        shutil.copyfile(Path(__file__).with_name("helper.py"), self.source / "helper.py")
        (self.source / "value").write_bytes(VALUE)
        self.refs = [reference(self.source / name, name) for name in ("helper.py", "value")]

    def owned_pids(self):
        path = self.run_root / "work/pids"
        return [int(pid) for pid in path.read_text().splitlines()] if path.is_file() else []

    def owned_live(self, pid):
        result = subprocess.run(["ps", "-ww", "-p", str(pid), "-o", "stat=", "-o", "args="],
                                capture_output=True, text=True, check=False)
        if result.returncode not in (0, 1):
            self.fail(f"process observation failed: {result.stderr}")
        fields = result.stdout.strip().split(maxsplit=1)
        return (len(fields) == 2 and not fields[0].startswith("Z")
                and str(self.run_root) in fields[1] and "../inputs/helper.py" in fields[1])

    def tearDown(self):
        pids = self.owned_pids()
        for pid in pids:
            if self.owned_live(pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 2
        while any(self.owned_live(pid) for pid in pids) and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertFalse([pid for pid in pids if self.owned_live(pid)], "fixture worker failed to stop")
        # Only the fixture owns these paths. Never chmod through tampered links.
        for directory, names, files in os.walk(self.root, followlinks=False):
            Path(directory).chmod(0o700)
            for name in names + files:
                path = Path(directory) / name
                if not path.is_symlink():
                    path.chmod(0o700 if path.is_dir() else 0o600)
        self.temp.cleanup()

    def probe(self, mode="normal", action="none", run_name="run", refs=None):
        config = {
            "source_root": str(self.source), "run_parent": str(self.parent),
            "run_name": run_name, "file_refs": self.refs if refs is None else refs,
            "tool": self.tool, "mode": mode, "action": action,
        }
        config_path = self.root / "fixture.json"
        config_path.write_text(json.dumps(config))
        process = subprocess.run([str(self.driver), str(config_path)], check=True,
                                 capture_output=True, text=True, timeout=12)
        self.assertEqual(process.stderr, "")
        envelope = json.loads(process.stdout)
        self.assertLess(envelope["elapsed_ms"], 6000)
        return envelope["result"]

    def assert_retained(self, result):
        self.assertEqual(Path(result["retained_path"]), self.run_root)
        self.assertTrue(result["retained_after_drop"])
        self.assertTrue(self.run_root.is_dir())

    def assert_no_child(self):
        self.assertFalse(self.owned_pids())

    def test_real_process_reads_copy_and_work_output_survives_drop(self):
        result = self.probe()
        self.assertEqual(result["admission"], "EligibleForValidation")
        expected = {"text": VALUE.decode(), "sha256": hashlib.sha256(VALUE).hexdigest(), "length": len(VALUE)}
        self.assertEqual(json.loads(result["stdout"]), expected)
        self.assertEqual(json.loads((self.run_root / "work/result.json").read_text()), expected)
        self.assertEqual((self.source / "value").read_bytes(), VALUE)
        self.assertNotEqual((self.source / "value").stat().st_ino,
                            (self.run_root / "inputs/value").stat().st_ino)
        self.assert_retained(result)

    def test_post_capture_source_change_does_not_change_consumed_snapshot(self):
        result = self.probe(action="change-source")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(json.loads(result["stdout"])["text"], VALUE.decode())
        self.assertEqual((self.run_root / "inputs/value").read_bytes(), VALUE)
        self.assertEqual((self.source / "value").read_bytes(), b"new source after capture")
        self.assert_retained(result)

    def test_prepared_layout_permissions_and_nested_inputs(self):
        nested = self.source / "nested/deep/extra"
        nested.parent.mkdir(parents=True)
        nested.write_bytes(b"nested")
        result = self.probe(action="prepare-only", refs=self.refs + [reference(nested, "nested/deep/extra")])
        self.assertEqual(result["phase"], "prepared")
        for path in (self.run_root, self.run_root / "work"):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700)
        for path in (self.run_root / "inputs", self.run_root / "inputs/nested", self.run_root / "inputs/nested/deep"):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o500)
        for path in (self.run_root / "inputs/value", self.run_root / "inputs/helper.py", self.run_root / "inputs/nested/deep/extra"):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o400)
        self.assert_retained(result)

    def test_existing_directory_is_not_overwritten_or_claimed(self):
        self.run_root.mkdir()
        sentinel = self.run_root / "sentinel"
        sentinel.write_bytes(b"owned by another run")
        result = self.probe()
        self.assertEqual(result["phase"], "prepare")
        self.assertIn("INPUT005", result["codes"])
        self.assertIsNone(result["retained_path"])
        self.assertEqual(list(self.run_root.iterdir()), [sentinel])
        self.assertEqual(sentinel.read_bytes(), b"owned by another run")

    def test_existing_symlink_is_not_followed_or_claimed(self):
        destination = self.root / "destination"
        destination.mkdir()
        (destination / "sentinel").write_text("untouched")
        self.run_root.symlink_to(destination, target_is_directory=True)
        result = self.probe()
        self.assertEqual(result["phase"], "prepare")
        self.assertIn("INPUT005", result["codes"])
        self.assertIsNone(result["retained_path"])
        self.assertTrue(self.run_root.is_symlink())
        self.assertEqual([p.name for p in destination.iterdir()], ["sentinel"])

    def test_nested_run_name_is_rejected_before_creating_paths(self):
        result = self.probe(run_name="nested/run")
        self.assertEqual(result["phase"], "prepare")
        self.assertIn("INPUT005", result["codes"])
        self.assertIsNone(result["retained_path"])
        self.assertEqual(list(self.parent.iterdir()), [])

    def test_wrong_expected_digest_cannot_create_a_run(self):
        refs = [dict(value) for value in self.refs]
        refs[1]["sha256"] = "sha256:" + "0" * 64
        result = self.probe(refs=refs)
        self.assertEqual(result["phase"], "capture")
        self.assertIn("INPUT003", result["codes"])
        self.assertFalse(self.run_root.exists())

    def test_missing_declared_input_cannot_create_a_run(self):
        (self.source / "value").unlink()
        result = self.probe()
        self.assertEqual(result["phase"], "capture")
        self.assertIn("INPUT003", result["codes"])
        self.assertFalse(self.run_root.exists())

    def test_symlink_hardlink_and_fifo_sources_are_rejected(self):
        target = self.source / "value"
        original = self.root / "original"
        original.write_bytes(VALUE)
        for kind in ("symlink", "hardlink", "fifo"):
            with self.subTest(kind=kind):
                target.unlink()
                if kind == "symlink":
                    target.symlink_to(original)
                elif kind == "hardlink":
                    os.link(original, target)
                else:
                    os.mkfifo(target)
                result = self.probe()
                self.assertEqual(result["phase"], "capture")
                self.assertIn("HOST002", result["codes"])
                self.assertFalse(self.run_root.exists())

    def test_pre_execution_copy_and_inventory_changes_prevent_spawn(self):
        actions = ("pre-tamper", "pre-extra-file", "pre-extra-directory", "pre-missing", "pre-symlink", "pre-hardlink")
        for index, action in enumerate(actions):
            with self.subTest(action=action):
                self.run_root = self.parent / f"run{index}"
                result = self.probe(action=action, run_name=self.run_root.name)
                self.assertEqual(result["error"], "EXEC017")
                self.assertEqual(result["stage"], "frozen-inputs")
                self.assert_no_child()
                self.assert_retained(result)

    def test_post_execution_copy_and_inventory_changes_reject_zero_exit(self):
        modes = ("mutate-input", "add-file", "add-directory", "remove-input", "replace-symlink", "replace-hardlink")
        for index, mode in enumerate(modes):
            with self.subTest(mode=mode):
                self.run_root = self.parent / f"run{index}"
                result = self.probe(mode=mode, run_name=self.run_root.name)
                self.assertEqual(result["admission"], "Unavailable")
                self.assertIn("code: Some(0)", result["termination"])
                self.assertEqual(result["diagnostic"], "EXEC017")
                self.assertEqual(result["diagnostic_stage"], "frozen-inputs")
                self.assertTrue((self.run_root / "work/result.json").is_file())
                self.assertEqual((self.source / "value").read_bytes(), VALUE)
                self.assert_retained(result)

    def test_non_work_cwd_is_rejected(self):
        result = self.probe(action="bad-cwd")
        self.assertEqual(result["error"], "EXEC002")
        self.assert_no_child()
        self.assert_retained(result)

    def test_cancel_before_start_retains_prepared_directory(self):
        result = self.probe(action="cancel-before")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertFalse(result["started"])
        self.assert_no_child()
        self.assert_retained(result)

    def test_cancel_running_tool_retains_partial_work(self):
        result = self.probe(mode="cancel", action="cancel-ready")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertTrue(result["leader_reaped"])
        self.assertEqual((self.run_root / "work/partial").read_text(), "owned scratch")
        self.assert_retained(result)

    def test_escaped_pipe_holder_retains_directory_and_cleanup_gap(self):
        result = self.probe(mode="escaped-pipe")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertEqual(result["diagnostic"], "EXEC016")
        self.assertFalse(result["cleanup_complete"])
        self.assertTrue(any(self.owned_live(pid) for pid in self.owned_pids()))
        self.assertTrue((self.run_root / "work/result.json").is_file())
        self.assert_retained(result)

    def test_local_pipe_completion_does_not_authorize_directory_deletion(self):
        result = self.probe(mode="escaped-closed-pipes")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertTrue(result["cleanup_complete"])
        self.assertTrue(any(self.owned_live(pid) for pid in self.owned_pids()))
        self.assert_retained(result)


if __name__ == "__main__":
    unittest.main()
