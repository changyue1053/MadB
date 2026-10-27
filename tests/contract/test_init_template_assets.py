"""Versioned authored-template preservation and complete-byte regeneration."""
from pathlib import Path
import hashlib
import importlib.util
import io
import json
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("build_init_template", ROOT / "scripts/build_init_template.py")
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)


class InitTemplateAssetsTests(unittest.TestCase):
    def test_every_version_exactly_regenerates(self):
        for bundle in BUILDER.bundles():
            with self.subTest(version=bundle.name):
                for path, expected in BUILDER.products(bundle).items():
                    self.assertEqual(path.read_bytes(), expected, str(path))

    def test_original_shipped_bytes_are_preserved(self):
        bundle = BUILDER.BUNDLE
        self.assertEqual(hashlib.sha256((bundle / "package.tar").read_bytes()).hexdigest(),
                         "97d0e9fd5747bd5ceea920574e75c8e17825e379037c94b68e878649a310a850")
        self.assertEqual(hashlib.sha256((bundle / "source/meta/template.json").read_bytes()).hexdigest(),
                         "3b23424744c85311b2b3b1aadb055bf20f72f623aee42696888a85f767fb1eda")

    def test_all_archive_entries_and_references_match_original_bytes(self):
        for bundle in BUILDER.bundles():
            raw = (bundle / "package.tar").read_bytes()
            pins = json.loads((bundle / "identity.json").read_bytes())
            self.assertEqual(pins["archive"], BUILDER.identity("template-package.tar", raw))
            with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as archive:
                names = archive.getnames()
                self.assertEqual(names, sorted(names))
                files = {entry.name: archive.extractfile(entry).read() for entry in archive}
            sources = {path.relative_to(bundle / "source").as_posix(): path.read_bytes()
                       for path in (bundle / "source").rglob("*") if path.is_file()}
            self.assertEqual(files, sources)
            manifest = json.loads(files["meta/template.json"])
            self.assertEqual(manifest["files"], [BUILDER.identity(name, value) for name, value in sorted(files.items()) if name != "meta/template.json"])
            self.assertEqual(pins["manifest"], {**BUILDER.identity("meta/template.json", files["meta/template.json"]), "schema": "madb.project-template/1"})
            for asset in manifest["assets"]:
                if asset["source"] is not None:
                    source = asset["source"]
                    self.assertEqual(source, BUILDER.identity(source["path"], files[source["path"]]))
            for boundary in manifest["boundary_refs"]:
                profile_ref = boundary["profile"]
                self.assertEqual(profile_ref, {**BUILDER.identity(profile_ref["path"], files[profile_ref["path"]]), "schema": "madb.profile/1"})
                sdk = json.loads(files[profile_ref["path"]])["sdk"]
                self.assertEqual(sdk, {**BUILDER.identity(sdk["path"], files[sdk["path"]]), "schema": "madb.native-toolchain/1"})

    def test_new_version_binds_native_and_build_tools_without_mixing_sdk_scope(self):
        source = BUILDER.BUNDLE / "versions/0.1.1/source"
        manifest = json.loads((source / "meta/template.json").read_bytes())
        profile = json.loads((source / "profile.json").read_bytes())
        sdk = json.loads((source / "toolchain.json").read_bytes())
        native = [{key: tool[key] for key in ("id", "version", "sha256")}
                  for tool in profile["tools"]]
        self.assertEqual(native, [{key: tool[key] for key in ("id", "version", "sha256")}
                                  for tool in sdk["tools"]])
        self.assertEqual(len(native), 10)
        self.assertFalse({"init-cmake", "init-ninja"}.intersection(tool["id"] for tool in native))
        expected = sorted(native + [
            {"id": "init-cmake", "version": "4.4.4", "sha256": "sha256:80cbc0d5ee08c9e6166bcc44bf51faf5469e360346f5cefbf36e8f8e4351a4cb"},
            {"id": "init-ninja", "version": "1.13.2", "sha256": "sha256:458cdf3a6cf289118da14cc897b5106d152c66545947329b016629d9df085815"},
        ], key=lambda tool: tool["id"])
        requirements = manifest["profiles"][0]["requirements"]
        self.assertEqual(requirements["build"]["tools"], expected)
        self.assertEqual(requirements["run"]["tools"], expected)
        self.assertEqual(requirements["structure"]["tools"], [])

    def test_shipped_legacy_version_cannot_be_regenerated_with_new_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = Path(temporary) / "legacy"
            shutil.copytree(BUILDER.BUNDLE / "source", bundle / "source")
            (bundle / "source/assets/main.c").write_text("different author input\n")
            with patch.object(BUILDER, "BUNDLE", bundle):
                with self.assertRaisesRegex(ValueError, "0.1.0 is immutable"):
                    BUILDER.products(bundle)

    def test_new_version_must_match_directory_and_reject_source_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = Path(temporary) / "0.1.1"
            shutil.copytree(BUILDER.BUNDLE / "versions/0.1.1/source", bundle / "source")
            manifest = bundle / "source/meta/template.json"
            document = json.loads(manifest.read_bytes())
            document["version"] = "0.1.0"
            manifest.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, "version differs"):
                BUILDER.products(bundle)
            document["version"] = "0.1.1"
            manifest.write_text(json.dumps(document))
            (bundle / "source/alias").symlink_to("assets/author.rs")
            with self.assertRaisesRegex(ValueError, "ordinary files"):
                BUILDER.products(bundle)


if __name__ == "__main__":
    unittest.main()
