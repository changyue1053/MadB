"""Exact native composition checks; no compiler, launcher or device is invoked."""
from pathlib import Path
import hashlib
import json
import re
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts import native


class NativeCompositionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.work = Path(temporary.name)
        self.repo = self.work / "repo"
        for route in ("c", "cpp", "rust"):
            for source in native.source_paths(ROOT, route=route, variant="test"):
                target = self.repo / source.relative_to(ROOT)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
        self.output = self.work / "composed"

    def compose(self, route="c", variant="test"):
        return native.compose_native(self.repo, self.output, route=route, variant=variant)

    def test_ordinary_has_only_sixteen_methods_and_no_support_code(self):
        result = self.compose(variant="ordinary")
        declaration = result.native_declaration.read_text()
        self.assertEqual(len(re.findall(r"^  \w+\(", declaration, re.M)), 16)
        self.assertNotIn("testArm", declaration)
        self.assertNotIn("testObserve", (self.output / "src/runtime.cpp").read_text())
        self.assertFalse((self.output / "src/support.cpp").exists())
        self.assertEqual(result.facade_template.read_bytes(),
                         (self.repo / "sdk/arkts/facade/Boundary.ets.in").read_bytes())
        self.assertFalse(any("test-support" in str(item.path) for item in result.source_inputs))

    def test_companion_has_exact_frozen_facade_and_thirty_two_methods(self):
        result = self.compose()
        declaration = result.native_declaration.read_text()
        self.assertEqual(len(re.findall(r"^  \w+\(", declaration, re.M)), 32)
        self.assertIn("testArmPartialOpenFailure(): void;", declaration)
        self.assertIn("testLastAdmissionAudit(): number[];", declaration)
        self.assertIn("testBootstrapTotals(): number[];", declaration)
        self.assertTrue((self.output / "src/observer.cpp").is_file())
        baseline = json.loads((self.repo / "sdk/arkts/test-support/facade/baseline.json").read_text())
        self.assertEqual(hashlib.sha256(result.facade_template.read_bytes()).hexdigest(),
                         baseline["derived_sha256"]["Boundary.ets.in"])
        self.assertEqual(len(result.units), 8)
        self.assertTrue((self.output / "src/support.cpp").is_file())

    def test_memory_units_derive_the_actual_core_and_provider(self):
        for route in ("c", "cpp", "rust"):
            result = native.compose_native(self.repo, self.work / route,
                                           route=route, variant="test")
            self.assertEqual(result.units[0].path, self.work / route / "runtime/protocol.cpp")
            self.assertEqual(sum(u.path.name == "protocol.cpp" for u in result.units), 1)
            self.assertTrue(any(u.path.name == "memory.cpp" for u in result.units))
            if route != "rust":
                self.assertEqual(result.units[-1].path,
                                 self.work / route / ("provider/provider." + route))
            self.assertIn("testMemory(runtime: string): number[];",
                          result.native_declaration.read_text())

    def test_memory_original_core_and_provider_drift_rejected(self):
        for source in ("sdk/arkts/native-runtime/src/protocol.cpp",
                       "sdk/c/arkts-provider/src/provider.c"):
            with self.subTest(source=source):
                path = self.repo / source
                old = path.read_bytes()
                path.write_bytes(old + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "baseline changed"):
                    self.compose()
                self.assertFalse(self.output.exists())
                path.write_bytes(old)

    def test_memory_patch_output_hash_is_checked(self):
        path = self.repo / "sdk/arkts/test-support/native/memory-provider-c-baseline.json"
        data = json.loads(path.read_text())
        data["derived_sha256"]["provider/provider.c"] = "0" * 64
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "derived content mismatch"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_unique_core_and_real_provider_sources_all_routes(self):
        for route, language in (("c", "c"), ("cpp", "c++"), ("rust", "rust")):
            with self.subTest(route=route):
                output = self.work / route
                result = native.compose_native(self.repo, output, route=route, variant="ordinary")
                self.assertEqual(result.provider_factory, f"mp_{route}_create")
                self.assertEqual(result.units[0].path,
                                 self.repo / "sdk/arkts/native-runtime/src/protocol.cpp")
                self.assertIn(f"sdk/{route}/arkts-provider/src/", str(result.units[-1].path))
                self.assertEqual(result.units[-1].language, language)
                self.assertEqual(result.registrar, "rust-macro" if route == "rust" else "napi-constructor")
                self.assertEqual((output / "worker/worker.cpp").read_bytes(),
                                 (self.repo / "sdk/arkts/native-runtime/src/worker.cpp").read_bytes())
                self.assertFalse((output / "src/protocol.cpp").exists())

    def test_generated_factory_module_and_raw_bridge_seams_remain_unique(self):
        self.compose()
        text = (self.output / "src/api.cpp").read_text()
        self.assertEqual(text.count("auto status = GatewayProviderCreate("), 1)
        self.assertEqual(text.count("init, GatewayModuleName,"), 1)
        self.assertNotIn("mp_c_create(", text)
        for part in ("CALLBACK", "REGISTRAR"):
            self.assertEqual(text.count(f"// MADB_NATIVE_{part}_BEGIN"), 1)
            self.assertEqual(text.count(f"// MADB_NATIVE_{part}_END"), 1)

    def test_all_inputs_and_outputs_have_exact_byte_identity(self):
        result = self.compose()
        for item in (*result.source_inputs, *result.derived_files):
            content = item.path.read_bytes()
            self.assertEqual(item.size, len(content))
            self.assertEqual(item.sha256, hashlib.sha256(content).hexdigest())
        self.assertEqual({i.path for i in result.source_inputs},
                         set(native.source_paths(self.repo, route="c", variant="test")))
        self.assertEqual({p for p in self.output.rglob("*") if p.is_file()},
                         {i.path for i in result.derived_files})

    def test_changed_gateway_baseline_is_rejected_before_any_output(self):
        path = self.repo / "sdk/arkts/native-gateway/src/api.cpp"
        path.write_bytes(path.read_bytes() + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "baseline changed"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_changed_worker_baseline_is_rejected_before_any_output(self):
        path = self.repo / "sdk/arkts/native-runtime/src/worker.cpp"
        path.write_bytes(path.read_bytes() + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "baseline changed"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_changed_facade_baseline_is_rejected_before_any_output(self):
        path = self.repo / "sdk/arkts/facade/Boundary.ets.in"
        path.write_bytes(path.read_bytes() + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "baseline changed"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_duplicate_baseline_json_key_is_rejected(self):
        path = self.repo / "sdk/arkts/test-support/native/baseline.json"
        path.write_text('{"base_sha256":{},"base_sha256":{},"derived_sha256":{}}')
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_baselines_are_decoded_from_captured_bytes_without_live_text_reads(self):
        with patch.object(Path, "read_text", side_effect=AssertionError("live baseline read")):
            result = self.compose()
        self.assertTrue(result.native_declaration.is_file())

    def test_baseline_keeps_the_existing_utf8_only_metadata_contract(self):
        path = self.repo / "sdk/arkts/test-support/native/baseline.json"
        path.write_bytes(path.read_text().encode("utf-16"))
        with self.assertRaises(UnicodeDecodeError):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_wrong_derived_hash_is_rejected(self):
        path = self.repo / "sdk/arkts/test-support/native/baseline.json"
        baseline = json.loads(path.read_text())
        baseline["derived_sha256"]["src/api.cpp"] = "0" * 64
        path.write_text(json.dumps(baseline))
        with self.assertRaisesRegex(ValueError, "derived content mismatch"):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_missing_input_is_not_silently_omitted(self):
        (self.repo / "sdk/c/arkts-provider/src/provider.c").unlink()
        with self.assertRaises(FileNotFoundError):
            self.compose()
        self.assertFalse(self.output.exists())

    def test_existing_output_is_never_overwritten(self):
        self.output.mkdir()
        sentinel = self.output / "sentinel"
        sentinel.write_text("preserve")
        with self.assertRaisesRegex(ValueError, "already exists"):
            self.compose()
        self.assertEqual(sentinel.read_text(), "preserve")

    def test_dangling_output_symlink_is_not_followed(self):
        destination = self.work / "missing"
        self.output.symlink_to(destination, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "already exists"):
            self.compose()
        self.assertFalse(destination.exists())

    def test_unknown_route_or_variant_is_rejected(self):
        for route, variant in (("js", "ordinary"), ("c", "debug")):
            with self.subTest(route=route, variant=variant):
                with self.assertRaisesRegex(ValueError, "unsupported"):
                    self.compose(route=route, variant=variant)
        self.assertFalse(self.output.exists())

    def test_input_changed_during_derivation_is_rejected_before_output(self):
        real_derive = native._derive
        def change(*args):
            result = real_derive(*args)
            path = self.repo / "sdk/c/arkts-provider/src/provider.c"
            path.write_bytes(path.read_bytes() + b"\n")
            return result
        with patch.object(native, "_derive", side_effect=change):
            with self.assertRaisesRegex(ValueError, "changed during derivation"):
                self.compose()
        self.assertFalse(self.output.exists())


class ExactPatchTests(unittest.TestCase):
    BASE = {"a": "first\nsecond\nthird\n"}
    PATCH = "--- a/a\n+++ b/a\n@@ -1,3 +1,3 @@\n first\n-second\n+changed\n third\n"

    def test_exact_context_patch(self):
        self.assertEqual(native._apply_exact_patch(self.BASE, self.PATCH),
                         {"a": "first\nchanged\nthird\n"})

    def test_wrong_path_and_rename_rejected(self):
        for text in (self.PATCH.replace("a/a", "a/../a").replace("b/a", "b/../a"),
                     self.PATCH.replace("+++ b/a", "+++ b/b")):
            with self.assertRaisesRegex(ValueError, "unknown, renamed"):
                native._apply_exact_patch(self.BASE, text)

    def test_duplicate_file_patch_rejected(self):
        with self.assertRaisesRegex(ValueError, "repeated"):
            native._apply_exact_patch(self.BASE, self.PATCH + self.PATCH)

    def test_no_fuzzy_context_or_line_offset(self):
        for text in (self.PATCH.replace(" first", " wrong"),
                     self.PATCH.replace("@@ -1,3", "@@ -2,3")):
            with self.assertRaises(ValueError):
                native._apply_exact_patch(self.BASE, text)

    def test_wrong_hunk_count_and_binary_metadata_rejected(self):
        for text in (self.PATCH.replace("-1,3", "-1,2"),
                     "diff --git a/a b/a\n" + self.PATCH):
            with self.assertRaises(ValueError):
                native._apply_exact_patch(self.BASE, text)


if __name__ == "__main__":
    unittest.main()
