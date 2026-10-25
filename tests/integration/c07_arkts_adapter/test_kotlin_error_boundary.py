"""Execute generated Kotlin helper error boundaries with a native-port double."""
from pathlib import Path
import json
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts.kotlin_binding import public_source
from adapters.toolchains.arkts import kotlin_frame


class KotlinHelperErrorBoundary(unittest.TestCase):
    def test_native_errors_and_user_error_identity(self):
        source = public_source(ROOT, {"kotlin": {"test_support": True}}, "host-regression")
        with tempfile.TemporaryDirectory() as temporary:
            generated = Path(temporary) / "Boundary.ets"
            generated.write_bytes(source)
            frame = Path(temporary) / "RuntimeFrame.ets"
            frame.write_bytes(kotlin_frame.render()["generated/facade/RuntimeFrame.ets"])
            result = subprocess.run(
                ["node", str(ROOT / "tests/integration/c07_arkts_facade/kotlin_errors.cjs"), str(generated), str(frame)],
                text=True, capture_output=True, timeout=30, check=False,
            )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        observation = json.loads(result.stdout)
        self.assertEqual(observation["native_helpers"], 24)
        self.assertEqual(observation["saved_calls"], 4)
        for key in ("getter_identity", "brand_before_getters", "getters_once", "reentrant_close", "settler_identity", "container_validation", "field_validation"):
            self.assertTrue(observation[key], key)


if __name__ == "__main__":
    unittest.main()
