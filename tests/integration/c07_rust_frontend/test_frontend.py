"""Finite seam rejection tests; real compiler checks belong to C04 evidence."""

from copy import deepcopy
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import rust_frontend
from adapters.toolchains.arkts import rust_metadata
from adapters.toolchains.arkts.native import compose_native


class RawSeam(unittest.TestCase):
    def test_ordinary_and_companion_share_exact_marker_extraction(self):
        for variant in ("ordinary", "test"):
            with self.subTest(
                variant=variant
            ), tempfile.TemporaryDirectory() as temporary:
                native = compose_native(
                    ROOT, Path(temporary) / "native", route="rust", variant=variant
                )
                original = next(
                    u.path for u in native.units if u.path.name == "api.cpp"
                ).read_text()
                generated = rust_frontend.raw_api(
                    original, test_support=variant == "test"
                )
                self.assertIn(
                    "if (argc != (count) || ((count) > 0 && !argv))", generated
                )
                self.assertNotIn("napi_module_register", generated)
                self.assertIn("GatewayProviderCreate", generated)
                body = original.split(rust_frontend.CALLBACK_END, 1)[1].split(
                    rust_frontend.REGISTRAR_BEGIN, 1
                )[0]
                self.assertIn(
                    body, generated
                )  # All original lifecycle statements stay byte-exact.

    def test_missing_duplicate_and_reversed_markers_reject(self):
        for text in (
            "",
            rust_frontend.CALLBACK_BEGIN * 2 + rust_frontend.CALLBACK_END,
            rust_frontend.CALLBACK_END + rust_frontend.CALLBACK_BEGIN,
        ):
            with self.assertRaises(ValueError):
                rust_frontend.raw_api(text)

    def test_raw_dispatch_permits_zero_without_dereferencing_argv(self):
        descriptor = {
            "methods": [{"name": "f" + str(i), "params": []} for i in range(16)]
        }
        source = rust_frontend.render_bridge(descriptor)["runtime_ffi.cpp"].decode()
        self.assertIn("argc > 4 || (argc > 0 && !argv)", source)
        self.assertIn("for (size_t i = 0; i < argc; ++i)", source)
        self.assertNotIn("argv[0]", source)
        self.assertIn('default: return gateway::error(env, "InvalidArgument")', source)

    def test_export_order_is_part_of_private_dispatch_identity(self):
        gateway = {"methods": [{"name": "f" + str(i), "params": []} for i in range(16)]}
        raw = {
            "exports": [
                {"name": m["name"], "arity": 0, "rawReturn": "unknown"}
                for m in gateway["methods"]
            ]
        }
        rust_frontend.compare_exports(raw, gateway)
        wrong = deepcopy(raw)
        wrong["exports"][0], wrong["exports"][1] = (
            wrong["exports"][1],
            wrong["exports"][0],
        )
        with self.assertRaises(ValueError):
            rust_frontend.compare_exports(wrong, gateway)
        wrong = deepcopy(raw)
        wrong["exports"][0]["rawReturn"] = "Snapshot"
        with self.assertRaises(ValueError):
            rust_frontend.compare_exports(wrong, gateway)

    def test_companion_entry_authority_is_preserved_before_dispatch_errors(self):
        with tempfile.TemporaryDirectory() as temporary:
            native = compose_native(
                ROOT, Path(temporary) / "native", route="rust", variant="test"
            )
            original = next(
                u.path for u in native.units if u.path.name == "api.cpp"
            ).read_text()
            generated = rust_frontend.raw_api(original, test_support=True)
            self.assertEqual(generated.count("MADB_NATIVE_TEST_ENTRY(env);"), 1)
            with self.assertRaises(ValueError):
                rust_frontend.raw_api(original)
            with self.assertRaises(ValueError):
                rust_frontend.raw_api(
                    original.replace("MADB_NATIVE_TEST_ENTRY(env);", ""),
                    test_support=True,
                )
        descriptor = {
            "methods": [{"name": "f" + str(i), "params": []} for i in range(31)]
        }
        bridge = rust_frontend.render_bridge(descriptor, test_support=True)[
            "runtime_ffi.cpp"
        ].decode()
        self.assertLess(
            bridge.index("napi_observer::Scope"), bridge.index("if (argc > 4")
        )
        with self.assertRaises(ValueError):
            rust_frontend.render_bridge(descriptor)

    def test_frontend_variant_requires_explicit_boolean(self):
        self.assertEqual(
            rust_metadata.selected_features({"rust": {"test_support": False}}), []
        )
        self.assertEqual(
            rust_metadata.selected_features({"rust": {"test_support": True}}),
            ["--features", "test-support"],
        )
        for value in ("false", 0, None):
            with self.assertRaises(ValueError):
                rust_metadata.selected_features({"rust": {"test_support": value}})


if __name__ == "__main__":
    unittest.main()
