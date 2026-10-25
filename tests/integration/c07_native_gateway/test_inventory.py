import unittest
from pathlib import Path
import tempfile
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "sdk/arkts/test-support/native"))
from observer_inventory import check_preprocessed, code_only, sites

class InventoryControls(unittest.TestCase):
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
