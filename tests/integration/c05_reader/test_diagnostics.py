"""Selected diagnostics surface through real cbindgen, Clang and core finalizer."""
from copy import deepcopy
from pathlib import Path
import sys
import tempfile
import unittest

import test_reader as reader

ROOT = reader.ROOT


class DiagnosticsReader(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        reader.NativeReader.setUpClass.__func__(cls)
        sys.path.insert(0, str(ROOT))
        from adapters.toolchains.rust_c.project import prepare
        from adapters.toolchains.rust_c.tools import discover
        cls.temporary = tempfile.TemporaryDirectory(prefix="madb-l02-reader-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        observation = discover(ROOT, cls.tools["bindgen"]["path"], cls.tools["cbindgen"]["path"])
        cls.inputs = cls.root / "inputs"
        cls.prepared = prepare(ROOT, cls.inputs, observation, "c-to-rust-diagnostics")
        cls.config = cls.prepared["config"]
        cls.request = cls.prepared["read"]
        scratch = cls.root / "read"
        scratch.mkdir()
        _, cls.surface, cls.seed = cls.driver._read(cls.request, cls.config, cls.inputs, scratch)
        cls.contract = cls.driver.metadata(cls.request, cls.config, cls.inputs, scratch)

    def test_nine_real_roles_finalize_without_new_ir_types(self):
        self.assertEqual(len(self.contract["symbols"]), 9)
        types = {item["id"]: item["definition"] for item in self.contract["types"]}
        self.assertEqual(types["diagnostic_pull"], dict(kind="record", layout="logical", fields=[
            dict(name="present", type_id="u64"), dict(name="event", type_id="diagnostic_event"),
            dict(name="stats", type_id="diagnostic_stats")]))
        self.assertEqual({item["subject"] for item in self.contract["observations"]}, {"error", "diagnostic_event", "diagnostic_stats"})
        shapes = {item["public_id"]: item["shape"] for item in self.contract["symbols"]}
        self.assertEqual(shapes["madb.engine.diagnostics_drain.v1"]["parameters"], [])
        self.assertEqual(shapes["madb.engine.diagnostics_drain.v1"]["result"], "diagnostic_pull")
        for item in self.prepared["project"]["policies"]:
            if item["selector"]["id"].startswith("madb.engine.diagnostics_") and item["slot"]["kind"] == "call":
                self.assertEqual(item["values"], dict(threading="caller", blocking="non-blocking", delivery="work-completed",
                    cancellation="none", cancellation_confirmation="not-supported"))

    def test_selected_surface_rejects_wrong_or_extra_declarations(self):
        from support import NativeError
        mutations = [lambda x: x["records"]["MadbDiagnosticEvent"][6].update(type="u64"),
                     lambda x: x["layouts"]["MadbDiagnosticStats"].update(size=48),
                     lambda x: x["functions"]["madb_fixture_diagnostics_drain"]["parameters"][0].update(type="MadbDiagnosticEvent **"),
                     lambda x: x["aliases"].update(Undeclared="u64"),
                     lambda x: x["constants"].update(MADB_UNDECLARED=1),
                     lambda x: x["functions"].pop("madb_fixture_diagnostics_configure")]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                surface = deepcopy(self.surface)
                mutate(surface)
                with self.assertRaises(NativeError) as error:
                    self.lowering.validate_surface(surface, self.config)
                self.assertIn(error.exception.code, ("NATIVE004", "NATIVE005"))
        config = dict(self.config, diagnostics=False)
        with self.assertRaises(NativeError) as error:
            self.lowering.validate_surface(self.surface, config)
        self.assertEqual(error.exception.code, "NATIVE001")

    def test_non_boolean_and_c_provider_diagnostics_rejected(self):
        from support import NativeError
        for value in (None, 0, 1, "true", [], {}):
            with self.subTest(value=value), self.assertRaises(NativeError) as error:
                self.driver._verify_context(self.request, dict(self.config, diagnostics=value), self.inputs)
            self.assertEqual(error.exception.code, "NATIVE001")
        with self.assertRaises(NativeError):
            self.driver._verify_context(self.request, dict(self.config, provider="c", diagnostics=True), self.inputs)

    def test_diagnostics_generation_replays_real_identity_and_checked_header(self):
        from support import canonical
        (self.inputs / "contract.json").write_bytes(canonical(self.contract))
        request = dict(self.request, schema="madb.generate-request/1", contract=dict(path="contract.json"),
            contract_digest=self.contract["structure_digest"], policy_digest=self.contract["policy_digest"],
            input_digest=self.contract["source_digest"], task_digest=reader.digest(b"fixed L02 reader test task"))
        runs = []
        for name in ("first", "second"):
            work = self.root / name
            work.mkdir()
            output = self.driver.generate(request, self.config, self.inputs, work)
            runs.append((output, {item["path"]: (work / item["path"]).read_bytes() for item in output["files"]}))
        self.assertEqual(runs[0], runs[1])
        self.assertEqual(set(runs[0][1]), {"include/madb_rust.h", "include/madb_rust_client.h", "identity.json", "identity.rs", "responsibilities.md"})
        ownership = runs[0][1]["responsibilities.md"].decode()
        rows = ownership.split("## Responsibilities\n", 1)[1].split("## Effective policy inputs\n", 1)[0]
        self.assertEqual(sum(line.startswith("| ") for line in rows.splitlines()) - 2, 33)
        self.assertIn(b"madb_fixture_diagnostics_drain", runs[0][1]["include/madb_rust.h"])
        self.assertIn(b"madb_engine_create_checked", runs[0][1]["include/madb_rust_client.h"])

    def test_worker_feature_selection_is_strict(self):
        from adapters.toolchains.rust_c import worker
        for value in (None, 1, "false"):
            with self.assertRaises(ValueError):
                worker.phase_outputs(dict(self.config, diagnostics=value), "compile")
        with self.assertRaises(ValueError):
            worker.phase_outputs(dict(self.config, provider="c", diagnostics=True), "compile")

    def test_responsibility_asset_binds_actual_source_and_checked_context(self):
        from ownership import render_ownership
        from support import NativeError, canonical
        original = render_ownership(self.contract, self.request, self.config, self.inputs)
        for field in ("component", "selector"):
            request = deepcopy(self.request)
            request[field]["sha256" if field == "component" else "profile_digest"] = "sha256:" + "f" * 64
            with self.subTest(field=field), self.assertRaises(NativeError) as error:
                render_ownership(self.contract, request, self.config, self.inputs)
            self.assertEqual(error.exception.code, "NATIVE006")
        source = self.inputs / "sdk/rust/engine/src/runtime.rs"
        before = source.read_bytes()
        try:
            source.write_bytes(bytes([before[0] ^ 1]) + before[1:])
            with self.assertRaises(NativeError) as error:
                render_ownership(self.contract, self.request, self.config, self.inputs)
            self.assertEqual(error.exception.code, "NATIVE006")
        finally:
            source.write_bytes(before)
        self.assertEqual(render_ownership(self.contract, self.request, self.config, self.inputs), original)
        self.assertIsNone(render_ownership(self.contract, self.request, dict(self.config, source="other/lib.rs"), self.inputs))
        other = deepcopy(self.request)
        other["component"]["id"] = "another.reader"
        self.assertIsNone(render_ownership(self.contract, other, self.config, self.inputs))
        profile_path = self.inputs / self.request["profile"]["path"]
        profile_bytes = profile_path.read_bytes()
        try:
            data = canonical(dict(self.prepared["profile"], route="OTHER-ROUTE"))
            profile_path.write_bytes(data)
            request = deepcopy(self.request)
            request["profile"].update(sha256=reader.digest(data), size=len(data))
            with self.assertRaises(NativeError) as error:
                render_ownership(self.contract, request, self.config, self.inputs)
            self.assertEqual(error.exception.code, "NATIVE006")
        finally:
            profile_path.write_bytes(profile_bytes)


if __name__ == "__main__":
    unittest.main()
