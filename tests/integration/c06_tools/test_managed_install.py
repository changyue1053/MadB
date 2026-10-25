"""Reject mutable compiler images and unverified dependency acquisition."""
from pathlib import Path
import hashlib
import io
import json
import sys
import os
import py_compile
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.kotlin_rust import install_tools as tools
from adapters.toolchains.kotlin_rust import tools as observation
from adapters.toolchains.kotlin_rust import python_runtime


class ManagedInstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_cache_hit_is_rehashed_without_network(self):
        payload = b"source archive"
        checksum = hashlib.sha256(payload).hexdigest()
        source = self.root / checksum
        source.write_bytes(payload)
        spec = dict(source="https://example.invalid/tool.zip", sha256=checksum, size=len(payload))
        with patch.object(tools.urllib.request, "urlopen", side_effect=AssertionError("network")):
            self.assertEqual(tools.acquire(spec, self.root), source)
            source.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "cached source SHA-256"):
                tools.acquire(spec, self.root)

    def test_download_mismatch_never_publishes_cache_entry(self):
        expected = hashlib.sha256(b"expected").hexdigest()
        spec = dict(source="https://example.invalid/tool.zip", sha256=expected)
        with patch.object(tools.urllib.request, "urlopen", return_value=io.BytesIO(b"wrong")):
            with self.assertRaisesRegex(ValueError, "download SHA-256 mismatch"):
                tools.acquire(spec, self.root)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_zip_cannot_escape_extraction_root(self):
        archive = self.root / "bad.zip"
        with zipfile.ZipFile(archive, "w") as writer:
            writer.writestr("../escape", "bad")
        with self.assertRaisesRegex(ValueError, "invalid installation path"):
            tools.extract(archive, dict(format="zip", archive_root="tool", path="tool"), self.root / "install")
        self.assertFalse((self.root / "escape").exists())

    def test_tar_cannot_escape_extraction_root(self):
        archive = self.root / "bad.tar.gz"
        with tarfile.open(archive, "w:gz") as writer:
            member = tarfile.TarInfo("../escape")
            member.size = 3
            writer.addfile(member, io.BytesIO(b"bad"))
        with self.assertRaises(tarfile.OutsideDestinationError):
            tools.extract(archive, dict(format="tar.gz", archive_root="tool", path="tool"), self.root / "install")

    def test_receipt_binds_jars_not_only_launcher(self):
        recipe = self.root / "recipe.json"
        spec = dict(version="1", source="https://example.invalid/tool.zip", sha256="0" * 64,
                    format="zip", archive_root="tool", path="tool")
        recipe.write_text(json.dumps(dict(archives=dict(tool=spec))))
        installation = self.root / "install"
        image = installation / "tool"
        image.mkdir(parents=True)
        (image / "launcher").write_text("script")
        (image / "compiler.jar").write_bytes(b"compiler")
        receipt = installation / "receipt.json"
        receipt.write_text(json.dumps(dict(schema="madb.managed-installation/1", recipe_sha256=tools.digest(recipe),
            components=dict(tool=dict(source=spec, files=tools.inventory(image))))))
        pinned = tools.digest(receipt)
        with patch.object(tools, "RECIPE", recipe):
            self.assertEqual(tools.verify(installation, pinned)["paths"]["tool"], str(image.resolve()))
            (image / "compiler.jar").write_bytes(b"injected")
            with self.assertRaisesRegex(ValueError, "installed distribution changed"):
                tools.verify(installation, pinned)
            receipt.write_text(receipt.read_text() + " ")
            with self.assertRaisesRegex(ValueError, "receipt changed"):
                tools.verify(installation, pinned)

    def test_external_symlink_is_rejected(self):
        image = self.root / "image"
        image.mkdir()
        (image / "compiler").symlink_to("/usr/bin/true")
        with self.assertRaisesRegex(ValueError, "symlink escapes"):
            tools.inventory(image)

    def test_maven_traversal_is_rejected_before_fetch(self):
        lock = self.root / "maven.json"
        lock.write_text(json.dumps(dict(schema="madb.maven-files/1", files=[dict(path="../outside")])))
        with patch.object(tools, "acquire", side_effect=AssertionError("must not fetch")):
            with self.assertRaisesRegex(ValueError, "invalid installation path"):
                tools.install_dependencies(lock, self.root / "dependencies")


    def test_tree_digest_detects_injected_dependency(self):
        image = self.root / "image"
        image.mkdir()
        (image / "compiler.jar").write_bytes(b"compiler")
        first = observation.snapshot(image)
        (image / "unexpected.jar").write_bytes(b"injected")
        self.assertNotEqual(observation.snapshot(image), first)

    def test_dependency_tree_is_closed_against_lock(self):
        root = self.root / "dependencies"
        root.mkdir()
        jar = root / "runtime.jar"
        jar.write_bytes(b"runtime")
        lock = self.root / "lock.json"
        lock.write_text(json.dumps(dict(schema="madb.maven-files/1", files=[
            dict(path=jar.name, source="https://example.invalid/runtime.jar",
                 sha256=tools.digest(jar).removeprefix("sha256:"), size=jar.stat().st_size)])))
        observed = dict(dependency_locks={}, trees={})
        observation.register_dependencies(observed, "runtime", root, lock)
        self.assertIn("runtime", observed["trees"])
        (root / "extra.jar").write_bytes(b"extra")
        with self.assertRaisesRegex(ValueError, "closed inventory"):
            observation.register_dependencies(observed, "runtime", root, lock)



    def test_multicall_tool_keeps_invocation_name_and_pins_resolved_image(self):
        image = self.root / "frontend"
        image.write_bytes(b"compiler")
        launcher = self.root / "symbolgraph-extract"
        launcher.symlink_to(image.name)
        observed = dict(tools={}, trees={}, dependency_locks={}, gobley=None,
                        managed=dict(root="unused", receipt_sha256="unused"))
        entry = observation.register_image(observed, "symbolgraph", launcher, "1", "test://compiler")
        self.assertEqual(entry["path"], str(launcher))
        self.assertEqual(entry["image_path"], str(image.resolve()))
        self.assertEqual(entry["invocation_name"], launcher.name)
        with patch.object(observation, "verify_managed"):
            observation.verify(observed)
            replacement = self.root / "different-frontend"
            replacement.write_bytes(image.read_bytes())
            launcher.unlink()
            launcher.symlink_to(replacement.name)
            with self.assertRaisesRegex(ValueError, "image or launcher changed"):
                observation.verify(observed)

    def test_multicall_tool_rejects_image_mutation(self):
        image = self.root / "frontend"
        image.write_bytes(b"compiler")
        launcher = self.root / "symbolgraph-extract"
        launcher.symlink_to(image.name)
        observed = dict(tools={}, trees={}, dependency_locks={}, gobley=None,
                        managed=dict(root="unused", receipt_sha256="unused"))
        observation.register_image(observed, "symbolgraph", launcher, "1", "test://compiler")
        image.write_bytes(b"tampered")
        with patch.object(observation, "verify_managed"):
            with self.assertRaisesRegex(ValueError, "image or launcher changed"):
                observation.verify(observed)


    def test_python_runtime_only_ignores_disabled_import_locations(self):
        root = self.root / "python"
        (root / "lib/__pycache__").mkdir(parents=True)
        (root / "lib/module.py").write_text("source")
        first = observation.snapshot(root, python_runtime.SCOPE)
        (root / "lib/__pycache__/module.pyc").write_bytes(b"cache")
        (root / "lib/site-packages").symlink_to("/outside/not-followed")
        self.assertEqual(observation.snapshot(root, python_runtime.SCOPE), first)
        (root / "lib/sourceless.pyc").write_bytes(b"importable bytecode")
        self.assertNotEqual(observation.snapshot(root, python_runtime.SCOPE), first)
        (root / "lib/escape").symlink_to("/usr/bin/true")
        with self.assertRaisesRegex(ValueError, "symlink escapes"):
            observation.snapshot(root, python_runtime.SCOPE)

    def test_isolated_python_ignores_existing_unchecked_bytecode_cache(self):
        module = self.root / "cached_module.py"
        module.write_text("VALUE = 'injected'\n")
        py_compile.compile(str(module), invalidation_mode=py_compile.PycInvalidationMode.UNCHECKED_HASH)
        module.write_text("VALUE = 'source'\n")
        source = "import sys;sys.path.insert(0," + repr(str(self.root)) + ");import cached_module;print(cached_module.VALUE)"
        value = subprocess.check_output([sys.executable, *python_runtime.ISOLATION, "-c", source], text=True)
        self.assertEqual(value.strip(), "source")

    def test_isolated_python_does_not_execute_site_customization(self):
        marker = self.root / "executed"
        (self.root / "sitecustomize.py").write_text("open(" + repr(str(marker)) + ",'w').write('bad')")
        env = dict(os.environ, PYTHONPATH=str(self.root))
        value = subprocess.check_output([sys.executable, *python_runtime.ISOLATION, "-c", "import sys;print(sys.flags.no_site,sys.flags.isolated)"], env=env, text=True)
        self.assertEqual(value.strip(), "1 1")
        self.assertFalse(marker.exists())

    def test_python_dynamic_dependency_retarget_is_rejected(self):
        lib = self.root / "library.dylib"
        lib.write_bytes(b"native")
        alias = self.root / "alias.dylib"
        alias.symlink_to(lib)
        observed = dict(tools=dict(python=dict(invocation_arguments=python_runtime.ISOLATION)),
            python_runtime=dict(libraries=[dict(path=str(alias), image_path=str(lib.resolve()),
                sha256=tools.digest(lib), size=lib.stat().st_size)]))
        python_runtime.verify(observed)
        other = self.root / "other.dylib"
        other.write_bytes(lib.read_bytes())
        alias.unlink(); alias.symlink_to(other)
        with self.assertRaisesRegex(ValueError, "native dependency changed"):
            python_runtime.verify(observed)

    def test_verifier_loads_from_current_frozen_input_root(self):
        fake_tool = self.root / "adapters/toolchains/kotlin_rust/tools.py"
        fake_driver = self.root / "adapters/generators/gobley/driver.py"
        fake_driver.parent.mkdir(parents=True)
        fake_driver.write_text("def verify_install(root): return 'frozen-root'\n")
        with patch.object(observation, "__file__", str(fake_tool)):
            loaded = observation.load_verifier("gobley")
            self.assertEqual(loaded.verify_install(None), "frozen-root")
            self.assertEqual(Path(loaded.__file__), fake_driver.resolve())


if __name__ == "__main__":
    unittest.main()
