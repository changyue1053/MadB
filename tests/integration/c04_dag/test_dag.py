"""Real serial DAG execution from independently hashed wire tasks and source files."""
from copy import deepcopy
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest

from fixture import ROOT, PYTHON, LATER, materialize, raw, reference, save, save_tasks


class DagExecution(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = subprocess.run(["cargo", "build", "-p", "madb-cli", "--example", "dag_probe", "--locked"],
                               cwd=ROOT, capture_output=True, text=True)
        if built.returncode:
            raise AssertionError(built.stdout + built.stderr)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
        cls.driver = Path(metadata["target_directory"]) / "debug/examples/dag_probe"
        image = PYTHON.read_bytes()
        cls.tool = {"path": str(PYTHON), "sha256": raw(image), "size": len(image)}

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-dag-")
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.parent = self.root / "runs"
        self.parent.mkdir()
        self.graph_path = self.parent / "graph"
        self.markers = self.root / "markers"
        self.markers.mkdir()
        self.tasks, self.expected = materialize(self.source, self.markers)

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

    def probe(self, action="none", target=None):
        save_tasks(self.source, self.tasks)
        config = {"source_root": str(self.source), "run_parent": str(self.parent),
                  "marker": str(self.markers / "a.json"), "tool": self.tool, "action": action,
                  "tasks": ["tasks/" + task["id"] + ".json" for task in reversed(self.tasks)],
                  "target": target}
        path = self.root / "driver.json"
        save(self.root, path.name, config)
        process = subprocess.run([str(self.driver), str(path)], capture_output=True,
                                 text=True, timeout=15, check=True)
        self.assertEqual(process.stderr, "")
        envelope = json.loads(process.stdout)
        self.assertLess(envelope["elapsed_ms"], 8000)
        return envelope["result"]

    def assert_retained(self, result):
        self.assertEqual(result["retained_path"], str(self.graph_path))
        self.assertTrue(result["retained_after_drop"])
        self.assertTrue(result["values_drained"], "node owners must not retain accepted values")
        self.assertEqual(result["late_cancel"], "AlreadyFinished")

    def assert_success(self, result, ids):
        self.assertEqual(result["status"], "Validated", result)
        self.assertEqual([node["id"] for node in result["nodes"]], ids)
        self.assertEqual(set(result["results"]), set(ids))
        for node_id in ids:
            output = result["results"][node_id]["outputs"]
            expected = self.expected[node_id + ".bin"]
            self.assertEqual(output, [{"path": node_id + ".bin", "size": len(expected),
                                       "sha256": raw(expected), "bytes": list(expected)}])
            self.assertEqual(result["results"][node_id]["logs"][0]["bytes"], list(b"completed:" + node_id.encode()))
        self.assert_retained(result)

    def assert_failed_before_b(self, result):
        self.assertEqual(result["status"], "Failed", result)
        self.assertIsNone(result["results"])
        self.assertFalse((self.markers / "b.json").exists())
        self.assert_retained(result)

    def test_real_producer_consumer_uses_validated_result_bytes(self):
        result = self.probe()
        self.assert_success(result, ["a", "b"])
        self.assertEqual((self.graph_path / "node-0001/inputs/a.bin").read_bytes(), self.expected["a.bin"])
        self.assertEqual([node["path"] for node in result["nodes"]],
                         [str(self.graph_path / "node-0000"), str(self.graph_path / "node-0001")])

    def test_real_fork_join_has_deterministic_dependency_order(self):
        # A fresh source tree avoids overwriting any fixture context in place.
        import shutil
        shutil.rmtree(self.source)
        self.source.mkdir()
        self.tasks, self.expected = materialize(self.source, self.markers, fork=True)
        self.assert_success(self.probe(), ["a", "b", "c", "d"])

    def test_named_target_does_not_read_unselected_external_inputs(self):
        unrelated = deepcopy(self.tasks[0])
        unrelated.update(id="z", dependencies=[], outputs=["z.bin"],
                         inputs=[reference("missing.bin", b"unselected source")])
        self.tasks.append(unrelated)
        result = self.probe(target="b")
        self.assert_success(result, ["a", "b"])
        self.assertFalse((self.markers / "z.json").exists())

    def test_named_producer_ignores_unselected_consumer_runtime_identity(self):
        self.tasks[1]["inputs"] = [reference("a.bin", b"different unselected expectation")
                                    if ref["path"] == "a.bin" else ref
                                    for ref in self.tasks[1]["inputs"]]
        result = self.probe(target="a")
        self.assert_success(result, ["a"])
        self.assertFalse((self.markers / "b.json").exists())

    def test_external_source_is_frozen_before_first_node(self):
        self.assert_success(self.probe(action="change-source"), ["a", "b"])
        self.assertEqual((self.source / "later.bin").read_bytes(), b"changed external source")
        self.assertEqual((self.graph_path / "node-0001/inputs/later.bin").read_bytes(), LATER)

    def test_upstream_work_mutation_does_not_change_captured_outputs(self):
        self.assert_success(self.probe(action="change-output"), ["a", "b"])
        self.assertEqual((self.graph_path / "node-0000/work/a.bin").read_bytes(), b"changed live output")
        self.assertEqual((self.graph_path / "node-0001/inputs/a.bin").read_bytes(), self.expected["a.bin"])

    def test_wrong_downstream_reference_never_falls_back_to_old_source(self):
        wrong = b"stale same-named source"
        (self.source / "a.bin").write_bytes(wrong)
        self.tasks[1]["inputs"] = [reference("a.bin", wrong) if ref["path"] == "a.bin" else ref
                                    for ref in self.tasks[1]["inputs"]]
        result = self.probe()
        self.assert_failed_before_b(result)
        self.assertEqual([node["id"] for node in result["nodes"]], ["a"])
        self.assertEqual(result["nodes"][0]["execution"]["admission"], "EligibleForValidation")
        # The scheduler rejects the producer result against the declared consumer
        # identity before attempting the dependent node. The raw process did pass.
        self.assertEqual(result["failed_task"], "a")
        self.assertIn("DAG003", result["failure"]["codes"])

    def test_missing_producer_edge_rejected_even_for_named_target(self):
        self.tasks[1]["dependencies"] = []
        (self.source / "a.bin").write_bytes(self.expected["a.bin"])
        result = self.probe(target="b")
        self.assertEqual(result["phase"], "bind", result)
        self.assertFalse(self.graph_path.exists())
        self.assertEqual(list(self.markers.glob("*.json")), [])

    def test_log_is_not_a_dependency_output(self):
        self.tasks[1]["inputs"] = [reference("logs/a.txt", b"completed:a") if ref["path"] == "a.bin" else ref
                                    for ref in self.tasks[1]["inputs"]]
        result = self.probe()
        self.assert_failed_before_b(result)
        self.assertEqual(result["nodes"], [])
        self.assertEqual(list(self.markers.glob("*.json")), [])
        self.assertIn("INPUT003", result["failure"]["codes"])

    def test_nonzero_upstream_stops_downstream_and_preserves_owner(self):
        self.tasks[0]["arguments"][1] = "nonzero"
        result = self.probe()
        self.assert_failed_before_b(result)
        self.assertEqual(result["failed_task"], "a")
        self.assertEqual(result["skipped"], ["b"])
        node = result["nodes"][0]
        self.assertIn("code: Some(7)", node["execution"]["termination"])
        self.assertTrue(Path(node["path"]).is_dir())

    def test_bad_upstream_evidence_stops_downstream(self):
        self.tasks[0]["arguments"][1] = "bad-json"
        result = self.probe()
        self.assert_failed_before_b(result)
        self.assertEqual(result["nodes"][0]["execution"]["diagnostic"], "EXEC018")
        self.assertIn("FMT001", result["nodes"][0]["execution"]["diagnostic_source"])

    def test_wrong_upstream_file_hash_stops_downstream(self):
        self.tasks[0]["arguments"][1] = "wrong-output-hash"
        result = self.probe()
        self.assert_failed_before_b(result)
        self.assertEqual(result["nodes"][0]["execution"]["diagnostic"], "EXEC018")
        self.assertIn("INPUT003", result["nodes"][0]["execution"]["diagnostic_source"])

    def test_next_node_directory_collision_retains_existing_and_prior_owner(self):
        result = self.probe(action="node-collision")
        self.assert_failed_before_b(result)
        self.assertEqual([node["id"] for node in result["nodes"]], ["a", "b"])
        self.assertEqual(result["nodes"][0]["execution"]["admission"], "EligibleForValidation")
        self.assertIsNone(result["nodes"][1]["execution"])
        self.assertIsNotNone(result["nodes"][1]["preparation_error"])
        self.assertEqual((self.graph_path / "node-0001/existing").read_bytes(), b"unrelated existing directory")
        self.assertEqual((self.graph_path / "node-0000/work/a.bin").read_bytes(), self.expected["a.bin"])

    def test_replaced_graph_root_is_rejected_before_next_node(self):
        result = self.probe(action="replace-graph")
        self.assert_failed_before_b(result)
        self.assertIsNotNone(result["nodes"][1]["preparation_error"])
        self.assertFalse((self.graph_path / "node-0001").exists())
        self.assertTrue(self.graph_path.with_suffix(".moved").is_dir())

    def test_precancelled_graph_does_not_capture_or_start(self):
        (self.source / "seed.bin").unlink()
        result = self.probe(action="cancel-before")
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertEqual(result["nodes"], [])
        self.assertIsNone(result["results"])
        self.assertEqual(list(self.markers.glob("*.json")), [])
        self.assert_retained(result)

    def test_cancel_between_nodes_discards_graph_results_keeps_history(self):
        result = self.probe(action="cancel-after-first")
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertEqual(result["between_cancel"], "Requested")
        self.assertEqual(len(result["nodes"]), 1)
        self.assertEqual(result["nodes"][0]["execution"]["admission"], "EligibleForValidation")
        self.assertIsNone(result["results"])
        self.assertFalse((self.markers / "b.json").exists())
        self.assert_retained(result)

    def test_running_cancel_reaches_active_process_and_stops_downstream(self):
        self.tasks[0]["arguments"][1] = "cancel-running"
        result = self.probe(action="cancel-running")
        self.assertEqual(result["status"], "Cancelled", result)
        self.assertEqual(result["observer_cancel"], "Requested")
        self.assertEqual(result["nodes"][0]["execution"]["admission"], "RejectedByCancellation")
        self.assertTrue(result["nodes"][0]["execution"]["leader_reaped"])
        self.assertIsNone(result["results"])
        self.assertFalse((self.markers / "b.json").exists())
        self.assert_retained(result)


if __name__ == "__main__":
    unittest.main()
