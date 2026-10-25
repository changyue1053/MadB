from pathlib import Path
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from adapters.artifacts.arkts.pack import run_har_command, OBFUSCATION_OPTIONS
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
                run_har_command(
                    "probe", argv, work, work, dict(os.environ), commands, timeout=0.25
                )
            self.assertEqual((work / "probe.stdout").read_bytes(), b"PARTIAL_OUT\n")
            self.assertEqual((work / "probe.stderr").read_bytes(), b"PARTIAL_ERR\n")
            receipt = json.loads((work / "probe-command.json").read_text())
            self.assertEqual(receipt["arguments"], argv)
            self.assertIsNone(receipt["exit_code"])
            self.assertIn("error", receipt)
            self.assertEqual(commands, [])


class PublicDeclarationPackaging(unittest.TestCase):
    def test_parameter_names_are_kept_without_disabling_the_three_obfuscations(self):
        self.assertEqual(
            set(OBFUSCATION_OPTIONS),
            {
                "-enable-property-obfuscation",
                "-enable-export-obfuscation",
                "-enable-filename-obfuscation",
                "-keep-parameter-names",
            },
        )


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
            with patch.object(FrozenPipeline, "package") as delegate, self.assertRaises(
                ValueError
            ):
                obj.package("unused")
            delegate.assert_not_called()
            self.assertEqual(destination.read_bytes(), b"other admitted context")

    def test_missing_changed_and_conflicting_log_refuse_before_package(self):
        for mode in ("missing", "corrupt", "conflict"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                obj, _, _ = self.fixture(Path(temp).resolve(), **{mode: True})
                with patch.object(
                    FrozenPipeline, "package"
                ) as delegate, self.assertRaises(ValueError):
                    obj.package("unused")
                delegate.assert_not_called()

class KotlinCompileReceiptClosure(unittest.TestCase):
    def fixture(self, root):
        pipeline = Pipeline.__new__(Pipeline)
        pipeline.config = {"provider": "kotlin"}
        pipeline.built = root / "build/files"
        pipeline.built.mkdir(parents=True)
        receipt = pipeline.built / "kotlin-compile.json"
        receipt.write_bytes(b'{"actual_ksp":"original accepted bytes"}\n')
        reference = {"path": receipt.name, "sha256": sha(receipt), "size": receipt.stat().st_size}
        evidence = {"phase": "compile", "outputs": [dict(reference)]}
        evidence_path = pipeline.built / "evidence/task-0000.json"
        evidence_path.parent.mkdir()
        inventory = {"schema": "madb.sdk-inventory/1", "files": ["native/sdk.so"]}
        (pipeline.built / "sdk-inventory.json").write_text(json.dumps(inventory))
        self.save_evidence(pipeline, evidence, reference)
        return pipeline, receipt, reference, evidence

    def save_evidence(self, pipeline, evidence, reference):
        path = pipeline.built / "evidence/task-0000.json"
        path.write_text(json.dumps(evidence))
        refs = [{"path": "evidence/task-0000.json", "sha256": sha(path), "size": path.stat().st_size}]
        if reference is not None:
            refs.append(reference)
        (pipeline.built.parent / "metadata.json").write_text(json.dumps({"files": refs}))

    def test_original_committed_compile_receipt_is_public_without_rewriting(self):
        with tempfile.TemporaryDirectory() as temporary:
            obj, receipt, reference, _ = self.fixture(Path(temporary).resolve())
            original = receipt.read_bytes()
            self.assertEqual(obj.public_built(), ["native/sdk.so", "kotlin-compile.json"])
            self.assertEqual(receipt.read_bytes(), original)
            self.assertEqual(sha(receipt), reference["sha256"])
            self.assertEqual(obj.public_built(), ["native/sdk.so", "kotlin-compile.json"])

    def test_other_routes_keep_existing_link_inventory(self):
        for route in ("c", "cpp", "rust"):
            with self.subTest(route=route), tempfile.TemporaryDirectory() as temporary:
                obj, receipt, _, _ = self.fixture(Path(temporary).resolve())
                obj.config["provider"] = route
                receipt.unlink()
                (obj.built.parent / "metadata.json").unlink()
                self.assertEqual(obj.public_built(), ["native/sdk.so"])

    def test_missing_or_changed_receipt_does_not_get_rehashed(self):
        for mode in ("missing", "changed"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                obj, receipt, _, _ = self.fixture(Path(temporary).resolve())
                if mode == "missing":
                    receipt.unlink()
                else:
                    receipt.write_bytes(b"replacement")
                with self.assertRaises((ValueError, FileNotFoundError)):
                    obj.public_built()

    def test_uncommitted_conflicting_duplicate_or_wrong_phase_reference_fails(self):
        for mode in ("uncommitted", "conflict", "duplicate", "wrong-phase"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                obj, _, reference, evidence = self.fixture(Path(temporary).resolve())
                if mode == "uncommitted":
                    reference = None
                elif mode == "conflict":
                    evidence["outputs"][0]["sha256"] = "sha256:" + "0" * 64
                elif mode == "duplicate":
                    evidence["outputs"].append(dict(reference))
                else:
                    evidence["phase"] = "link"
                self.save_evidence(obj, evidence, reference)
                with self.assertRaises(ValueError):
                    obj.public_built()

    def test_inventory_does_not_duplicate_the_accepted_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            obj, _, _, _ = self.fixture(Path(temporary).resolve())
            path = obj.built / "sdk-inventory.json"
            value = json.loads(path.read_text())
            value["files"].append("kotlin-compile.json")
            path.write_text(json.dumps(value))
            self.assertEqual(obj.public_built().count("kotlin-compile.json"), 1)
            value["files"].append("kotlin-compile.json")
            path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "duplicate"):
                obj.public_built()


if __name__ == "__main__":
    unittest.main()
