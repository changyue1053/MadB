"""Compiler workspaces are exclusive, portable and refuse to clear unowned/still-active files."""
from pathlib import Path
import copy
import json
import os
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.skie import workspace


class SkieWorkspaceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.base = self.root / "compiler" / "skie"
        self.patcher = patch.object(workspace, "BASE", self.base)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.key = "a" * 64

    def test_identity_changes_for_content_not_install_paths(self):
        source = [{"path": "Boundary.kt", "sha256": "sha256:a", "size": 1}]
        observed = {"tools": {"compiler": {"id": "native", "version": "1", "sha256": "sha256:b", "size": 2, "path": "/a/bin"}},
            "trees": {"deps": {"sha256": "sha256:c", "entries": 1, "size": 3, "root": "/a/deps"}}, "xcode": {"build": "locked"}}
        changed = copy.deepcopy(observed)
        changed["tools"]["compiler"]["path"] = "/b/bin"
        changed["trees"]["deps"]["root"] = "/b/deps"
        first = workspace.identity(source, observed, "profile", "interface")
        self.assertEqual(first, workspace.identity(source, changed, "profile", "interface"))
        changed["tools"]["compiler"]["sha256"] = "sha256:d"
        self.assertNotEqual(first, workspace.identity(source, changed, "profile", "interface"))

    def test_cold_rebuild_and_exclusive_lock(self):
        with workspace.acquire(self.key) as stage:
            (stage / "project").mkdir()
            (stage / "project/stale").write_text("old")
            with self.assertRaisesRegex(TimeoutError, "same compiler input"):
                with workspace.acquire(self.key, timeout=0.02): pass
        with workspace.acquire(self.key) as stage:
            self.assertEqual({path.name for path in stage.iterdir()}, {"owner.json"})

    def test_tampered_marker_or_unknown_file_is_preserved(self):
        with workspace.acquire(self.key) as stage:
            (stage / "foreign").write_text("keep")
        with self.assertRaisesRegex(ValueError, "Unexpected"):
            with workspace.acquire(self.key): pass
        self.assertEqual((stage / "foreign").read_text(), "keep")
        (stage / "foreign").unlink()
        (stage / "owner.json").write_text('{}')
        with self.assertRaisesRegex(ValueError, "ownership marker"):
            with workspace.acquire(self.key): pass

    def test_symlink_and_unclean_compiler_refuse_reuse(self):
        with workspace.acquire(self.key) as stage:
            (stage / "project").symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            with workspace.acquire(self.key): pass
        (stage / "project").unlink()
        (stage / "compiler-process.json").write_text('{}')
        with self.assertRaisesRegex(ValueError, "orphan-process recovery"):
            with workspace.acquire(self.key): pass
        self.assertTrue((stage / "compiler-process.json").exists())

    def test_exited_leader_retains_identity_until_surviving_child_is_stopped(self):
        import time
        heartbeat = self.root / "heartbeat"
        pid_file = self.root / "grandchild-pid"
        command = """
import os, signal, sys, time
from pathlib import Path
readable, writable = os.pipe()
if os.fork() == 0:
    os.close(readable)
    os.close(1); os.close(2)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    Path(sys.argv[2]).write_text(str(os.getpid()))
    with open(sys.argv[1], 'ab', buffering=0) as stream:
        stream.write(b'alive')
        os.write(writable, b'1'); os.close(writable)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            stream.write(b'.'); time.sleep(.01)
    os._exit(0)
os.close(writable); os.read(readable, 1); os.close(readable)
os._exit(37)
"""
        common = workspace._common
        actual_kill = os.killpg
        observed = []
        def checked_signal(group, number):
            status = os.waitid(os.P_PID, group, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            self.assertIsNotNone(status, "the exited leader must remain waitable at every group signal")
            self.assertEqual(status.si_status, 37)
            observed.append(number)
            return actual_kill(group, number)
        with workspace.acquire(self.key) as stage:
            project = stage / "project"; project.mkdir()
            with patch.object(common.os, "killpg", side_effect=checked_signal):
                code = workspace.execute([sys.executable, "-c", command, str(heartbeat), str(pid_file)],
                    project, dict(os.environ), stage / "build.log")
            self.assertEqual(code, 37)
            self.assertIn(signal.SIGKILL, observed)
            self.assertTrue(pid_file.exists())
            self.assertFalse((stage / "compiler-process.json").exists())
            stable = heartbeat.read_bytes()
            time.sleep(.05)
            self.assertEqual(heartbeat.read_bytes(), stable, "grandchild must stop writing before workspace reuse")
        with workspace.acquire(self.key) as stage:
            self.assertEqual({p.name for p in stage.iterdir()}, {"owner.json"})

    def test_group_observation_failure_preserves_marker(self):
        common = workspace._common
        with workspace.acquire(self.key) as stage:
            project = stage / "project"; project.mkdir()
            with patch.object(common, "_quiescent_group", side_effect=RuntimeError("group observation unavailable")):
                with self.assertRaisesRegex(RuntimeError, "group observation unavailable"):
                    workspace.execute([sys.executable, "-c", "pass"], project, dict(os.environ), stage / "build.log")
            marker = json.loads((stage / "compiler-process.json").read_text())
            self.assertGreater(marker["leader_pid"], 1)
            self.assertEqual(marker["process_group"], marker["leader_pid"])
        with self.assertRaisesRegex(ValueError, "orphan-process recovery"):
            with workspace.acquire(self.key): pass

    def test_cancelled_compiler_group_releases_workspace_for_retry(self):
        ready = self.root / "ready"
        script = """
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from adapters.generators.skie import workspace, build
workspace.BASE=Path(sys.argv[2])
with workspace.acquire('b'*64) as stage:
 project=stage/'project'; project.mkdir()
 command=[sys.executable,'-c',"from pathlib import Path; import time; Path("+repr(sys.argv[3])+").write_text('ready'); time.sleep(60)"]
 build._execute(command,project,{'PATH':'/usr/bin:/bin'},stage/'build.log')
"""
        process = subprocess.Popen([sys.executable, "-c", script, str(ROOT), str(self.base), str(ready)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            import time
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                if process.poll() is not None: self.fail(process.communicate()[1].decode())
                time.sleep(0.01)
            self.assertTrue(ready.exists(), "real compiler child did not start")
            process.send_signal(signal.SIGTERM)
            process.communicate(timeout=8)
            self.assertNotEqual(process.returncode, 0)
            with workspace.acquire("b" * 64) as stage:
                self.assertEqual({p.name for p in stage.iterdir()}, {"owner.json"})
        finally:
            if process.poll() is None:
                process.kill(); process.communicate()


if __name__ == "__main__": unittest.main()
