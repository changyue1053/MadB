"""Producer negative-fixture guards; these tests do not claim target rejection."""
import importlib.util
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import support
from adapters.toolchains.arkts.native import compose_native

spec = importlib.util.spec_from_file_location(
    "identity_fixtures", ROOT / "sdk/arkts/test-support/build_identity_fixtures.py")
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)


class IdentityFixtures(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="c07-identity-fixture-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        composed = self.inputs / "derived/native"
        compose_native(ROOT, composed, route="c", variant="test")
        self.identity = "madb-arkts/1:" + ":".join(letter * 64 for letter in "abc")
        constants = {
            "GatewayAbi": "a" * 64, "GatewayProfile": "b" * 64, "GatewayBuild": "c" * 64,
            "GatewayIdentity": self.identity, "GatewayRouteIdentity": "c/" + self.identity,
            "GatewayModuleName": "madbc07sdkctest",
        }
        self.write(fixtures.HEADER, '#pragma once\n#include "provider_abi.h"\nnamespace gateway {\n'
                   + "".join("inline constexpr const char* " + key + "=" + json.dumps(value)
                             + ";\n" for key, value in constants.items())
                   + "inline constexpr auto GatewayProviderCreate = ::mp_c_create;\n}\n")
        self.write(fixtures.FACADE, "const EXPECTED_IDENTITY: string = " + json.dumps(self.identity) + ";\n")
        self.write(fixtures.WORKER, "raise AssertionError('prepare-only must not import the worker')\n")
        self.config = dict(variant="test", provider="c", native_module="libmadbc07sdkctest.so",
                           tools={"python": {}}, build={"units": [
                               {"source": "derived/native/src/runtime.cpp", "name": "runtime", "language": "cpp"}]})
        self.write("backend.json", json.dumps(self.config))
        self.write("identity.json", json.dumps({"expected_identity": self.identity}))
        names = re.findall(r"^  (\w+)\(", (composed / "Index.d.ts").read_text(), re.M)
        self.assertEqual(len(names), 31)
        self.write("declarations.json", json.dumps({"gateway": {"methods": [{"name": name} for name in names]}}))
        layout = "sdk/arkts/test-support/contracts/bootstrap-totals-layout.json"
        self.write(layout, (ROOT / layout).read_text())
        self.task_path = self.root / "compile.json"
        self.refresh_task()

    def write(self, name, text):
        path = self.inputs / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def refresh_task(self):
        self.task = dict(schema="madb.build-task/1", phase="compile", dependencies=[],
                         selector={"target": "aarch64-linux-ohos"}, generation_id="sha256:" + "d" * 64,
                         arguments=["../inputs/" + fixtures.WORKER, "--config", "../inputs/backend.json"],
                         inputs=[fixtures.reference(path, path.relative_to(self.inputs).as_posix())
                                 for path in sorted(self.inputs.rglob("*")) if path.is_file()])
        self.task_path.write_text(json.dumps(self.task))
        self.pin = support.sha(self.task_path)

    def validate(self):
        return fixtures.validate_source(self.task_path, self.pin, self.inputs)

    def test_three_tuple_components_and_two_derived_strings_only(self):
        before = (self.inputs / fixtures.HEADER).read_bytes()
        original = fixtures.constants(before)
        for case, (component, _, _, _, _) in fixtures.CASES.items():
            with self.subTest(case=case):
                after, edits = fixtures.derive_header(before, case)
                changed = fixtures.constants(after)
                self.assertEqual([item["constant"] for item in edits],
                                 [component, "GatewayIdentity", "GatewayRouteIdentity"])
                for key in original.keys() - {component, "GatewayIdentity", "GatewayRouteIdentity"}:
                    self.assertEqual(original[key], changed[key])
                self.assertNotEqual(original[component], changed[component])
                self.assertEqual(sum(a != b for a, b in zip(before.splitlines(), after.splitlines())), 3)

    def test_prepare_is_non_candidate_and_preserves_every_other_byte(self):
        out = self.root / "fixtures"
        receipts = fixtures.build_fixtures(self.task_path, self.pin, self.inputs, out, prepare_only=True)
        self.assertEqual(len(receipts), 3)
        support.verify_refs(self.inputs, self.task["inputs"])
        for case, (_, _, _, code, counter) in fixtures.CASES.items():
            receipt = json.loads((out / case / "fixture-receipt.json").read_text())
            self.assertFalse(receipt["candidate"])
            self.assertFalse(receipt["target_executed"])
            self.assertEqual(receipt["build_status"], "not-run")
            self.assertEqual(receipt["changed_files"], [fixtures.HEADER])
            self.assertEqual(receipt["expected_error"], code)
            self.assertEqual(receipt["expected_bootstrap_delta"][counter], 1)
            self.assertEqual(sum(receipt["expected_bootstrap_delta"]), 2)
            frozen = out / case / "compile/inputs"
            for ref in self.task["inputs"]:
                if ref["path"] != fixtures.HEADER:
                    self.assertEqual((frozen / ref["path"]).read_bytes(), (self.inputs / ref["path"]).read_bytes())
        with self.assertRaisesRegex(ValueError, "fresh"):
            fixtures.build_fixtures(self.task_path, self.pin, self.inputs, out, prepare_only=True)

    def test_parent_pin_and_input_drift_rejected(self):
        with self.assertRaisesRegex(ValueError, "caller pin"):
            fixtures.validate_source(self.task_path, "sha256:" + "0" * 64, self.inputs)
        self.write(fixtures.FACADE, "changed")
        with self.assertRaisesRegex(ValueError, "frozen input changed"):
            self.validate()

    def test_ordinary_route_and_old_companion_rejected(self):
        self.config["variant"] = "ordinary"
        self.write("backend.json", json.dumps(self.config))
        self.refresh_task()
        with self.assertRaisesRegex(ValueError, "test companion"):
            self.validate()
        self.config["variant"] = "test"
        self.write("backend.json", json.dumps(self.config))
        self.write("declarations.json", json.dumps({"gateway": {"methods": [{"name": "runtimeOpen"}]}}))
        self.refresh_task()
        with self.assertRaisesRegex(ValueError, "31-method"):
            self.validate()

    def test_changed_error_or_counter_is_not_silently_reinterpreted(self):
        path = self.inputs / "derived/native/src/runtime.cpp"
        path.write_text(path.read_text().replace('return error(env, "AbiMismatch");',
                                                 'return error(env, "InvalidArgument");'))
        self.refresh_task()
        with self.assertRaisesRegex(ValueError, "rejection code/event changed"):
            self.validate()

    def test_facade_mismatch_and_symlink_are_rejected(self):
        self.write(fixtures.FACADE, "const EXPECTED_IDENTITY = 'wrong';")
        self.refresh_task()
        with self.assertRaisesRegex(ValueError, "unchanged facade expectation disagree"):
            self.validate()
        path = self.inputs / fixtures.FACADE
        path.unlink()
        path.symlink_to(self.task_path)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
