"""Real DAG execution with explicit source, absence and directory observations."""
from copy import deepcopy
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
_spec = importlib.util.spec_from_file_location("observation_dag_fixture", ROOT / "tests/integration/c04_dag/fixture.py")
fixture = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fixture)


class InputObservations(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = subprocess.run(["cargo", "build", "-p", "madb-cli", "--example", "dag_probe", "--locked"],
                               cwd=ROOT, capture_output=True, text=True)
        if built.returncode:
            raise AssertionError(built.stdout + built.stderr)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
        cls.driver = Path(metadata["target_directory"]) / "debug/examples/dag_probe"
        image = fixture.PYTHON.read_bytes()
        cls.tool = {"path": str(fixture.PYTHON), "sha256": fixture.raw(image), "size": len(image)}

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-observed-dag-")
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.parent = self.root / "runs"
        self.parent.mkdir()
        self.graph_path = self.parent / "graph"
        self.markers = self.root / "markers"
        self.markers.mkdir()
        self.tasks, self.expected = fixture.materialize(self.source, self.markers)
        (self.source / "watched/nested").mkdir(parents=True)
        (self.source / "watched/keep.h").write_bytes(b"untracked directory member")
        (self.source / "unwatched").mkdir()

    def live_helper(self, pid, marker):
        observed = subprocess.run(["ps", "-ww", "-p", str(pid), "-o", "stat=", "-o", "args="],
                                  capture_output=True, text=True)
        self.assertIn(observed.returncode, (0, 1), observed.stderr)
        fields = observed.stdout.strip().split(maxsplit=1)
        return (len(fields) == 2 and not fields[0].startswith("Z")
                and "../inputs/helper.py" in fields[1] and str(marker) in fields[1])

    def tearDown(self):
        for marker in self.markers.glob("*.json"):
            pid = json.loads(marker.read_text())["pid"]
            if self.live_helper(pid, marker):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            deadline = time.monotonic() + 2
            while self.live_helper(pid, marker) and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertFalse(self.live_helper(pid, marker), "owned fixture did not stop")
        for directory, dirs, files in os.walk(self.root, followlinks=False):
            Path(directory).chmod(0o700)
            for name in dirs + files:
                path = Path(directory) / name
                if not path.is_symlink():
                    path.chmod(0o700 if path.is_dir() else 0o600)
        self.temp.cleanup()

    def probe(self, action="none", target=None, absent=None, directories=None, hook="none", observed=True):
        fixture.save_tasks(self.source, self.tasks)
        config = {"source_root": str(self.source), "run_parent": str(self.parent),
                  "marker": str(self.markers / "a.json"), "tool": self.tool, "action": action,
                  "tasks": ["tasks/" + task["id"] + ".json" for task in reversed(self.tasks)],
                  "target": target, "watch_hook": hook}
        if observed:
            config["watch"] = {"absent": ["watched/optional.h"] if absent is None else absent,
                               "directories": ["watched"] if directories is None else directories}
        fixture.save(self.root, "driver.json", config)
        process = subprocess.run([str(self.driver), str(self.root / "driver.json")],
                                 capture_output=True, text=True, timeout=15, check=True)
        self.assertEqual(process.stderr, "")
        envelope = json.loads(process.stdout)
        self.assertLess(envelope["elapsed_ms"], 8000)
        return envelope["result"]

    def assert_retained(self, result, ids):
        self.assertEqual([node["id"] for node in result["nodes"]], ids, result)
        self.assertTrue(result["values_drained"], "retained node owners must not contain accepted values")
        self.assertEqual(result["late_cancel"], "AlreadyFinished")
        self.assertTrue(result["retained_after_drop"])
        self.assertEqual(result["retained_path"], str(self.graph_path))
        for node in result["nodes"]:
            self.assertTrue(Path(node["path"]).is_dir())
            self.assertEqual(node["execution"]["admission"], "EligibleForValidation")
            self.assertTrue(node["execution"]["leader_reaped"])
            self.assertFalse(node["execution"]["pending_cleanup"])

    def assert_success(self, result, ids=("a", "b")):
        self.assertEqual(result["status"], "Validated", result)
        self.assertEqual(set(result["results"]), set(ids))
        self.assert_retained(result, list(ids))
        for node_id in ids:
            expected = self.expected[node_id + ".bin"]
            self.assertEqual(result["results"][node_id]["outputs"],
                             [{"path": node_id + ".bin", "size": len(expected),
                               "sha256": fixture.raw(expected), "bytes": list(expected)}])

    def assert_final_rejection(self, result, code):
        self.assertEqual(result["status"], "Failed", result)
        self.assertIsNone(result["results"])
        self.assertIsNone(result["failed_task"])
        self.assertEqual(result["failure"]["codes"], [code])
        self.assertEqual(result["skipped"], [])
        self.assert_retained(result, ["a", "b"])
        self.assertEqual((self.graph_path / "node-0001/inputs/later.bin").read_bytes(), fixture.LATER)

    def assert_initial_rejection(self, result, code):
        self.assertEqual(result["status"], "Failed", result)
        self.assertIsNone(result["results"])
        self.assertEqual(result["failure"]["codes"], [code])
        self.assertEqual(list(self.markers.glob("*.json")), [])
        self.assert_retained(result, [])

    def test_unchanged_observations_accept_real_graph(self):
        result = self.probe()
        self.assert_success(result)
        self.assertGreater(result["source_reads"], 0)
        self.assertGreater(result["exists_reads"], 0)
        self.assertGreater(result["directory_reads"], 0)

    def test_source_bytes_changed_during_execution_reject_all_results(self):
        result = self.probe(action="change-source")
        self.assert_final_rejection(result, "INPUT004")
        self.assertEqual((self.source / "later.bin").read_bytes(), b"changed external source")

    def test_plain_entry_preserves_frozen_source_behavior(self):
        result = self.probe(action="change-source", observed=False)
        self.assert_success(result)
        self.assertEqual(result["directory_reads"], 0)
        self.assertEqual(result["exists_reads"], 0)

    def test_optional_path_appearing_during_execution_rejects_results(self):
        self.assert_final_rejection(self.probe(action="watch-optional"), "OBS003")
        self.assertTrue((self.source / "watched/optional.h").is_file())

    def test_new_direct_member_rejects_results(self):
        self.assert_final_rejection(self.probe(action="watch-add"), "OBS003")

    def test_removed_direct_member_rejects_results(self):
        self.assert_final_rejection(self.probe(action="watch-remove"), "OBS003")

    def test_same_name_changed_type_rejects_results(self):
        self.assert_final_rejection(self.probe(action="watch-type"), "OBS003")
        self.assertTrue((self.source / "watched/keep.h").is_dir())

    def test_unwatched_directory_change_does_not_reject(self):
        self.assert_success(self.probe(action="watch-unrelated"))

    def test_directory_observation_is_not_member_content_tracking(self):
        self.assert_success(self.probe(action="watch-content"))
        self.assertEqual((self.source / "watched/keep.h").read_bytes(), b"changed untracked content")

    def test_directory_observation_is_not_recursive(self):
        self.assert_success(self.probe(action="watch-nested"))
        self.assertTrue((self.source / "watched/nested/new.h").is_file())

    def test_explicit_nested_directory_observation_detects_change(self):
        self.assert_final_rejection(self.probe(action="watch-nested", directories=["watched", "watched/nested"]), "OBS003")

    def test_dangling_link_does_not_match_expected_absence(self):
        (self.source / "optional.h").symlink_to("does-not-exist")
        self.assert_initial_rejection(self.probe(absent=["optional.h"], directories=[]), "OBS003")

    def test_symlink_directory_member_is_not_silently_ignored(self):
        (self.source / "watched/link.h").symlink_to("keep.h")
        self.assert_initial_rejection(self.probe(), "HOST002")

    def test_fifo_directory_member_is_rejected_without_blocking(self):
        os.mkfifo(self.source / "watched/pipe")
        self.assert_initial_rejection(self.probe(), "HOST002")

    def test_missing_observed_directory_is_not_an_empty_directory(self):
        self.assert_initial_rejection(self.probe(directories=["missing"]), "HOST002")

    def test_non_directory_parent_is_not_absence(self):
        (self.source / "parent").write_bytes(b"file")
        self.assert_initial_rejection(self.probe(absent=["parent/optional.h"], directories=[]), "HOST002")

    def test_named_closure_does_not_read_unselected_external_source(self):
        unrelated = deepcopy(self.tasks[0])
        unrelated.update(id="z", dependencies=[], outputs=["z.bin"],
                         inputs=[fixture.reference("missing.bin", b"unselected source")])
        self.tasks.append(unrelated)
        self.assert_success(self.probe(target="b"))
        self.assertFalse((self.markers / "z.json").exists())

    def test_precancel_does_not_read_source_or_observations(self):
        (self.source / "seed.bin").unlink()
        result = self.probe(action="cancel-before", directories=["missing"])
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertIsNone(result["results"])
        self.assertEqual([result[k] for k in ["source_reads", "exists_reads", "directory_reads"]], [0, 0, 0])
        self.assert_retained(result, [])

    def test_cancel_during_observation_capture_starts_no_nodes(self):
        result = self.probe(hook="cancel-capture")
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertEqual(result["watch_cancel"], "Requested")
        self.assertTrue(result["watch_hook_fired"])
        self.assertIsNone(result["results"])
        self.assertEqual(list(self.markers.glob("*.json")), [])
        self.assert_retained(result, [])

    def test_cancel_during_final_observation_discards_all_values(self):
        result = self.probe(hook="cancel-final")
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertEqual(result["watch_cancel"], "Requested")
        self.assertTrue(result["watch_hook_fired"])
        self.assertIsNone(result["results"])
        self.assert_retained(result, ["a", "b"])

    def test_final_observation_io_error_is_not_treated_as_missing(self):
        result = self.probe(hook="error-final")
        self.assert_final_rejection(result, "OBS003")
        self.assertTrue(result["watch_hook_fired"])
        self.assertIn("TEST_OBSERVE_IO", result["failure"]["message"])


if __name__ == "__main__":
    unittest.main()
