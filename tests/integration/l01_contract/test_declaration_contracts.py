"""L01 boundary proof using recorded compiler facts, not a compiler/SDK rerun."""

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]


def load(name):
    path = ROOT / "adapters/generators/kotlin_rust" / (name + ".py")
    spec = importlib.util.spec_from_file_location("l01_" + name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


LOWER, SWIFT = load("lowering"), load("skie_lowering")


class DeclarationContract(unittest.TestCase):
    def setUp(self):
        fixtures = ROOT / "tests/integration/c06_reader/fixtures"
        self.rust = json.loads((fixtures / "uniffi-api.json").read_text())
        self.swift = json.loads((fixtures / "skie-api.json").read_text())

    def seed(self, descriptor, variant="swift-rust"):
        return LOWER.lower(descriptor, {"selector": {}}, {
            "variant": variant, "features": [], "source": "recorded-public-api",
        })

    def test_existing_swift_inputs_still_resolve_their_own_policy(self):
        self.assertTrue(LOWER.policies(self.seed(self.rust)))
        descriptor = SWIFT.descriptor(self.swift, "recorded-public.h")
        seed = self.seed(descriptor, "swift-kotlin")
        self.assertTrue(LOWER.policies(seed))
        definitions = {item["id"]: item["definition"] for item in seed["types"]}
        self.assertEqual(definitions["Swift.Error"], {
            "kind": "resource", "representation": "backend-handle", "owner": "swift.error",
        })

    def test_structural_business_edits_do_not_inherit_sample_engine_semantics(self):
        for edit in ("add-method", "add-field", "rename", "delete-method"):
            with self.subTest(edit=edit):
                value = deepcopy(self.rust)
                method = next(f for f in value["functions"] if f["name"] == "roundtrip")
                if edit == "add-method":
                    new = deepcopy(method)
                    new["name"] = "read_summary"
                    new["ffi_name"] = "independent_read_summary"
                    value["functions"].append(new)
                elif edit == "add-field":
                    record = next(t for t in value["types"] if t["kind"] == "record")
                    record["fields"].append({
                        "name": "revision", "type": {"kind": "u32"}, "has_default": False,
                    })
                elif edit == "rename":
                    method["name"] = "normalize"
                else:
                    value["functions"].remove(method)
                seed = self.seed(value)
                self.assertTrue(seed["symbols"], "parsed structure is distinct from safe acceptance")
                with self.assertRaisesRegex(LOWER.UnsupportedDeclaration, "reviewed C06 semantic surface"):
                    LOWER.policies(seed)

    def test_renamed_source_does_not_reuse_old_source_identity(self):
        before = self.seed(self.rust)
        value = deepcopy(self.rust)
        next(f for f in value["functions"] if f["name"] == "roundtrip")["name"] = "normalize"
        after = self.seed(value)
        old = next(s for s in before["symbols"] if s["source_key"]["qualified_name"].endswith(".roundtrip"))
        new = next(s for s in after["symbols"] if s["source_key"]["qualified_name"].endswith(".normalize"))
        self.assertNotEqual(old["source_id"], new["source_id"])
        self.assertNotEqual(old["public_id"], new["public_id"])

    def test_async_callback_cannot_acquire_synchronous_retention_policy(self):
        callback = next(f for f in self.rust["functions"] if f["kind"] == "callback_method")
        callback["async"] = True
        with self.assertRaisesRegex(LOWER.UnsupportedDeclaration, "one synchronous method"):
            self.seed(self.rust)

    def test_borrow_and_default_are_not_implicit_value_contracts(self):
        for name in ("by_ref", "has_default"):
            with self.subTest(name=name):
                value = deepcopy(self.rust)
                parameter = next(f for f in value["functions"] if f["parameters"])["parameters"][0]
                parameter[name] = True
                with self.assertRaisesRegex(LOWER.UnsupportedDeclaration, "borrowed UDL|default argument"):
                    self.seed(value)


if __name__ == "__main__":
    unittest.main()
