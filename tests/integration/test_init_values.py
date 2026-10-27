"""Installed init preparation and exact-source refusal; no target certification."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.rust_c import init_values as init
from adapters.toolchains.rust_c import values_worker
from adapters.toolchains.common.documents import canonical


def original_inputs():
    source = ROOT / "templates/projects/rust-c-sdk-values/source"
    profile_bytes = (source / "profile.json").read_bytes()
    sdk_bytes = (source / "toolchain.json").read_bytes()
    profile, sdk = json.loads(profile_bytes), json.loads(sdk_bytes)
    tools = {}
    for item in sdk["tools"]:
        key = "values_reader" if item["id"] == "madb-native-values-read" else item["id"].removeprefix("c05-")
        tools[key] = {**item, "path": "/fixed-tools/" + key}
    sdk_inputs = []
    for item in sdk["sdk_inputs"]:
        name = item["name"]
        if name.startswith("sdk/"):
            name = "/sdk/" + name.removeprefix("sdk/")
        elif name.startswith("clang-resource/"):
            name = "/clang/" + name.removeprefix("clang-resource/")
        else:
            name = "/native/" + name.removeprefix("native/").removesuffix("-" + item["sha256"][7:])
        sdk_inputs.append(dict(path=name, sha256=item["sha256"], size=item["size"]))
    compiler = sdk["compiler"]
    observation = dict(schema=sdk["schema"], target=sdk["target"], system=sdk["system"], tools=tools,
        sdk_inputs=sdk_inputs, environment={"LC_ALL": "C"},
        clang_arguments=["-target", compiler["target"], "-isysroot", "/sdk", "-resource-dir", "/clang",
            "-mmacosx-version-min=" + compiler["deployment_target"], "-std=" + compiler["language_standard"],
            *compiler["warnings"]])
    return profile_bytes, sdk_bytes, observation


def minimal_plan():
    return dict(namespace="reading", errors=[],
        records=[dict(name="Reading", fields=[dict(name="value", kind="i32")])],
        functions=[dict(name="normalize", record="Reading",
                        parameters=[dict(name="reading", kind="Reading")], error=None)])


class InstalledProfileTests(unittest.TestCase):
    def test_original_profile_and_public_sdk_match_the_fixed_observation(self):
        profile, sdk, observation = original_inputs()
        with patch.object(init, "verify_tool") as verify:
            init.validate_profile(json.loads(profile), sdk, observation)
        self.assertEqual(verify.call_count, len(observation["tools"]) + len(observation["sdk_inputs"]))

    def test_profile_tool_alias_or_sdk_content_change_is_not_re_pinned(self):
        original, sdk, observation = original_inputs()
        for change, message in (
            (lambda p, o: p["tools"][0].__setitem__("id", "new-alias"), "profile tools"),
            (lambda p, o: o["tools"]["python"].__setitem__("version", "different"), "profile tools"),
            (lambda p, o: o["sdk_inputs"][0].__setitem__("size", 1), "original SDK"),
            (lambda p, o: o["tools"]["python"].__setitem__("arguments", ["-c", "hidden"]), "hidden invocation"),
            (lambda p, o: p.__setitem__("schema", "madb.profile/2"), "outside"),
            (lambda p, o: p["sdk"].__setitem__("path", "nested/toolchain.json"), "original SDK"),
        ):
            profile, observed = json.loads(original), deepcopy(observation)
            change(profile, observed)
            with self.subTest(message=message), patch.object(init, "verify_tool") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    init.validate_profile(profile, sdk, observed)
                verify.assert_not_called()

    def test_sdk_extra_bytes_require_the_original_ref_not_just_equal_json(self):
        profile, sdk, observation = original_inputs()
        with self.assertRaisesRegex(ValueError, "original SDK"):
            init.validate_profile(json.loads(profile), sdk + b"\n", observation)

    def test_json_duplicate_and_inexact_number_rejected(self):
        for data in (b'{"id":1,"id":2}', b'{"size":0.5}', b'{"size":9007199254740992}', b'{"size":NaN}'):
            with self.subTest(data=data), self.assertRaises(ValueError):
                init.decode(data)


class InstalledPreparationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="madb-init-values-中文-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.native = self.root / "native"
        self.native.mkdir()
        for name in (*init.RUNTIME_FILES, *init.SCHEMA_FILES):
            target = self.native / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        profile, sdk, observation = original_inputs()
        self.originals = dict(profile=profile, sdk=sdk, author=b"natural Rust author bytes\n")
        for name, data in self.originals.items():
            (self.root / name).write_bytes(data)
        (self.native / "installation.json").write_text(canonical(observation))
        (self.native / "execution-resources.json").write_bytes(b'{"fixture":"private installed resource bytes"}\n')
        self.options = SimpleNamespace(profile=self.root / "profile", sdk=self.root / "sdk",
            author=self.root / "author", observation=self.native / "installation.json", namespace="reading")

    def prepare(self, *, dependency_failure=False):
        with patch.object(init, "verify_tool"), \
             patch.object(init.values_driver, "read_plan", return_value=minimal_plan()), \
             patch.object(init.values_driver, "generated_headers", return_value={}), \
             patch.object(init.values_driver, "header_dependencies", side_effect=ValueError("unlocked SDK header") if dependency_failure else None) as dependencies, \
             patch.object(init, "finish_context", return_value={}) as finish:
            result = init.prepare(self.options, self.native)
        return result, dependencies, finish

    def test_original_profile_and_sdk_bytes_are_preserved(self):
        _, dependencies, finish = self.prepare()
        self.assertEqual((self.native / "profile.json").read_bytes(), self.originals["profile"])
        self.assertEqual((self.native / "toolchain.json").read_bytes(), self.originals["sdk"])
        self.assertEqual(finish.call_args.args[1], json.loads(self.originals["profile"]))
        self.assertIn("execution-resources.json", finish.call_args.args[6])
        self.assertIn("installation.json", finish.call_args.args[6])
        # No discovery=True: an undeclared header must fail, never grow the lock.
        self.assertEqual(dependencies.call_args.kwargs, {})
        self.assertEqual(init.load(self.native / "backend.json")["sdk_inputs"],
                         init.load(self.native / "installation.json")["sdk_inputs"])

    def test_undeclared_sdk_stops_before_context_documents(self):
        with self.assertRaisesRegex(ValueError, "unlocked SDK"):
            self.prepare(dependency_failure=True)
        self.assertFalse((self.native / "read.json").exists())
        self.assertFalse((self.native / "madb.lock").exists())
        self.assertEqual((self.native / "profile.json").read_bytes(), self.originals["profile"])

    def test_source_change_after_preparation_does_not_rewrite_the_copy(self):
        self.prepare()
        (self.options.author).write_bytes(b"changed author")
        with patch.object(init, "verify_tool"), self.assertRaisesRegex(ValueError, "original project input changed"):
            init.check_originals(self.options, self.native, initial=False)
        self.assertEqual((self.native / "author.rs").read_bytes(), self.originals["author"])

    def test_existing_profile_is_never_overwritten(self):
        (self.native / "profile.json").write_bytes(b"other owner")
        with self.assertRaises(FileExistsError):
            self.prepare()
        self.assertEqual((self.native / "profile.json").read_bytes(), b"other owner")

    def test_runtime_symlink_is_not_an_installed_source(self):
        path = self.native / "LICENSE"
        path.unlink()
        path.symlink_to(self.root / "author")
        with self.assertRaisesRegex(ValueError, "redirected path"):
            self.prepare()
        self.assertFalse((self.native / "profile.json").exists())


class PlanningOnlyTests(unittest.TestCase):
    def test_driver_does_not_duplicate_an_existing_fixed_import_root(self):
        module_root = str(ROOT / "adapters/generators/rust_c")
        program = ("import sys; root = " + repr(module_root) + "; "
            "sys.path.insert(0, root); before = list(sys.path); "
            "import values_driver; assert sys.path == before")
        result = subprocess.run([sys.executable, "-I", "-S", "-B", "-X", "pycache_prefix=/dev/null", "-c", program],
            env={"LC_ALL": "C"}, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_real_interpreters_confirm_the_fixed_modes(self):
        program = ("import sys; sys.path.insert(0, " + repr(str(ROOT)) + "); "
            "from adapters.toolchains.rust_c import values_worker as worker; "
            "import json; print(json.dumps(worker.interpreter_arguments()))")
        fixed = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]
        for arguments, expected in ((["-B"], []), (fixed, fixed)):
            result = subprocess.run([sys.executable, *arguments, "-c", program],
                env={"LC_ALL": "C"}, capture_output=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), expected)
        for arguments in (["-I", "-S", "-B"], [*fixed, "-X", "dev"]):
            result = subprocess.run([sys.executable, *arguments, "-c", program],
                env={"LC_ALL": "C"}, capture_output=True, timeout=15)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"interpreter mode differs from the fixed recipe", result.stderr)

    def test_interpreter_prefix_requires_actual_complete_mode(self):
        plain = SimpleNamespace(isolated=0, no_site=0, dont_write_bytecode=0,
                                ignore_environment=0, optimize=0)
        isolated = SimpleNamespace(isolated=1, no_site=1, dont_write_bytecode=1,
                                   ignore_environment=1, optimize=0)
        cases = [
            (plain, {}, None, []),
            (SimpleNamespace(**{**vars(plain), "dont_write_bytecode": 1}), {}, None, []),
            (isolated, {"pycache_prefix": "/dev/null"}, "/dev/null",
             ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]),
        ]
        for flags, options, prefix, expected in cases:
            with self.subTest(flags=flags), patch.object(values_worker.sys, "flags", flags), \
                 patch.object(values_worker.sys, "_xoptions", options), patch.object(values_worker.sys, "pycache_prefix", prefix):
                self.assertEqual(values_worker.interpreter_arguments(), expected)
        for flags, options, prefix in (
            (isolated, {}, None),
            (plain, {"pycache_prefix": "/dev/null"}, "/dev/null"),
            (isolated, {"pycache_prefix": "/dev/null", "dev": True}, "/dev/null"),
            (SimpleNamespace(**{**vars(isolated), "no_site": 0}), {"pycache_prefix": "/dev/null"}, "/dev/null"),
        ):
            with self.subTest(flags=flags, options=options), patch.object(values_worker.sys, "flags", flags), \
                 patch.object(values_worker.sys, "_xoptions", options), patch.object(values_worker.sys, "pycache_prefix", prefix), \
                 self.assertRaisesRegex(ValueError, "interpreter mode differs"):
                values_worker.interpreter_arguments()

    def test_absolute_and_parent_input_references_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for name in ("/etc/passwd", "../outside", "nested/../outside", "nested//file"):
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "logical installed"):
                    init.verify_refs(root, [dict(path=name, size=0, sha256="sha256:" + "0" * 64)])

    def test_nested_host_execution_is_unconditionally_rejected(self):
        pipeline = object.__new__(init.InitPipeline)
        with self.assertRaisesRegex(RuntimeError, "cannot launch a nested MadB"):
            pipeline.execute("build", {})

    def test_missing_or_escaping_current_batch_cannot_recover(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "domain").mkdir()
            pipeline = object.__new__(init.InitPipeline)
            pipeline.root = root
            for value in (None, "../../outside", "sha256:../outside", "sha256:" + "A" * 64):
                (root / "domain/current.json").write_text(json.dumps({"source": value}))
                with self.subTest(value=value), self.assertRaisesRegex(ValueError, "invalid committed"):
                    pipeline.layer("source")

    def test_other_source_parent_cannot_be_used_for_packaging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            built = root / ("a" * 64) / "files"
            built.mkdir(parents=True)
            pipeline = SimpleNamespace(layer=lambda _: built, source=root / ("b" * 64) / "files",
                read={"selector": {"profile_digest": "profile"}}, outputs={"generation_id": "generation"},
                contract={"source_digest": "input"})
            metadata = built.parent / "metadata.json"
            metadata.write_text(json.dumps(dict(schema="madb.storage-batch/1", id="sha256:"+"a"*64,
                spec=dict(layer="build", profile_digest="profile", generation_id="generation", input_digest="input", parent="sha256:"+"c"*64), files=[])))
            with self.assertRaisesRegex(ValueError, "not from the current source"):
                init.restore_build(pipeline, "sha256:" + hashlib.sha256(metadata.read_bytes()).hexdigest())

    def test_recomputed_metadata_is_not_a_new_host_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "metadata.json"
            path.write_bytes(b'{"files":[],"id":"original"}\n')
            fixed = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(init.checked_metadata(path, fixed)["id"], "original")
            path.write_bytes(b'{"files":[],"id":"replacement"}\n')
            with self.assertRaisesRegex(ValueError, "differs from the Host receipt"):
                init.checked_metadata(path, fixed)
            for missing in (None, "", "sha256:" + "A" * 64):
                with self.subTest(missing=missing), self.assertRaisesRegex(ValueError, "fixed Host metadata receipt"):
                    init.checked_metadata(path, missing)


if __name__ == "__main__":
    unittest.main()
