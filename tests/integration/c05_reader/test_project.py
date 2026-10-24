"""Preparation identities remain portable while execution locators stay private."""
from pathlib import Path
import json
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from adapters.toolchains.rust_c.project import _public_observation


class PortableObservation(unittest.TestCase):
    def observation(self, prefix):
        checksum = "sha256:" + "a" * 64
        def reference(path):
            return dict(path=prefix + path, sha256=checksum, size=17)
        return dict(schema="madb.native-toolchain/1", target="aarch64-apple-darwin", system="Darwin",
                    tools={"clang": dict(reference("/tools/clang"), id="clang", version="21.0")},
                    clang_arguments=["-target", "arm64-apple-macosx14.0", "-isysroot", prefix + "/SDK",
                        "-resource-dir", prefix + "/resource", "-mmacosx-version-min=14.0", "-std=c11", "-Wall", "-Wextra", "-Werror"],
                    sdk_inputs=[reference("/SDK/usr/include/stdint.h"), reference("/resource/include/limits.h"),
                                reference("/rust/lib/libstd.rlib")],
                    environment=dict(LIBCLANG_PATH=prefix + "/tools"))

    def test_host_relocation_preserves_public_observation(self):
        first = _public_observation(self.observation("/producer-one"))
        second = _public_observation(self.observation("/producer-two"))
        self.assertEqual(first, second)
        encoded = json.dumps(first)
        self.assertNotIn("/producer-", encoded)
        self.assertNotIn("LIBCLANG_PATH", encoded)
        self.assertEqual(first["compiler"]["deployment_target"], "14.0")
        self.assertEqual(first["sdk_inputs"][0]["name"], "clang-resource/include/limits.h")

    def test_same_basename_different_content_remains_distinct(self):
        observation = self.observation("/producer")
        duplicate = dict(observation["sdk_inputs"][-1], path="/elsewhere/libstd.rlib", sha256="sha256:" + "b" * 64)
        observation["sdk_inputs"].append(duplicate)
        names = [item["name"] for item in _public_observation(observation)["sdk_inputs"]]
        self.assertEqual(len(names), len(set(names)))

    def test_runtime_protocol_is_embedded_without_rewriting(self):
        protocol = dict(schema="madb.native-runtime-protocol/1", provider="rust", capacities=dict(handles=4096))
        public = _public_observation(self.observation("/producer"), protocol)
        self.assertEqual(public["runtime_protocol"], protocol)
        protocol["capacities"]["handles"] = 1
        self.assertEqual(public["runtime_protocol"]["capacities"]["handles"], 4096)


if __name__ == "__main__":
    unittest.main()
