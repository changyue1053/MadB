"""CRIT-10: real compiler discovery and consumed cold/hot generated C."""
import copy
import json
from pathlib import Path
import shutil
import sys
import unittest

from dependency_fixture import (consume_generated, expected_value, find_clang, host_target,
                                materialize_sources, set_base, set_optional)
from support import PackageFixture, build_binary, byte_digest, tree_bytes, write_json


class Dependencies(PackageFixture):
    @classmethod
    def setUpClass(cls):
        cls.probe = build_binary("dependency_probe", example=True)
        cls.clang = find_clang()
        image = cls.clang.read_bytes()
        cls.compiler = {"path": cls.clang.name, "sha256": byte_digest(image), "size": len(image)}

    def setUp(self):
        super().setUp()
        self.source = self.root / "workspace"
        materialize_sources(self.source)
        self.runs = self.root / "runs"
        self.runs.mkdir()
        self.cache = self.root / "cache"
        self.sequence = 0
        self.config = {
            "source_root": str(self.source), "tool_root": str(self.clang.parent),
            "run_parent": str(self.runs), "cache_root": str(self.cache),
            "context": byte_digest(b"fixture-profile-toolchain-v1"),
            "request": {
                "schema": "madb.dependency-request/1", "compiler": self.compiler.copy(),
                "target": host_target(),
                "sources": ["source/main.c"], "include_directories": ["include"],
                "absent": [], "arguments": ["-std=c11"], "environment": {},
            },
        }

    def invoke(self, config=None):
        self.sequence += 1
        config = copy.deepcopy(config or self.config)
        config["run_name"] = "case-" + str(self.sequence)
        path = self.root / ("request-" + str(self.sequence) + ".json")
        write_json(path, config)
        process = self.spawn([str(self.probe), str(path)])
        out, err = self.finish(process, timeout=90)
        try:
            result = json.loads(out)
        except json.JSONDecodeError as error:
            self.fail(f"probe did not return a JSON result: {error}: {out!r} {err!r}")
        self.assertTrue(result["owners_drained"], result)
        return result

    def accepted(self, result, *, hit, value=7):
        self.assertEqual(result["status"], "ok", result)
        self.assertEqual(result["cache_hit"], hit, result)
        self.assertEqual(set(result["files"]), {"generated.c"}, result)
        generated = bytes(result["files"]["generated.c"])
        consume_generated(self, self.clang, generated, value,
                          self.root / ("consumer-" + str(self.sequence)))
        return generated

    def cold(self, *, value=7):
        result = self.invoke()
        self.accepted(result, hit=False, value=value)
        return result

    def changed(self, previous, *, value=7):
        result = self.invoke()
        self.accepted(result, hit=False, value=value)
        self.assertNotEqual(result["input_key"], previous["input_key"])
        hot = self.invoke()
        self.accepted(hot, hit=True, value=value)
        self.assertEqual(hot["files"], result["files"])
        return result

    def test_cold_and_hot_outputs_are_compiled_and_executed(self):
        cold = self.cold()
        hot = self.invoke()
        self.accepted(hot, hit=True)
        self.assertEqual(hot["input_key"], cold["input_key"])
        self.assertEqual(hot["files"], cold["files"])
        self.assertIn("include/detail/value.h", cold["manifest"]["discovered"])
        self.assertEqual(cold["manifest"], hot["manifest"])

    def test_indirect_include_change_changes_generated_behavior(self):
        cold = self.cold()
        set_base(self.source, 19)
        changed = self.changed(cold, value=expected_value(base=19))
        self.assertNotEqual(changed["files"], cold["files"])

    def test_macro_change_changes_generated_behavior(self):
        cold = self.cold()
        self.config["request"]["arguments"].append("-DSCALE=3")
        changed = self.changed(cold, value=expected_value(scale=3))
        self.assertNotEqual(changed["files"], cold["files"])

    def test_explicit_target_changes_identity_and_actual_builtin_expansion(self):
        (self.source / "source/main.c").write_text(
            "#if defined(__aarch64__)\n#define ARCH_VALUE 64\n"
            "#elif defined(__x86_64__)\n#define ARCH_VALUE 86\n"
            "#else\n#error unexpected fixture target\n#endif\n"
            "int read_value(void) { return ARCH_VALUE; }\n")
        targets = ["arm64-apple-macos11.0", "x86_64-apple-macos10.15"] if sys.platform == "darwin" else [
            "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
        self.config["request"]["target"] = targets[0]
        cold = self.cold(value=64)
        self.config["request"]["target"] = targets[1]
        changed = self.changed(cold, value=86)
        self.assertNotEqual(cold["files"], changed["files"])

    def test_explicit_environment_is_part_of_identity(self):
        self.config["request"]["environment"] = {"LC_ALL": "C"}
        cold = self.cold()
        self.config["request"]["environment"] = {"LC_ALL": "POSIX"}
        self.changed(cold)

    def test_profile_and_locked_component_context_changes_invalidate(self):
        cold = self.cold()
        self.config["context"] = byte_digest(b"fixture-profile-toolchain-v2")
        self.changed(cold)

    def test_include_search_order_changes_generated_behavior(self):
        alternate = self.source / "alternate"
        alternate.mkdir()
        (alternate / "entry.h").write_text("#define BASE_VALUE 23\n")
        self.config["request"]["include_directories"] = ["include", "alternate"]
        cold = self.cold()
        self.config["request"]["include_directories"] = ["alternate", "include"]
        self.changed(cold, value=23)

    def test_optional_header_appearance_and_removal_invalidate(self):
        cold = self.cold()
        set_optional(self.source, 11)
        present = self.changed(cold, value=expected_value(optional=11))
        (self.source / "include/optional.h").unlink()
        absent = self.invoke()
        # The original exact closure may reuse the earlier immutable cache entry.
        self.accepted(absent, hit=True)
        self.assertEqual(absent["input_key"], cold["input_key"])
        self.assertNotEqual(present["files"], absent["files"])

    def test_search_directory_member_changes_invalidate(self):
        cold = self.cold()
        (self.source / "include/unused.h").write_text("#define UNUSED 1\n")
        self.changed(cold)

    def test_unrelated_workspace_change_preserves_a_hit(self):
        outside = self.source / "outside"
        outside.mkdir()
        (outside / "noise.txt").write_text("first")
        cold = self.cold()
        (outside / "noise.txt").write_text("changed unrelated bytes")
        hot = self.invoke()
        self.accepted(hot, hit=True)
        self.assertEqual(hot["input_key"], cold["input_key"])

    def assert_corruption_retained(self, before):
        quarantined = [path for path in self.cache.iterdir()
                       if path.is_dir() and path.name.startswith("quarantine-")]
        self.assertEqual(len(quarantined), 1, quarantined)
        self.assertEqual(tree_bytes(quarantined[0]), before)

    def test_corrupt_cached_output_is_a_miss_and_rebuilt(self):
        cold = self.cold()
        damaged = Path(cold["cache_paths"]["output"])
        damaged.chmod(0o600)
        damaged.write_bytes(b"wrong output\n")
        before = tree_bytes(Path(cold["cache_paths"]["entry"]))
        rebuilt = self.invoke()
        self.accepted(rebuilt, hit=False)
        self.assertEqual(rebuilt["files"], cold["files"])
        self.assert_corruption_retained(before)

    def test_missing_cached_output_is_a_miss_and_rebuilt(self):
        cold = self.cold()
        missing = Path(cold["cache_paths"]["output"])
        missing.parent.chmod(0o700)
        missing.unlink()
        before = tree_bytes(Path(cold["cache_paths"]["entry"]))
        rebuilt = self.invoke()
        self.accepted(rebuilt, hit=False)
        self.assertEqual(rebuilt["files"], cold["files"])
        self.assert_corruption_retained(before)

    def test_missing_dependency_manifest_is_a_miss_and_rebuilt(self):
        cold = self.cold()
        Path(cold["cache_paths"]["dependency"]).unlink()
        before = tree_bytes(Path(cold["cache_paths"]["entry"]))
        rebuilt = self.invoke()
        self.accepted(rebuilt, hit=False)
        self.assertEqual(rebuilt["files"], cold["files"])
        self.assert_corruption_retained(before)

    def test_corrupt_cache_metadata_is_a_miss_and_rebuilt(self):
        cold = self.cold()
        damaged = Path(cold["cache_paths"]["metadata"])
        damaged.chmod(0o600)
        damaged.write_text("{broken json")
        before = tree_bytes(Path(cold["cache_paths"]["entry"]))
        rebuilt = self.invoke()
        self.accepted(rebuilt, hit=False)
        self.assertEqual(rebuilt["files"], cold["files"])
        self.assert_corruption_retained(before)

    def test_unsupported_options_cannot_mint_cache_eligibility(self):
        self.config["request"]["arguments"].append("-fmodules")
        result = self.invoke()
        self.assertEqual(result["status"], "uncacheable", result)
        self.assertNotIn("input_key", result)
        self.assertEqual(list(self.cache.rglob("generated.c")) if self.cache.exists() else [], [])

    def test_changed_compiler_image_is_rejected_against_lock(self):
        tools = self.root / "tools"
        tools.mkdir()
        copied = tools / self.clang.name
        shutil.copy2(self.clang, copied)
        with copied.open("r+b") as stream:
            stream.seek(-1, 2)
            original = stream.read(1)
            stream.seek(-1, 2)
            stream.write(bytes([original[0] ^ 1]))
        self.config["tool_root"] = str(tools)
        result = self.invoke()
        self.assertEqual(result["status"], "error", result)
        self.assertEqual(result["codes"], ["DEP003"], result)
        self.assertNotIn("input_key", result)

    def test_time_dependent_macros_cannot_enable_cache(self):
        source = self.source / "source/main.c"
        original = source.read_text()
        for macro in ["__DATE__", "__TIME__", "__TIMESTAMP__"]:
            with self.subTest(macro=macro):
                source.write_text(original + f"const char *hidden_time = {macro};\n")
                result = self.invoke()
                self.assertEqual(result["status"], "error", result)
                self.assertIn("Wdate-time", result["preprocess"]["stderr"], result)
                self.assertNotIn("input_key", result)
                self.assertEqual(list(self.cache.rglob("generated.c")), [])

    def test_caller_complete_flag_cannot_grant_cache_eligibility(self):
        self.config["request"]["complete"] = True
        result = self.invoke()
        self.assertEqual(result["status"], "error", result)
        self.assertEqual(result["codes"], ["DEP001"], result)
        self.assertNotIn("input_key", result)

    def test_generated_bytes_do_not_embed_random_work_paths(self):
        cold = self.cold()
        second = self.root / "independent-workspace"
        materialize_sources(second)
        self.config["source_root"] = str(second)
        self.config["cache_root"] = str(self.root / "independent-cache")
        repeated = self.invoke()
        self.accepted(repeated, hit=False)
        self.assertEqual(repeated["input_key"], cold["input_key"])
        self.assertEqual(repeated["files"], cold["files"])
        self.assertNotIn(str(self.root).encode(), bytes(repeated["files"]["generated.c"]))


if __name__ == "__main__":
    unittest.main()
