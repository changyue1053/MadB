from pathlib import Path
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from adapters.artifacts.arkts.pack import run_har_command
from adapters.toolchains.arkts.pipeline import Pipeline, FrozenPipeline
from adapters.generators.arkts.support import sha


class CommandDiagnostics(unittest.TestCase):
    def test_real_timeout_retains_partial_outputs_and_original_command(self):
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp)
            commands = []
            argv = [
                sys.executable,
                "-c",
                'import sys,time;print("PARTIAL_OUT",flush=True);print("PARTIAL_ERR",file=sys.stderr,flush=True);time.sleep(3)',
            ]
            with self.assertRaisesRegex(RuntimeError, "could not complete"):
                run_har_command("probe", argv, work, work, dict(os.environ), commands, timeout=0.25)
            self.assertEqual((work / "probe.stdout").read_bytes(), b"PARTIAL_OUT\n")
            self.assertEqual((work / "probe.stderr").read_bytes(), b"PARTIAL_ERR\n")
            receipt = json.loads((work / "probe-command.json").read_text())
            self.assertEqual(receipt["arguments"], argv)
            self.assertIsNone(receipt["exit_code"])
            self.assertIn("error", receipt)
            self.assertEqual(commands, [])


class CandidateLogClosure(unittest.TestCase):
    def fixture(self, root, *, missing=False, corrupt=False, conflict=False):
        obj = Pipeline.__new__(Pipeline)
        obj.root = root
        obj.built = root / "domain/files"
        obj.built.mkdir(parents=True)
        logname = "logs/compile-commands.json"
        original = b'{"stderr":"actual compiler warning"}\n'
        src = root / "runs/run-1/node-0000/work" / logname
        src.parent.mkdir(parents=True)
        src.write_bytes(original)
        log = {"path": logname, "sha256": sha(src), "size": len(original)}
        if missing:
            src.unlink()
        if corrupt:
            src.write_bytes(b"corrupt")
        files = []
        evidence = [{"logs": [log]}]
        if conflict:
            evidence.append({"logs": [dict(log, sha256="sha256:" + "0" * 64)]})
        for i, value in enumerate(evidence):
            name = f"evidence/task-{i:04}.json"
            p = obj.built / name
            p.parent.mkdir(exist_ok=True)
            p.write_text(json.dumps(value))
            files.append({"path": name, "sha256": sha(p), "size": p.stat().st_size})
        (obj.built.parent / "metadata.json").write_text(json.dumps({"files": files}))
        return obj, log, original

    def test_original_logs_are_explicit_package_inputs(self):
        with tempfile.TemporaryDirectory() as temp:
            obj, log, data = self.fixture(Path(temp).resolve())
            with patch.object(
                FrozenPipeline, "package", return_value={"accepted": True}
            ) as delegate:
                self.assertEqual(obj.package("unused"), {"accepted": True})
                delegate.assert_called_once()
            self.assertEqual(obj._package_logs, [log])
            self.assertEqual((obj.root / log["path"]).read_bytes(), data)

    def test_old_different_run_is_not_admitted_and_identical_copies_are_safe(self):
        with tempfile.TemporaryDirectory() as temp:
            obj, log, data = self.fixture(Path(temp).resolve())
            old = obj.root / "runs/00-old/node-0000/work" / log["path"]
            old.parent.mkdir(parents=True)
            old.write_bytes(b"old unrelated compiler output")
            same = obj.root / "runs/02-identical/node-0000/work" / log["path"]
            same.parent.mkdir(parents=True)
            same.write_bytes(data)
            with patch.object(FrozenPipeline, "package", return_value={}):
                obj.package("first")
                obj.package("second")
            self.assertEqual((obj.root / log["path"]).read_bytes(), data)
            self.assertEqual(obj._package_logs, [log])

    def test_existing_log_conflict_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            obj, log, _ = self.fixture(Path(temp).resolve())
            destination = obj.root / log["path"]
            destination.parent.mkdir()
            destination.write_bytes(b"other admitted context")
            with patch.object(FrozenPipeline, "package") as delegate, self.assertRaises(ValueError):
                obj.package("unused")
            delegate.assert_not_called()
            self.assertEqual(destination.read_bytes(), b"other admitted context")

    def test_missing_changed_and_conflicting_log_refuse_before_package(self):
        for mode in ("missing", "corrupt", "conflict"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                obj, _, _ = self.fixture(Path(temp).resolve(), **{mode: True})
                with patch.object(FrozenPipeline, "package") as delegate, self.assertRaises(
                    ValueError
                ):
                    obj.package("unused")
                delegate.assert_not_called()


if __name__ == "__main__":
    unittest.main()
