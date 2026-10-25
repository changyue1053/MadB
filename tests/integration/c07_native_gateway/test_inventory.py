import unittest
from pathlib import Path
import tempfile
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "sdk/arkts/test-support/native"))
from observer_inventory import check_preprocessed, code_only, sites, source_files, compiled_coverage

class InventoryControls(unittest.TestCase):
    def test_extra_generated_source_is_hashed_inventory_input_and_deduplicated(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src").mkdir()
            baseline = root / "src/api.cpp"
            baseline.write_text("napi_a(env);")
            extra = root / "raw_api.cpp"
            extra.write_text("napi_a(env);")
            paths, explicit = source_files(root, [extra, extra])
            self.assertEqual(paths, sorted([baseline.resolve(), extra.resolve()]))
            self.assertEqual(explicit, {extra.resolve()})
            self.assertEqual(len(sites(extra, extra.read_text(), {"napi_a"}, {"napi_a"})), 1)

    def test_invalid_extra_source_is_not_silently_omitted(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            invalid = root / "raw.json"
            invalid.write_text("{}")
            for path in (root / "missing.cpp", invalid, root):
                with self.assertRaisesRegex(ValueError, "invalid explicit"):
                    source_files(root, [path])

    def test_actual_preprocessed_set_excludes_uncompiled_baseline(self):
        files = [{"path": "/base.cpp", "calls": [1, 2]},
                 {"path": "/raw.cpp", "calls": [3]}, {"path": "/bridge.cpp", "calls": []}]
        result = compiled_coverage(files, [{"gateway_files": ["/raw.cpp", "/bridge.cpp"]}],
                                   {Path("/raw.cpp"), Path("/bridge.cpp")})
        self.assertEqual(result["preprocessed_source_call_count"], 1)
        self.assertEqual(result["unobserved_source_paths"], ["/base.cpp"])

    def test_extra_source_must_appear_in_actual_preprocessing(self):
        with self.assertRaisesRegex(ValueError, "absent from actual preprocessing"):
            compiled_coverage([], [{"gateway_files": ["/base.cpp"]}], {Path("/raw.cpp")})

    def test_no_preprocessing_is_unknown_not_zero_actual_calls(self):
        result = compiled_coverage([], [], {Path("/raw.cpp")})
        self.assertIsNone(result["preprocessed_source_call_count"])

    def test_actual_call_is_recorded(self):
        self.assertEqual(sites("test", "napi_a(env);", {"napi_a"}, {"napi_a"}),
                         [{"api": "napi_a", "line": 1}])
    def test_new_api_fails(self):
        with self.assertRaisesRegex(ValueError, "uncovered"):
            sites("test", "napi_b(env);", {"napi_a"}, {"napi_a"})
    def test_function_pointer_bypass_fails(self):
        with self.assertRaisesRegex(ValueError, "bare"):
            sites("test", "auto fn = &napi_a;", {"napi_a"}, {"napi_a"})
    def test_disabled_wrapper_fails(self):
        with self.assertRaisesRegex(ValueError, "disable"):
            sites("test", "#undef napi_a\nnapi_a(env);", {"napi_a"}, {"napi_a"})
    def test_literals_do_not_invent_calls(self):
        source = '// napi_b();\nconst char *x="napi_b()";\n/* napi_b(); */\nnapi_a(env);'
        self.assertEqual(sites("test", source, {"napi_a"}, {"napi_a"}),
                         [{"api": "napi_a", "line": 4}])
    def test_raw_literal_requires_review(self):
        with self.assertRaisesRegex(ValueError, "raw"):
            code_only('R"(anything)"')
    def test_preprocessed_wrapper_and_bypass(self):
        with tempfile.TemporaryDirectory() as folder:
            source = str(Path(folder, "source.cpp").resolve())
            preprocessed = Path(folder, "source.i")
            preprocessed.write_text(f'# 1 "{source}"\n::gateway::napi_observer::envCall(::napi_a, env);\n')
            self.assertEqual(check_preprocessed(preprocessed, {source}, {"napi_a"})["observer_expressions"], 1)
            preprocessed.write_text(f'# 1 "{source}"\nnapi_a(env);\n')
            with self.assertRaisesRegex(ValueError, "raw Node-API call"):
                check_preprocessed(preprocessed, {source}, {"napi_a"})

if __name__ == "__main__":
    unittest.main()
