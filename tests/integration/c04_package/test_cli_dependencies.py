"""Public CLI native preparation and explicit receipt rebinding, without hidden discovery."""
import json
from pathlib import Path
import tempfile

from cli_fixture import CliWorkspace, generation_task, ref, save
from dependency_fixture import consume_generated, find_clang, host_target, materialize_sources
from support import PackageFixture, build_binary, byte_digest, tree_bytes


class NativePreparationCli(PackageFixture):
    @classmethod
    def setUpClass(cls):
        cls.binary = build_binary("madb")
        cls.clang = find_clang()
        cls.compiler_bytes = cls.clang.read_bytes()

    def setUp(self):
        super().setUp()
        self.marker_temp = tempfile.TemporaryDirectory(prefix="madb-native-cli-markers-")
        self.marker = Path(self.marker_temp.name) / "calls.jsonl"
        self.fixture = CliWorkspace(self.root, self.marker)
        values = self.fixture.values
        tool = next(tool for tool in values["profile"]["tools"] if tool["id"] == "fixture-clang")
        self.fixture.installed["path"] = (self.root / "tools/python").relative_to("/").as_posix()
        self.fixture.refresh_generation()
        materialize_sources(self.root)
        self.compiler_reference = dict(path=self.clang.relative_to("/").as_posix(), sha256=tool["sha256"], size=len(self.compiler_bytes))
        self.dependency_request = dict(schema="madb.dependency-request/1", compiler=self.compiler_reference,
                                      target=host_target(), sources=["source/main.c"], include_directories=["include"],
                                      absent=[], arguments=["-std=c11"], environment={})
        save(self.root, "dependencies.json", self.dependency_request)
        self.bundle = self.fixture.generate_bundle
        self.bundle["dependencies"] = dict(request=ref(self.root, "dependencies.json", "madb.dependency-request/1"), receipt=None)
        save(self.root, "generate.json", self.bundle)

    def tearDown(self):
        super().tearDown()
        self.marker_temp.cleanup()

    def execute(self, operation="generate", *, prepare=False, tool_root="/", expected=0):
        arguments = [str(self.binary), operation, "--root", str(self.root), "--tool-root", str(tool_root),
                     "--execution", "generate.json", "--frozen"]
        if prepare:
            arguments.extend(["--task", "dependencies"])
        return self.finish(self.spawn(arguments), expected=expected, timeout=90)

    def prepare(self):
        out, err = self.execute("build", prepare=True)
        self.assertEqual(err, "")
        self.assertIn("DependenciesPrepared", out)
        values = dict(line.split(": ", 1) for line in out.splitlines() if ": " in line)
        self.assertFalse(self.marker.exists(), "native preparation must not invoke the arbitrary recipe peer")
        self.assertFalse((self.domain / "current.json").exists())
        return values

    def bind_receipt(self, prepared, *, include_files=True):
        receipt = Path(prepared["Receipt"])
        self.assertTrue(receipt.is_relative_to(self.root))
        name = receipt.relative_to(self.root).as_posix()
        self.bundle["dependencies"]["receipt"] = ref(self.root, name, "madb.dependency-manifest/1")
        manifest = json.loads(receipt.read_text())
        recipe = self.bundle["recipe"]
        if include_files:
            inputs = {item["path"]: item for item in recipe["inputs"]}
            inputs.update({item["path"]: dict(item, schema="madb.source-input/1") for item in manifest["files"]})
            recipe["inputs"] = sorted(inputs.values(), key=lambda item: item["path"])
        request = self.fixture.generate_request
        request["task_digest"] = generation_task(request, recipe)
        save(self.root, "generate-request.json", request)
        recipe["request"] = ref(self.root, "generate-request.json", "madb.generate-request/1")
        save(self.root, "generate.json", self.bundle)

    def test_explicit_preparation_cold_hot_outputs_are_consumed_and_receipt_runs_generator(self):
        cold = self.prepare()
        self.assertEqual(cold["Cache"], "miss")
        consume_generated(self, self.clang, Path(cold["Output"]).read_bytes(), 7, self.root / "cold-consumer")
        hot = self.prepare()
        self.assertEqual(hot["Cache"], "hit")
        self.assertEqual(hot["DependencyKey"], cold["DependencyKey"])
        self.assertEqual(Path(hot["Output"]).read_bytes(), Path(cold["Output"]).read_bytes())
        consume_generated(self, self.clang, Path(hot["Output"]).read_bytes(), 7, self.root / "hot-consumer")
        self.bind_receipt(hot)
        out, err = self.execute()
        self.assertIn("Committed", out)
        self.assertIn("CacheDisabled", out, "the native receipt must not certify an arbitrary generator")
        self.assertEqual(err, "")
        manifest = json.loads((self.fixture.layer("source") / "outputs.json").read_text())
        self.assertEqual(manifest, self.fixture.generation_expected())

    def test_missing_receipt_requests_explicit_preparation_without_starting_tools(self):
        _, err = self.execute(expected=2)
        self.assertIn("DEP005", err)
        self.assertIn("build --task dependencies", err)
        self.assertFalse(self.marker.exists())
        self.assertEqual(list((self.root / "runs").iterdir()), [])
        self.assertEqual(list((self.root / "cache").iterdir()), [])
        self.assertFalse((self.domain / "current.json").exists())

    def test_receipt_does_not_silently_add_undeclared_discovered_inputs(self):
        prepared = self.prepare()
        self.bind_receipt(prepared, include_files=False)
        _, err = self.execute(expected=2)
        self.assertIn("DEP005", err)
        self.assertIn("explicitly rebind", err)
        self.assertFalse(self.marker.exists())
        self.assertFalse((self.domain / "current.json").exists())

    def test_receipt_rejects_changed_installed_compiler_with_original_request_lock_and_receipt(self):
        prepared = self.prepare()
        self.bind_receipt(prepared)
        self.execute()
        before = (self.domain / "current.json").read_bytes()
        source_before = tree_bytes(self.fixture.layer("source"))
        replacement = self.root / "alternate-tools"
        patched = replacement / self.compiler_reference["path"]
        patched.parent.mkdir(parents=True)
        data = bytearray(self.compiler_bytes)
        data[-1] ^= 1
        patched.write_bytes(data)
        original = {name: (self.root / name).read_bytes() for name in ["dependencies.json", "madb.lock", "profiles/rust-c.json"]}
        _, err = self.execute(tool_root=replacement, expected=2)
        self.assertIn("DEP005", err)
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual(tree_bytes(self.fixture.layer("source")), source_before)
        for name, data in original.items():
            self.assertEqual((self.root / name).read_bytes(), data)
        self.assertEqual(len(self.marker.read_text().splitlines()), 1)

    def test_native_target_mismatch_is_rejected_before_compiler_execution(self):
        self.dependency_request["target"] = "x86_64-unknown-linux-gnu" if "apple" in host_target() else "arm64-apple-macos11.0"
        save(self.root, "dependencies.json", self.dependency_request)
        self.bundle["dependencies"]["request"] = ref(self.root, "dependencies.json", "madb.dependency-request/1")
        save(self.root, "generate.json", self.bundle)
        _, err = self.execute("build", prepare=True, expected=2)
        self.assertIn("DEP003", err)
        self.assertIn("target", err)
        self.assertFalse(self.marker.exists())
        self.assertEqual(list((self.root / "runs").iterdir()), [])
        self.assertFalse((self.domain / "current.json").exists())

    def test_managed_domains_inside_include_search_tree_are_rejected_before_any_write(self):
        for field in ["store", "cache"]:
            with self.subTest(field=field):
                original = self.bundle[field]
                self.bundle[field] = "include/managed-" + field
                save(self.root, "generate.json", self.bundle)
                before = tree_bytes(self.root)
                _, err = self.execute("build", prepare=True, expected=4)
                self.assertIn("PIPE006", err)
                self.assertFalse(self.marker.exists())
                self.assertEqual(tree_bytes(self.root), before)
                self.bundle[field] = original
