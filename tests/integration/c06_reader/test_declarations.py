"""Adversarial declaration/policy and output closure tests, separate from native Lab evidence."""
from copy import deepcopy
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
DIRECTORY = ROOT / "adapters/generators/kotlin_rust"


def module(name):
    spec = importlib.util.spec_from_file_location("c06_test_" + name, DIRECTORY / (name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


LOWER, SWIFT, DRIVER = module("lowering"), module("skie_lowering"), module("driver")


class Declarations(unittest.TestCase):
    def setUp(self):
        self.rust = json.loads((Path(__file__).parent / "fixtures/uniffi-api.json").read_text())
        self.swift = json.loads((Path(__file__).parent / "fixtures/skie-api.json").read_text())

    def seed(self, descriptor, variant="kotlin-jvm-rust"):
        return LOWER.lower(descriptor, {"selector": {}}, {"variant": variant, "features": [], "source": "public-api"})

    def test_real_parser_snapshots_match_only_the_authored_surface(self):
        self.assertTrue(LOWER.policies(self.seed(self.rust)))
        swift = self.seed(SWIFT.descriptor(self.swift, "public.h"), "swift-kotlin")
        self.assertTrue(LOWER.policies(swift))
        errors = next(t["definition"] for t in swift["types"] if t["id"] == "Swift.Error")
        self.assertEqual(errors, {"kind": "resource", "representation": "backend-handle", "owner": "swift.error"})

    def test_new_resource_never_inherits_safe_close_and_thread_claims(self):
        self.rust["types"].append({"kind": "object", "name": "Other", "implementation": "struct"})
        extra = deepcopy(self.rust["functions"][0])
        extra.update(owner="Other", name="blocking_call", parameters=[], result=None, kind="method", **{"async": False})
        self.rust["functions"].append(extra)
        with self.assertRaisesRegex(ValueError, "reviewed C06 semantic surface"):
            LOWER.policies(self.seed(self.rust))

    def test_changed_error_and_argument_types_need_a_semantic_review(self):
        for mutation in ("throws", "parameters"):
            changed = deepcopy(self.rust)
            method = next(f for f in changed["functions"] if f["name"] == "roundtrip")
            if mutation == "throws":
                method["throws"] = None
            else:
                method["parameters"][0]["type"] = {"kind": "i64"}
            with self.assertRaisesRegex(ValueError, "reviewed C06 semantic surface"):
                LOWER.policies(self.seed(changed))

    def test_defaults_borrows_and_trait_interfaces_fail_closed(self):
        for field, value in (("has_default", True), ("by_ref", True)):
            changed = deepcopy(self.rust)
            changed["functions"][0]["parameters"][0][field] = value
            with self.assertRaises(ValueError):
                self.seed(changed)
        changed = deepcopy(self.rust)
        next(t for t in changed["types"] if t["kind"] == "object")["implementation"] = "rust-trait"
        with self.assertRaisesRegex(ValueError, "struct objects"):
            self.seed(changed)
        changed = deepcopy(self.rust)
        next(t for t in changed["types"] if t["kind"] == "record")["fields"][0]["has_default"] = True
        with self.assertRaisesRegex(ValueError, "default value"):
            self.seed(changed)

    def test_wrong_parser_version_is_not_a_compatible_descriptor(self):
        self.rust["parser"] = "uniffi-0.29.4"
        with self.assertRaisesRegex(ValueError, "parser version"):
            self.seed(self.rust)

    def test_callback_thread_protocol_is_not_assigned_to_async_callbacks(self):
        method = next(f for f in self.rust["functions"] if f["kind"] == "callback_method")
        method["async"] = True
        with self.assertRaisesRegex(ValueError, "synchronous method"):
            self.seed(self.rust)

    def test_nested_class_without_actual_sealed_inheritance_is_rejected(self):
        for item in self.swift["objc_declarations"]:
            if item["name"] == "MBSBoundaryEventData":
                item["super"] = "NSObject"
        with self.assertRaisesRegex(ValueError, "sealed subclass"):
            SWIFT.descriptor(self.swift, "public.h")

    def test_erased_swift_throws_cannot_silently_disappear(self):
        symbol = next(s for s in self.swift["swift_symbols"] if s["pathComponents"] == ["Engine", "awaitClosed()"])
        symbol["declarationFragments"] = [f for f in symbol["declarationFragments"] if f["spelling"] != "throws"]
        with self.assertRaisesRegex(ValueError, "reviewed C06 semantic surface"):
            LOWER.policies(self.seed(SWIFT.descriptor(self.swift, "public.h"), "swift-kotlin"))

    def test_array_cannot_be_lowered_as_its_scalar_element(self):
        reader = SWIFT.Reader(self.swift)
        fragments = [
            {"kind": "text", "spelling": "["},
            {"kind": "typeIdentifier", "spelling": "Int64", "preciseIdentifier": "s:s5Int64V"},
            {"kind": "text", "spelling": "]"},
        ]
        with self.assertRaisesRegex(ValueError, "type syntax"):
            reader.type(fragments)

    def test_function_ownership_modifier_cannot_hide_in_signature_projection(self):
        symbol = next(s for s in self.swift["swift_symbols"] if s["pathComponents"] == ["Engine", "roundtrip(value:)"])
        symbol["declarationFragments"].append({"kind": "keyword", "spelling": "inout"})
        with self.assertRaisesRegex(ValueError, "ownership/isolation modifier"):
            SWIFT.descriptor(self.swift, "public.h")

    def test_actor_attributes_and_default_arguments_are_not_erased(self):
        for kind, spelling in (("attribute", "@MainActor"), ("text", " = 42")):
            changed = deepcopy(self.swift)
            symbol = next(s for s in changed["swift_symbols"] if s["pathComponents"] == ["Engine", "roundtrip(value:)"])
            symbol["declarationFragments"].append({"kind": kind, "spelling": spelling})
            with self.assertRaises(ValueError):
                SWIFT.descriptor(changed, "public.h")
        changed = deepcopy(self.swift)
        symbol = next(s for s in changed["swift_symbols"] if s["pathComponents"] == ["Engine"])
        symbol["declarationFragments"].append({"kind": "attribute", "spelling": "@MainActor"})
        with self.assertRaisesRegex(ValueError, "attribute"):
            SWIFT.descriptor(changed, "public.h")


class ClosedOutputs(unittest.TestCase):
    def test_cli_only_keeps_response_and_declared_outputs(self):
        for operation in ("metadata", "generate"):
            with self.subTest(operation=operation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                work = root / "work"
                work.mkdir()
                config = root / "backend.json"
                config.write_text('{}')
                def helper(request, config, inputs, scratch):
                    (scratch / "home/cache").mkdir(parents=True)
                    (scratch / "read-skie-api.json").write_text('temporary compiler data')
                    if operation == "metadata":
                        return {"schema": "test"}
                    (scratch / "generated").mkdir()
                    (scratch / "generated/code.kt").write_text('published')
                    return {"files": [{"path": "generated/code.kt"}]}
                old = Path.cwd()
                try:
                    os.chdir(work)
                    with patch.object(DRIVER, operation, helper), patch.object(DRIVER.sys, "argv", ["driver", operation, "--config", str(config)]), patch.object(DRIVER.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(b'{}'))), patch.object(DRIVER.sys, "stdout", SimpleNamespace(buffer=io.BytesIO())):
                        DRIVER.main()
                finally:
                    os.chdir(old)
                paths = {p.relative_to(work).as_posix() for p in work.rglob('*') if p.is_file()}
                self.assertEqual(paths, {"contract.json"} if operation == "metadata" else {"outputs.json", "generated/code.kt"})
                self.assertFalse(any(p.name.startswith('private-driver-') for p in work.iterdir()))

    def test_generated_symlinks_and_unknown_artifacts_are_rejected_before_copy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root / 'secret'
            outside.write_text('not an output')
            output = root / 'out'
            (output / 'generated/uniffi').mkdir(parents=True)
            link = output / 'generated/uniffi/code.kt'
            link.symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "link or special"):
                DRIVER.generated_files(output, 'kotlin-jvm-rust')
            link.unlink()
            (output / 'arbitrary.sh').write_text('false')
            with self.assertRaisesRegex(ValueError, "unexpected generated"):
                DRIVER.generated_files(output, 'kotlin-jvm-rust')

    def test_output_root_link_cannot_launder_external_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'outside').mkdir()
            (root / 'outside/identity.json').write_text('{}')
            (root / 'output').symlink_to(root / 'outside', target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "ordinary directory"):
                DRIVER.generated_files(root / 'output', 'swift-rust')

    def test_copy_refuses_linked_parent_and_existing_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'source'
            source.write_text('data')
            outside = root / 'outside'
            outside.mkdir()
            output = root / 'out'
            output.mkdir()
            (output / 'alias').symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "ordinary directory"):
                DRIVER.copy_new(source, output, 'alias/file')
            self.assertFalse((outside / 'file').exists())
            DRIVER.copy_new(source, output, 'ordinary')
            with self.assertRaises(FileExistsError):
                DRIVER.copy_new(source, output, 'ordinary')

    def test_input_links_are_rejected_even_when_target_stays_inside_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'ordinary').mkdir()
            (root / 'ordinary/file').write_text('input')
            (root / 'alias').symlink_to(root / 'ordinary', target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                DRIVER.input_path(root, 'alias/file')


if __name__ == '__main__':
    unittest.main()
