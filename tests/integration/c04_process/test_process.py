"""Real host-process tests. Assertions are independent of the Rust probe driver."""
from pathlib import Path
import hashlib
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[3]
PYTHON = Path(sys.executable).resolve()
LITERAL = 'spaces ; $(touch forbidden) * "quoted"'


def identity(path):
    data = path.read_bytes()
    return "sha256:" + hashlib.sha256(data).hexdigest(), len(data)


class ProcessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(["cargo", "build", "-p", "madb-host", "--example", "process_probe", "--locked"],
                       cwd=ROOT, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version=1", "--locked"], cwd=ROOT))
        cls.driver = Path(metadata["target_directory"]) / "debug/examples/process_probe"
        cls.python_identity = identity(PYTHON)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-process-")
        self.root = Path(self.temp.name)
        (self.root / "work").mkdir()
        shutil.copyfile(Path(__file__).with_name("helper.py"), self.root / "helper.py")

    def owned_pids(self):
        return [int(pid) for name in ("work", "moved")
                if (path := self.root / name / "pids").is_file()
                for pid in path.read_text().splitlines()]

    def owned_live(self, pid):
        result = subprocess.run(["ps", "-p", str(pid), "-o", "stat=", "-o", "args="],
                                capture_output=True, text=True, check=False)
        if result.returncode not in (0, 1):
            self.fail(f"ps failed: {result.stderr}")
        fields = result.stdout.strip().split(maxsplit=1)
        # A reparented zombie is no longer executing. Only the direct child is
        # guaranteed reaped by this runner. Never kill an unrelated reused PID.
        return (len(fields) == 2 and not fields[0].startswith("Z")
                and str(self.root / "helper.py") in fields[1])

    def tearDown(self):
        for pid in self.owned_pids():
            if self.owned_live(pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        self.temp.cleanup()

    def assert_workers_stopped(self):
        pids = self.owned_pids()
        self.assertTrue(pids, "fixture must have started")
        deadline = time.monotonic() + 2
        while any(self.owned_live(pid) for pid in pids) and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertFalse([pid for pid in pids if self.owned_live(pid)])

    def run_probe(self, mode="normal", action="none", tool=None, digest=None):
        tool = tool or PYTHON
        expected, size = self.python_identity if tool == PYTHON else identity(tool)
        result = subprocess.run(
            [str(self.driver), str(tool), digest or expected, str(size), str(self.root), mode, action],
            env={**os.environ, "SECRET": "ambient-secret-must-not-arrive"},
            capture_output=True, text=True, timeout=12, check=True,
        )
        self.assertEqual(result.stderr, "", "tool stderr must not escape to the host terminal")
        envelope = json.loads(result.stdout)
        self.assertLess(envelope["elapsed_ms"], 6000, "bounded execution did not return promptly")
        return envelope["result"]

    def assert_local_cleanup(self, result):
        self.assertTrue(result["leader_reaped"])
        self.assertTrue(result["stdout_eof"])
        self.assertTrue(result["stderr_eof"])
        self.assertTrue(result["cleanup_complete"])
        self.assertFalse(result["pending_cleanup"])

    def wrapper(self, content=None):
        tool = self.root / "tool"
        tool.write_text(content or f'#!/bin/sh\nexec {shlex.quote(str(PYTHON))} "$@"\n')
        tool.chmod(0o755)
        return tool

    def test_argument_environment_directory_and_input_are_explicit(self):
        result = self.run_probe()
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(json.loads(result["stdout"]), {
            "arguments": [LITERAL], "allowed": "explicit-value", "ambient": None,
            "cwd": "work", "input": "hello input"})
        self.assertFalse((self.root / "work/forbidden").exists())
        self.assertEqual(result["cancellation_after"], "AlreadyFinished")
        self.assert_local_cleanup(result)

    def test_cancel_before_start_creates_no_child(self):
        result = self.run_probe(action="before")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertFalse(result["started"])
        self.assertFalse(self.owned_pids())

    def test_wrong_tool_identity_fails_before_spawn(self):
        result = self.run_probe(digest="sha256:" + "0" * 64)
        self.assertEqual(result["error"], "EXEC011")
        self.assertFalse(self.owned_pids())

    def test_environment_and_nul_rejected_before_spawn(self):
        for mode, code in [("bad-env", "EXEC003"), ("nul-arg", "EXEC002")]:
            with self.subTest(mode=mode):
                result = self.run_probe(mode)
                self.assertEqual(result["error"], code)
                self.assertNotIn("never-leak-this", json.dumps(result))
                self.assertFalse(self.owned_pids())

    def test_plain_text_tool_cannot_fall_back_to_a_shell(self):
        for prefix in ("", "#!\n", "#!   \n", "\x7fELF\n"):
            with self.subTest(prefix=repr(prefix)):
                result = self.run_probe(tool=self.wrapper(prefix + "touch forbidden\n"))
                self.assertEqual(result["error"], "EXEC010")
                self.assertEqual(result["stage"], "spawn")
                self.assertFalse((self.root / "work/forbidden").exists())
                self.assertFalse(self.owned_pids())

    def test_missing_interpreter_reports_spawn_failure(self):
        result = self.run_probe(tool=self.wrapper("#!/madb-fixture-no-such/interpreter\n"))
        self.assertEqual(result["error"], "EXEC010")
        self.assertEqual(result["stage"], "spawn")
        self.assertFalse(self.owned_pids())

    def test_work_directory_symlink_is_rejected(self):
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "sentinel").write_text("untouched")
        (self.root / "work").rmdir()
        (self.root / "work").symlink_to(outside, target_is_directory=True)
        result = self.run_probe()
        self.assertEqual(result["stage"], "work-directory")
        self.assertFalse(self.owned_pids())
        self.assertEqual(list(outside.iterdir()), [outside / "sentinel"])

    def test_nonzero_exit_and_signal_are_not_eligible(self):
        for mode, status in [("exit-failure", "code: Some(23)"), ("self-signal", "signal: Some(15)")]:
            with self.subTest(mode=mode):
                result = self.run_probe(mode)
                self.assertEqual(result["admission"], "Unavailable")
                self.assertIn(status, result["termination"])
                self.assert_local_cleanup(result)

    def test_stdout_limit_stops_a_flood(self):
        result = self.run_probe("stdout-flood")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertIn("StdoutLimit", result["termination"])
        self.assertEqual(len(result["stdout"]), 1024)
        self.assertGreater(result["stdout_bytes"], 1024)
        self.assert_local_cleanup(result)
        self.assert_workers_stopped()

    def test_stderr_is_drained_but_retained_with_a_cap(self):
        result = self.run_probe("stderr-flood")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(len(result["stderr"]), 256)
        self.assertEqual(result["stderr_bytes"], 2 * 1024 * 1024)
        self.assertTrue(result["stderr_truncated"])
        self.assert_local_cleanup(result)

    def test_stdin_and_stderr_backpressure_do_not_deadlock(self):
        result = self.run_probe("backpressure")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertEqual(json.loads(result["stdout"]), {
            "length": 1024 * 1024, "sha256": hashlib.sha256(b"a" * (1024 * 1024)).hexdigest()})
        self.assertEqual(result["stdin_written"], 1024 * 1024)
        self.assert_local_cleanup(result)

    def test_deadline_grace_and_escalation(self):
        for mode, forced in [("sleep", False), ("ignore-term", True), ("no-read", True)]:
            with self.subTest(mode=mode):
                result = self.run_probe(mode)
                self.assertEqual(result["admission"], "Unavailable")
                self.assertIn("Deadline", result["termination"])
                self.assertIn(f"forced: {str(forced).lower()}", result["termination"])
                if mode == "no-read":
                    self.assertLess(result["stdin_written"], 1024 * 1024)
                self.assert_local_cleanup(result)
                self.assert_workers_stopped()

    def test_early_stdin_close_rejects_a_partial_request(self):
        result = self.run_probe("close-stdin")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertIn("StdinClosed", result["termination"])
        self.assertLess(result["stdin_written"], 1024 * 1024)
        self.assert_local_cleanup(result)

    def test_cancel_stops_a_nonreading_process(self):
        result = self.run_probe("no-read", "on-ready")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertIn("Cancellation", result["termination"])
        self.assertIn("forced: true", result["termination"])
        self.assert_local_cleanup(result)
        self.assert_workers_stopped()

    def test_cancel_kills_same_group_workers(self):
        result = self.run_probe("tree", "on-ready")
        self.assertEqual(result["admission"], "RejectedByCancellation")
        self.assertGreaterEqual(len(self.owned_pids()), 2)
        self.assert_local_cleanup(result)
        self.assert_workers_stopped()

    def test_parent_exit_does_not_leave_same_group_pipe_owner(self):
        result = self.run_probe("parent-exits")
        self.assertEqual(result["admission"], "EligibleForValidation")
        self.assertTrue(result["group_kill_attempted"])
        self.assertGreaterEqual(len(self.owned_pids()), 2)
        self.assert_local_cleanup(result)
        self.assert_workers_stopped()

    def test_escaped_pipe_owner_reports_cleanup_gap(self):
        result = self.run_probe("escaped-pipe")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertEqual(result["diagnostic"], "EXEC016")
        self.assertIn("PipeTimeout", result["termination"])
        self.assertTrue(result["leader_reaped"])
        self.assertFalse(result["stdout_eof"])
        self.assertFalse(result["stderr_eof"])
        self.assertFalse(result["cleanup_complete"])
        self.assertFalse(result["pending_cleanup"])
        self.assertTrue(any(self.owned_live(pid) for pid in self.owned_pids()),
                        "fixture must demonstrate the process-group limit")

    def test_changed_tool_invalidates_a_zero_exit(self):
        result = self.run_probe("wait-release", "change-tool", tool=self.wrapper())
        self.assertEqual(result["admission"], "Unavailable")
        self.assertEqual(result["diagnostic"], "EXEC011")
        self.assert_local_cleanup(result)

    def test_changed_directory_invalidates_a_zero_exit(self):
        result = self.run_probe("wait-release", "change-directory")
        self.assertEqual(result["admission"], "Unavailable")
        self.assertEqual(result["diagnostic_stage"], "work-directory")
        self.assert_local_cleanup(result)


if __name__ == "__main__":
    unittest.main()
