"""Actual layout authorities, without SDK execution or target claims."""

from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import diagnostics, lowering, support


class DiagnosticProtocols(unittest.TestCase):
    def read(self, provider):
        spec = (
            diagnostics.KOTLIN_LAYOUTS if provider == "kotlin" else diagnostics.C_LAYOUTS
        )
        names = [name for name, _ in spec.values()]
        if provider == "kotlin":
            names.append("adapters/toolchains/arkts/kotlin_frame.py")
        refs = {
            name: {
                "path": name,
                "sha256": support.sha(ROOT / name),
                "size": (ROOT / name).stat().st_size,
            }
            for name in names
        }
        return diagnostics.read(
            ROOT, {"provider": provider, "variant": "test"}, refs, support
        )

    def test_actual_layouts_close_both_test_routes(self):
        native = self.read("c")
        self.assertEqual(len(native["protocols"]["native_frame"]), 54)
        self.assertEqual(native["protocols"]["test_frame"]["total_length"], 178)
        self.assertEqual(native["protocols"]["memory"]["length"], 27)
        managed = self.read("kotlin")
        self.assertEqual(managed["protocols"]["runtime"]["length"], 110)
        self.assertEqual(
            [r["count"] for r in managed["protocols"]["napi_observer"]["roles"]],
            [30, 101],
        )
        policy = support.read_json(
            ROOT / "adapters/generators/arkts/profiles/snapshot-kotlin-test-v1.json"
        )
        diagnostics.verify_bounds(managed, policy)
        policy["value_slots"]["testSubscriptionState.return"]["length_elements"] = 5
        with self.assertRaisesRegex(ValueError, "bound differs"):
            diagnostics.verify_bounds(managed, policy)

    def test_same_length_reorder_and_unit_change_affect_protocol(self):
        name, selectors = diagnostics.KOTLIN_LAYOUTS["controls"]
        document = support.read_json(ROOT / name)
        original = diagnostics.protocol(document, selectors)
        changed = deepcopy(document)
        changed["frame"]["fields"][1], changed["frame"]["fields"][2] = (
            changed["frame"]["fields"][2],
            changed["frame"]["fields"][1],
        )
        self.assertNotEqual(original, diagnostics.protocol(changed, selectors))
        changed = deepcopy(document)
        changed["frame"]["fields"][14]["unit"] = "UTF-16 code units"
        self.assertNotEqual(original, diagnostics.protocol(changed, selectors))

    def test_description_path_and_hash_do_not_affect_protocol(self):
        name, selectors = diagnostics.KOTLIN_LAYOUTS["controls"]
        document = support.read_json(ROOT / name)
        original = diagnostics.protocol(document, selectors)
        changed = deepcopy(document)
        changed.update(scope="edited commentary", source={"path": "new", "sha256": "new"})
        changed["frame"]["fields"][14]["description"] = "wording only"
        self.assertEqual(original, diagnostics.protocol(changed, selectors))

    def test_thread_protocol_binds_slots_units_and_encoded_bound(self):
        name, selectors = diagnostics.KOTLIN_LAYOUTS["thread"]
        document = support.read_json(ROOT / name)
        original = diagnostics.protocol(document, selectors)
        self.assertEqual(
            (original["format"], original["length"], original["max_bytes"]),
            (70604, 21, 440),
        )
        self.assertEqual(
            [field["index"] for field in original["fields"]], list(range(21))
        )
        for change in ("order", "unit", "bound"):
            changed = deepcopy(document)
            if change == "order":
                changed["fields"][7], changed["fields"][8] = (
                    changed["fields"][8],
                    changed["fields"][7],
                )
            elif change == "unit":
                changed["fields"][7]["unit"] = "flag0or1; no unknown state"
            else:
                changed["max_bytes"] = 441
            self.assertNotEqual(original, diagnostics.protocol(changed, selectors))
        changed = deepcopy(document)
        changed["scope"] = "commentary only"
        changed["source"] = {"path": "another.json", "sha256": "other"}
        changed["fields"][7]["description"] = "wording only"
        self.assertEqual(original, diagnostics.protocol(changed, selectors))
        policy = support.read_json(
            ROOT / "adapters/generators/arkts/profiles/snapshot-kotlin-test-v1.json"
        )
        policy["test_types"]["test.thread-csv"]["max_bytes"] = 441
        with self.assertRaisesRegex(ValueError, "string bound differs"):
            diagnostics.verify_bounds(self.read("kotlin"), policy)

    def test_missing_original_layout_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "omitted"):
            diagnostics.read(ROOT, {"provider": "c", "variant": "test"}, {}, support)
        self.assertIsNone(
            diagnostics.read(ROOT, {"provider": "c", "variant": "ordinary"}, {}, support)
        )

    def test_abi_binds_protocol_but_not_source_or_layout_path(self):
        contract = {"types": [], "symbols": [], "policies": [], "required_features": []}
        descriptor = {"diagnostics": self.read("kotlin")}
        policy = {
            "value_slots": {
                "observe.return": {"layout": "old.json", "length_elements": 110}
            },
            "failed_completion_handoff": {
                "allowed_integers": [0, 1],
                "deliver_pending": 0,
                "abandon_failed_pending": 1,
                "abandon_is_monotonic": True,
            },
        }

        def digest(d, p):
            return hashlib.sha256(
                json.dumps(
                    lowering.abi_projection(d, contract, p), sort_keys=True
                ).encode()
            ).hexdigest()

        before = digest(descriptor, policy)
        changed, changed_policy = deepcopy(descriptor), deepcopy(policy)
        changed["diagnostics"]["sources"] = {
            "new-source": {"path": "new", "sha256": "new"}
        }
        changed_policy["value_slots"]["observe.return"]["layout"] = "renamed.json"
        self.assertEqual(before, digest(changed, changed_policy))
        changed["diagnostics"]["protocols"]["runtime"]["fields"][1][
            "name"
        ] = "DifferentMeaning"
        self.assertNotEqual(before, digest(changed, changed_policy))


class KotlinTestPolicy(unittest.TestCase):
    def setUp(self):
        self.policy = support.read_json(
            ROOT / "adapters/generators/arkts/profiles/snapshot-kotlin-test-v1.json"
        )

    def lower(self):
        public = deepcopy(self.policy["public_surface"])
        for call in public["functions"] + [
            m for r in public["resources"] for m in r["methods"]
        ]:
            call["location"] = {"path": "shape-fixture.ets", "line": 1, "column": 1}
        return lowering.lower_public(
            public,
            self.policy,
            {"selector": {}},
            {
                "features": lowering.FEATURES,
                "boundary_id": "kotlin-arkts",
                "source_package": "madbc07sdkkotlintest",
                "variant": "test",
                "policy": "runtime-policy.json",
            },
        )

    def test_complete_explicit_managed_test_shape(self):
        self.assertNotIn("gateway_surface", self.policy)
        self.assertNotIn("c_seam_surface", self.policy)
        self.assertEqual(len(self.policy["public_surface"]["functions"]), 30)
        self.assertEqual(len(self.lower()[0]["symbols"]), 49)
        self.assertEqual(
            self.policy["calls"]["testSettleProbe"]["slots"]["parameter.action"][
                "context_owner"
            ],
            "caller",
        )
        self.assertEqual(
            self.policy["value_slots"]["testArmCloneFailure.nth"]["allowed_integers"],
            list(range(1, 9)),
        )

    def test_thread_helper_is_not_an_ordinary_or_ksp_export(self):
        action = self.policy["value_slots"]["testThreadWitness.action"]
        self.assertEqual(action["allowed_integers"], [0, 1, 2])
        self.assertEqual(action["closing_actions"], [0, 2])
        self.assertEqual(action["closed_actions"], [0])
        frontend = support.read_json(ROOT / "sdk/kotlin/arkts-owned/frontend-policy.json")
        self.assertEqual(len(frontend["functions"]), 15)
        self.assertEqual(len(frontend["development_test_functions"]), 8)
        self.assertNotIn(
            "testThreadWitness",
            frontend["functions"] + frontend["development_test_functions"],
        )
        ordinary = ROOT / "adapters/generators/arkts/profiles/snapshot-kotlin-v1.json"
        fixtures = sorted(
            (ROOT / "tests/fixtures/c07_kotlin_generation").glob("*/runtime-policy.json")
        )
        self.assertEqual(len(fixtures), 4)
        for path in [ordinary, *fixtures]:
            policy = support.read_json(path)
            names = [
                function["name"] for function in policy["public_surface"]["functions"]
            ]
            self.assertNotIn("testThreadWitness", names, str(path))
            self.assertNotIn("testThreadWitness", policy["calls"], str(path))

    def test_boolean_and_array_slots_require_their_authored_roles(self):
        for name in (
            "testFinishPublication.commit",
            "testMemoryFrame.return",
            "testSubscriptionState.return",
            "testThreadWitness.action",
            "testThreadWitness.return",
        ):
            with self.subTest(name=name):
                original = self.policy["value_slots"].pop(name)
                with self.assertRaisesRegex(
                    ValueError, "unsupported public value mapping"
                ):
                    self.lower()
                self.policy["value_slots"][name] = original


class KotlinSourceClosure(unittest.TestCase):
    def test_ordinary_excludes_test_runtime_and_observer_sources(self):
        from adapters.toolchains.arkts.kotlin import source_inputs

        ordinary = {entry["path"] for entry in source_inputs(ROOT, "ordinary")}
        test = {entry["path"] for entry in source_inputs(ROOT, "test")}
        for segment in ("/src/test/", "/test-runtime/", "/test-observer/", "/contracts/"):
            self.assertFalse(any(segment in name for name in ordinary))
            self.assertTrue(any(segment in name for name in test))
        self.assertTrue(any("/src/ordinary/" in name for name in ordinary))
        self.assertFalse(any("/src/ordinary/" in name for name in test))
        with self.assertRaises(ValueError):
            source_inputs(ROOT, "invented")

    def test_managed_patch_and_test_instrumentation_have_exact_original_pins(self):
        base = ROOT / "adapters/generators/akinterop"
        pin = support.read_json(base / "managed-owned-r6-source.json")
        self.assertEqual(
            support.sha(base / "patches/managed-owned-r6.patch"),
            "sha256:" + pin["patch_sha256"],
        )
        runtime = ROOT / "sdk/kotlin/arkts-owned/test-runtime"
        test = support.read_json(runtime / "source-pin.json")
        self.assertEqual(
            support.sha(runtime / "codec-memory.patch"), "sha256:" + test["patch_sha256"]
        )
        self.assertEqual(
            support.sha(runtime / "ManagedRecordMemoryProbe.kt"),
            "sha256:" + test["added_sha256"],
        )
        self.assertNotEqual(test["base_sha256"], test["result_sha256"])


if __name__ == "__main__":
    unittest.main()
