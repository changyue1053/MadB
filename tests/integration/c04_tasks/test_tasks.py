"""One-shot task execution against independently built contexts and real files."""
from copy import deepcopy
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest

from fixture import ROOT, PYTHON, VALUE, EXPECTED, LOG, materialize, raw, save, save_task


class TaskExecution(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = subprocess.run(["cargo", "build", "-p", "madb-cli", "--example", "task_probe", "--locked"],
                               cwd=ROOT, capture_output=True, text=True)
        if built.returncode:
            raise AssertionError(built.stdout + built.stderr)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
        cls.driver = Path(metadata["target_directory"]) / "debug/examples/task_probe"
        image = PYTHON.read_bytes()
        cls.tool = {"path": str(PYTHON), "sha256": raw(image), "size": len(image)}

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-task-result-")
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.parent = self.root / "runs"
        self.parent.mkdir()
        self.run_root = self.parent / "run"
        self.marker = self.root / "helper-marker.json"
        self.values = materialize(self.source, self.marker)

    def live_helper(self, pid):
        observed = subprocess.run(["ps", "-ww", "-p", str(pid), "-o", "stat=", "-o", "args="],
                                  capture_output=True, text=True)
        self.assertIn(observed.returncode, (0, 1), observed.stderr)
        fields = observed.stdout.strip().split(maxsplit=1)
        return (len(fields) == 2 and not fields[0].startswith("Z")
                and "../inputs/helper.py" in fields[1] and str(self.marker) in fields[1])

    def tearDown(self):
        if self.marker.is_file():
            pid = json.loads(self.marker.read_text())["pid"]
            if self.live_helper(pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            deadline = time.monotonic() + 2
            while self.live_helper(pid) and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertFalse(self.live_helper(pid), "owned fixture did not stop")
        for directory, dirs, files in os.walk(self.root, followlinks=False):
            Path(directory).chmod(0o700)
            for name in dirs + files:
                path = Path(directory) / name
                if not path.is_symlink():
                    path.chmod(0o700 if path.is_dir() else 0o600)
        self.temp.cleanup()

    def probe(self, mode="normal", action="none", tool=None):
        task = deepcopy(self.values["build-task"])
        if mode != "normal":
            task["arguments"][1] = mode
            save_task(self.source, task)
        config = {"source_root": str(self.source), "run_parent": str(self.parent),
                  "marker": str(self.marker), "tool": tool or self.tool, "action": action}
        config_path = self.root / "driver.json"
        save(self.root, config_path.name, config)
        process = subprocess.run([str(self.driver), str(config_path)], capture_output=True,
                                 text=True, timeout=12, check=True)
        self.assertEqual(process.stderr, "")
        envelope = json.loads(process.stdout)
        self.assertLess(envelope["elapsed_ms"], 6000)
        return envelope["result"]

    def assert_retained(self, result):
        self.assertEqual(result["retained_path"], str(self.run_root))
        self.assertTrue(result["retained_after_drop"])
        self.assertTrue(self.run_root.is_dir())

    def assert_bind_rejected(self, result, code):
        self.assertEqual(result["phase"], "bind", result)
        self.assertIn(code, result["codes"], result)
        self.assertIsNone(result["retained_path"])
        self.assertFalse(self.run_root.exists())
        self.assertFalse(self.marker.exists())

    def assert_result_rejected(self, result, cause):
        self.assertEqual(result["phase"], "execute", result)
        self.assertEqual(result["admission"], "Unavailable", result)
        self.assertIn("code: Some(0)", result["termination"])
        self.assertEqual(result["diagnostic"], "EXEC018", result)
        self.assertEqual(result["diagnostic_stage"], "task-result")
        self.assertIn(cause, result["diagnostic_source"])
        self.assertIsNone(result["value"])
        self.assert_retained(result)

    def test_real_recipe_captures_outputs_and_logs(self):
        result = self.probe()
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(result["validation_calls"], 1)
        expected = {"outputs": [{"path": "output.bin", "size": len(EXPECTED), "sha256": raw(EXPECTED), "bytes": list(EXPECTED)}],
                    "logs": [{"path": "logs/build.txt", "size": len(LOG), "sha256": raw(LOG), "bytes": list(LOG)}]}
        self.assertEqual(result["value"], expected)
        self.assertEqual((self.run_root / "work/output.bin").read_bytes(), EXPECTED)
        self.assertEqual(result["late_cancel"], "AlreadyFinished")
        self.assert_retained(result)

    def test_captured_input_survives_source_changes(self):
        result = self.probe(action="change-source")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(bytes(result["value"]["outputs"][0]["bytes"]), EXPECTED)
        self.assertEqual((self.run_root / "inputs/value").read_bytes(), VALUE)
        self.assertEqual((self.source / "value").read_bytes(), b"changed after preparation")

    def test_captured_result_survives_work_file_changes(self):
        result = self.probe(action="change-result")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(result["value"], result["value_after_change"])
        self.assertEqual(bytes(result["value"]["outputs"][0]["bytes"]), EXPECTED)
        self.assertEqual((self.run_root / "work/output.bin").read_bytes(), b"changed after captured result")

    def test_task_digest_and_source_lineage_are_checked(self):
        original = deepcopy(self.values["build-task"])
        for field, code in [("task_digest", "PLAN002"), ("input_digest", "TASK001")]:
            with self.subTest(field=field):
                task = deepcopy(original)
                task[field] = "sha256:" + "0" * 64
                save_task(self.source, task, recompute=field != "task_digest")
                self.assert_bind_rejected(self.probe(), code)

    def test_task_selector_and_dependencies_are_checked(self):
        original = deepcopy(self.values["build-task"])
        for edit, code in [("selector", "PLAN002"), ("dependencies", "TASK001"), ("cwd", "TASK001")]:
            with self.subTest(edit=edit):
                task = deepcopy(original)
                if edit == "selector":
                    task["selector"]["target"] = "different-target"
                elif edit == "dependencies":
                    task["dependencies"] = ["unexecuted-parent"]
                else:
                    task["working_directory"] = "other"
                save_task(self.source, task)
                self.assert_bind_rejected(self.probe(), code)

    def test_tool_component_is_not_an_installed_image(self):
        task = deepcopy(self.values["build-task"])
        task["tool"] = deepcopy(self.values["read-request"]["component"])
        save_task(self.source, task)
        self.assert_bind_rejected(self.probe(), "LOCK004")

    def test_installed_image_digest_must_match_profile(self):
        tool = dict(self.tool, sha256="sha256:" + "0" * 64)
        self.assert_bind_rejected(self.probe(tool=tool), "LOCK004")

    def test_profile_and_lock_bytes_must_match_references(self):
        for name in ["profiles/rust-c.json", "madb.lock"]:
            with self.subTest(name=name):
                path = self.source / name
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                self.assert_bind_rejected(self.probe(), "FILE004")
                path.write_bytes(original)

    def test_duplicate_environment_and_directory_case_alias_rejected(self):
        original = deepcopy(self.values["build-task"])
        for field, value in [("environment", [{"name": "KEY", "value": "a"}, {"name": "KEY", "value": "b"}]),
                             ("outputs", ["Out/a", "out/b"]), ("outputs", ["out", "out/file"])]:
            with self.subTest(value=value):
                task = deepcopy(original)
                task[field] = value
                save_task(self.source, task)
                result = self.probe()
                self.assert_bind_rejected(result, "TASK001" if field == "outputs" else "HASH004")

    def test_failed_capture_does_not_allocate_or_execute(self):
        (self.source / "value").write_bytes(b"wrong declared source")
        result = self.probe()
        self.assertEqual(result["phase"], "prepare")
        self.assertIn("INPUT003", result["codes"])
        self.assertFalse(self.run_root.exists())
        self.assertFalse(self.marker.exists())

    def test_malformed_payload_rejects_zero_exit(self):
        self.assert_result_rejected(self.probe(mode="bad-json"), "FMT001")

    def test_result_identity_must_match_exact_task(self):
        # Each sub-case uses its own run name and source fixture via a fresh case.
        for mode in ["wrong-task", "wrong-generation", "wrong-profile", "wrong-input"]:
            with self.subTest(mode=mode):
                result = self.probe(mode=mode)
                self.assert_result_rejected(result, "PORT002")
                self.remove_completed_run()

    def remove_completed_run(self):
        # These helpers exit normally; assert their recorded PID stopped before cleanup.
        pid = json.loads(self.marker.read_text())["pid"]
        self.assertFalse(self.live_helper(pid))
        for directory, dirs, files in os.walk(self.run_root, followlinks=False):
            Path(directory).chmod(0o700)
            for name in dirs + files:
                path = Path(directory) / name
                if not path.is_symlink():
                    path.chmod(0o700 if path.is_dir() else 0o600)
        import shutil
        shutil.rmtree(self.run_root)

    def test_file_bytes_and_exact_inventory_are_independently_verified(self):
        cases = [("wrong-output-hash", "INPUT003"), ("wrong-log-hash", "INPUT003"),
                 ("missing-output", "INPUT003"), ("missing-log", "INPUT003"),
                 ("extra-file", "INPUT006"), ("extra-directory", "INPUT006")]
        for mode, code in cases:
            with self.subTest(mode=mode):
                self.assert_result_rejected(self.probe(mode=mode), code)
                self.remove_completed_run()

    def test_unsafe_output_kinds_and_budgets_are_rejected(self):
        cases = [("symlink-output", "HOST002"), ("hardlink-output", "HOST002"),
                 ("fifo-output", "HOST002"), ("oversized-declaration", "INPUT002"),
                 ("oversized-file", "HOST003")]
        for mode, code in cases:
            with self.subTest(mode=mode):
                self.assert_result_rejected(self.probe(mode=mode), code)
                self.remove_completed_run()

    def test_failed_evidence_and_conflicting_logs_rejected(self):
        for mode, code in [("failed-evidence", "PORT004"), ("log-overlap", "TASK002"), ("log-case-alias", "INPUT001")]:
            with self.subTest(mode=mode):
                self.assert_result_rejected(self.probe(mode=mode), code)
                self.remove_completed_run()

    def test_nonzero_process_cannot_be_rescued_by_passed_claim(self):
        result = self.probe(mode="nonzero-claimed-passed")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertIn("code: Some(7)", result["termination"])
        self.assertEqual(result["validation_calls"], 0)
        self.assertIsNone(result["value"])
        self.assert_retained(result)

    def test_input_mutation_prevents_result_validation(self):
        result = self.probe(mode="input-tamper")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertEqual(result["diagnostic"], "EXEC017")
        self.assertEqual(result["validation_calls"], 0)
        self.assertIsNone(result["value"])
        self.assert_retained(result)

    def test_cancel_before_execution_does_not_start_recipe(self):
        result = self.probe(action="cancel-before")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertFalse(result["started"])
        self.assertFalse(self.marker.exists())
        self.assertEqual(result["validation_calls"], 0)
        self.assertIsNone(result["value"])
        self.assert_retained(result)

    def test_cancel_running_recipe_retains_partial_work(self):
        result = self.probe(mode="cancel-running", action="cancel-running")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertEqual(result["observer_cancel"], "Requested")
        self.assertTrue(result["leader_reaped"])
        self.assertEqual(result["validation_calls"], 0)
        self.assertIsNone(result["value"])
        self.assertTrue((self.run_root / "work/output.bin").exists())
        self.assert_retained(result)

    def test_cancel_after_process_exit_during_result_validation(self):
        result = self.probe(action="cancel-validating")
        self.assertIn("code: Some(0)", result["termination"])
        self.assertEqual(result["validation_calls"], 1)
        self.assertEqual(result["validation_cancel"], "Requested")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertIsNone(result["value"])
        self.assertEqual(result["late_cancel"], "AlreadyFinished")
        self.assert_retained(result)

    def test_final_verification_covers_validator_input_and_work_changes(self):
        for action in ["tamper-input-validating", "replace-work-validating"]:
            with self.subTest(action=action):
                result = self.probe(action=action)
                self.assertEqual(result["validation_calls"], 1)
                self.assertIn("code: Some(0)", result["termination"])
                self.assertEqual(result["admission"], "Unavailable")
                self.assertEqual(result["diagnostic"], "EXEC017")
                self.assertIsNone(result["value"])
                self.assert_retained(result)
                self.remove_completed_run()


if __name__ == "__main__":
    unittest.main()
