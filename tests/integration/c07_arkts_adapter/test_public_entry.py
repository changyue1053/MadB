"""Pure public-entry/policy gates; compiler facts are exercised by the public CLI."""

from copy import deepcopy
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import driver, lowering, support
from adapters.toolchains.arkts.requirements import validate, load
from adapters.toolchains.arkts.toolchain import check_distribution


class PrivateJson(unittest.TestCase):
    def test_duplicate_fields_and_non_utf8_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "policy.json"
            for data in (b'{"mode":1,"mode":2}', '{"mode":1}'.encode("utf-16")):
                file.write_bytes(data)
                with self.subTest(data=data), self.assertRaises(ValueError):
                    support.read_json(file)


class PublicRendering(unittest.TestCase):
    def test_render_only_the_two_literal_slots(self):
        data = (ROOT / "sdk/arkts/facade/Boundary.ets.in").read_bytes()
        identity = "madb-arkts/1:" + ":".join(["a" * 64] * 3)
        result = driver.render_template(data, "libtest.so", identity)
        self.assertIn(json.dumps(identity).encode(), result)
        restored = result.replace(json.dumps(identity).encode(), b"@MADB_EXPECTED_IDENTITY@")
        restored = restored.replace(b'"libtest.so"', b"@MADB_NATIVE_MODULE@")
        self.assertEqual(restored, data)
        for bad in (
            data.replace(b"@MADB_NATIVE_MODULE@", b"missing"),
            data + b"@MADB_NATIVE_MODULE@",
        ):
            with self.subTest(bad=bad[-35:]), self.assertRaises(ValueError):
                driver.render_template(bad, "libtest.so", identity)

    def test_actual_rename_fixture_changes_declaration_and_implementation(self):
        folder = ROOT / "tests/fixtures/c07_arkts_generation/rename-record"
        source = (folder / "Boundary.ets.in").read_text()
        self.assertIn("export interface Parcel", source)
        self.assertIn("copyRecord(value: Parcel): Parcel {", source)
        self.assertIn("this.session.copyRecord(this.record, value)", source)
        self.assertIn("native.roundtrip(record.handle, value, text)", source)
        self.assertNotIn("Snapshot", source)
        policy = json.loads((folder / "runtime-policy.json").read_text())
        self.assertIn("Engine.copyRecord", policy["calls"])
        self.assertNotIn("Engine.roundtrip", policy["calls"])
        self.assertEqual(policy["public_surface"]["records"][0]["name"], "Parcel")


class ExplicitPolicy(unittest.TestCase):
    def setUp(self):
        self.policy = json.loads(
            (ROOT / "adapters/generators/arkts/profiles/snapshot-v1.json").read_text()
        )

    def lower(self, package="madbc07sdkcpp"):
        # This is a unit shape fixture, not a substitute for compiler discovery.
        public = deepcopy(self.policy["public_surface"])
        for function in public["functions"] + [
            m for r in public["resources"] for m in r["methods"]
        ]:
            function["location"] = {"path": "unit-shape.ets", "line": 5, "column": 3}
        return lowering.lower(
            public,
            self.policy["gateway_surface"],
            self.policy["c_seam_surface"],
            self.policy,
            {"selector": {}},
            {
                "features": lowering.FEATURES,
                "boundary_id": "cpp-arkts",
                "source_package": package,
                "variant": self.policy.get("variant", "ordinary"),
                "policy": "runtime-policy.json",
            },
        )

    def test_source_package_and_erased_error_are_explicit(self):
        contract, bindings = self.lower()
        self.assertEqual(
            {s["source_key"]["package"] for s in contract["symbols"]}, {"madbc07sdkcpp"}
        )
        types = {t["id"]: t["definition"] for t in contract["types"]}
        self.assertEqual(types["i32"], {"kind": "integer", "width": 32, "signed": True})
        self.assertEqual(types["ArkTSError"]["owner"], "arkts-runtime")
        self.assertEqual(self.policy["limits"]["state_pending"], 1)
        self.assertEqual(self.policy["limits"]["retained_utf8_bytes"], 4096)
        state = [
            b
            for b in bindings
            if b["selector"]["id"] == "madb.c07.Engine.observeState.v1"
            and b["slot"]["kind"] == "call"
        ]
        self.assertEqual(state[0]["values"]["capacity_items"], 3)

    def test_missing_and_duplicate_authored_public_ids_reject(self):
        for mode in ("missing", "duplicate", "invalid"):
            original = deepcopy(self.policy)
            if mode == "missing":
                self.policy["calls"]["openRuntime"].pop("public_id")
            if mode == "duplicate":
                self.policy["calls"]["openRuntime"]["public_id"] = self.policy["calls"][
                    "Runtime.connect"
                ]["public_id"]
            if mode == "invalid":
                self.policy["calls"]["openRuntime"]["public_id"] = "not a valid ID"
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "public_id"):
                self.lower()
            self.policy = original

    def test_author_rename_retains_capability_id_but_changes_source_and_shape(self):
        original, _ = self.lower()
        self.policy = json.loads(
            (
                ROOT / "tests/fixtures/c07_arkts_generation/rename-record/runtime-policy.json"
            ).read_text()
        )
        renamed, _ = self.lower()
        public_id = "madb.c07.Engine.roundtrip.v1"
        before = next(s for s in original["symbols"] if s["public_id"] == public_id)
        after = next(s for s in renamed["symbols"] if s["public_id"] == public_id)
        self.assertEqual(after["public_id_origin"], "explicit")
        self.assertEqual(after["source_key"]["qualified_name"], "Engine.copyRecord")
        self.assertEqual(after["shape"]["link_name"], "Engine.copyRecord")
        self.assertNotEqual(before["source_id"], after["source_id"])
        self.assertNotEqual(before["shape"], after["shape"])

    def test_unknown_policy_shape_does_not_silently_lower(self):
        self.policy["calls"]["unknown"] = self.policy["calls"]["openRuntime"]
        with self.assertRaisesRegex(ValueError, "policy closure"):
            self.lower()

    def test_companion_has_explicit_values_and_runtime_owned_hook(self):
        self.policy = json.loads(
            (ROOT / "adapters/generators/arkts/profiles/snapshot-test-v1.json").read_text()
        )
        contract, bindings = self.lower()
        self.assertEqual(len(contract["symbols"]), 36)
        types = {item["id"]: item["definition"] for item in contract["types"]}
        self.assertEqual(types["test.array.178"]["max_bytes"], 1424)
        self.assertEqual(types["test.array.178"]["length_unit"], "elements")
        self.assertEqual(types["test.array.27"]["max_bytes"], 216)
        self.assertEqual(types["test.array.4"]["max_bytes"], 32)
        state = self.policy["value_slots"]["testSubscriptionState.return"]
        self.assertEqual(state["length_elements"], 4)
        self.assertEqual(json.loads((ROOT / state["layout"]).read_text())["length"], 4)
        memory = self.policy["value_slots"]["testMemory.return"]
        layout = json.loads((ROOT / memory["layout"]).read_text())
        self.assertEqual(memory["length_elements"], layout["length"])
        self.assertEqual(layout["length"], 27)
        self.assertIn("excludes allocator overhead and VM/RSS", memory["element_domain"])
        self.assertEqual(
            self.policy["value_slots"]["testFailClone.mode"]["allowed_integers"], [1, 2, 4]
        )
        hook = next(
            b
            for b in bindings
            if b["selector"]["id"] == "madb.c07.testAfterSettlement.v1"
            and b["slot"] == {"kind": "parameter", "name": "callback"}
        )
        self.assertEqual(hook["values"]["context_owner"], "caller")
        self.assertEqual(hook["values"]["stop"], "request-and-drain")
        self.assertNotEqual(hook["values"]["keep_alive"], "call-frame")
        self.assertIn("Runtime", self.policy["calls"]["testAfterSettlement"]["lifetime"])

    def test_companion_value_slot_is_not_inferred_from_number(self):
        self.policy = json.loads(
            (ROOT / "adapters/generators/arkts/profiles/snapshot-test-v1.json").read_text()
        )
        self.policy["value_slots"].pop("testDeliveryGate.command")
        with self.assertRaisesRegex(ValueError, "unsupported public value mapping"):
            self.lower()


class KotlinPublicRoles(unittest.TestCase):
    def setUp(self):
        self.policy = json.loads(
            (ROOT / "adapters/generators/arkts/profiles/snapshot-kotlin-v1.json").read_text()
        )

    def test_kotlin_roles_do_not_manufacture_c_or_napi_gateway_facts(self):
        self.assertNotIn("c_seam_surface", self.policy)
        self.assertNotIn("gateway_surface", self.policy)
        public = deepcopy(self.policy["public_surface"])
        for function in public["functions"] + [
            m for r in public["resources"] for m in r["methods"]
        ]:
            function["location"] = {"path": "unit-shape.ets", "line": 5, "column": 3}
        config = {
            "features": lowering.FEATURES,
            "boundary_id": "kotlin-arkts",
            "source_package": "madbc07sdkkotlin",
            "variant": "ordinary",
            "policy": "runtime-policy.json",
        }
        contract, _ = lowering.lower_public(public, self.policy, {"selector": {}}, config)
        self.assertEqual(len(contract["symbols"]), len(self.policy["calls"]))
        public["records"][0]["fields"][0]["type"] = {"tag": "string"}
        with self.assertRaisesRegex(ValueError, "public_surface"):
            lowering.lower_public(public, self.policy, {"selector": {}}, config)


class Allocation(unittest.TestCase):
    def values(self):
        ordinary = "c07-c-arkts-aarch64-ohos23"
        route = "madb.arkts-c-napi"
        left = ["ordinary." + str(i) for i in range(5)]
        right = ["companion." + str(i) for i in range(25)]
        profiles = [
            {
                "profile_id": ordinary,
                "route": route,
                "kind": "ordinary",
                "relation_profile": ordinary + "-test",
                "required_cases": left,
            },
            {
                "profile_id": ordinary + "-test",
                "route": route,
                "kind": "test-companion",
                "relation_profile": ordinary,
                "required_cases": right,
            },
        ]
        return {
            "schema": "madb.lab-c07-candidate-case-allocation/1",
            "profiles": profiles,
            "cases": [
                {"id": name, "primary_profile_id": p["profile_id"]}
                for p in profiles
                for name in p["required_cases"]
            ],
        }, {"required_cases_by_route": {route: left + right}}

    def test_entire_related_pair_is_required(self):
        data, required = self.values()
        self.assertEqual(len(validate(data, required)["required_cases"]), 5)
        for mode in ("missing", "owner", "pair", "route", "duplicate"):
            value = deepcopy(data)
            if mode == "missing":
                value["profiles"][0]["required_cases"].pop()
            if mode == "owner":
                value["cases"][0]["primary_profile_id"] = "other"
            if mode == "pair":
                value["profiles"][0]["relation_profile"] = "other"
            if mode == "route":
                value["profiles"][0]["route"] = "other"
            if mode == "duplicate":
                value["profiles"].append(value["profiles"][0])
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                validate(value, required)

    def test_legacy_single_case_fallback_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "legacy.json"
            file.write_text('{"required_cases":["ordinary"]}')
            with self.assertRaisesRegex(ValueError, "schema"):
                load(file, "sha256:" + "0" * 64)


class Distribution(unittest.TestCase):
    def test_exact_component_versions_and_formal_sdk(self):
        value = {
            "command_line_tools": {
                "version": "26.0.0.851",
                "distribution_platform": "mac-arm64",
                "releaseType": "release",
                "hvigor_version": "6.26.8",
                "ohpm_version": "26.0.0.630",
            },
            "sdk": {
                "is_formal": True,
                "product": {
                    "version": "26.0.0.105",
                    "apiVersion": "26",
                    "releaseType": "Release",
                },
            },
        }
        check_distribution(value)
        for field, bad in (
            ("version", "26.0.0.821"),
            ("hvigor_version", "different"),
            ("releaseType", "beta"),
        ):
            changed = deepcopy(value)
            changed["command_line_tools"][field] = bad
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_distribution(changed)
        value["sdk"]["is_formal"] = False
        with self.assertRaises(ValueError):
            check_distribution(value)


if __name__ == "__main__":
    unittest.main()
