"""C06 source provenance, closure rejection and public path redaction."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.kotlin_rust import project

spec = importlib.util.spec_from_file_location("c06_entrypoint", ROOT / "scripts/build_managed_routes.py")
entry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(entry)


def write(root, name, content):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    return path


def reference(root, name):
    data = (root / name).read_bytes()
    return dict(path=name, sha256="sha256:" + hashlib.sha256(data).hexdigest(), size=len(data))


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-c06-project-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def test_source_closure_keeps_locks_and_excludes_build_caches(self):
        repo, frozen = self.root / "repo", self.root / "frozen"
        repo.mkdir(); frozen.mkdir()
        for name in project.EXTRA_INPUTS:
            write(repo, name, "author\n")
        write(repo, "docs/schemas/test.schema.json", "{}")
        write(repo, "sdk/rust/uniffi/Cargo.lock", "locked\n")
        write(repo, "sdk/rust/uniffi/src/lib.rs", "pub fn api() {}\n")
        write(repo, "sdk/rust/uniffi/target/release/leak.rs", "not author\n")
        write(repo, "sdk/kotlin/skie/gradle/wrapper/gradle-wrapper.jar", "wrapper\n")
        write(repo, "sdk/kotlin/skie/.gradle/cache", "private\n")
        names, schemas = project._copy_sources(repo, frozen)
        self.assertIn("sdk/rust/uniffi/Cargo.lock", names)
        self.assertIn("sdk/kotlin/skie/gradle/wrapper/gradle-wrapper.jar", names)
        self.assertNotIn("sdk/rust/uniffi/target/release/leak.rs", names)
        self.assertFalse(any(".gradle" in name for name in names))
        self.assertEqual(schemas, ["docs/schemas/test.schema.json"])
        self.assertEqual((frozen / "sdk/rust/uniffi/Cargo.lock").read_text(), "locked\n")

    def test_author_symlink_cannot_escape_or_alias_source(self):
        repo, frozen = self.root / "repo", self.root / "frozen"
        repo.mkdir(); frozen.mkdir()
        outside = write(self.root, "outside.rs", "unowned\n")
        (repo / "model.rs").symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "escapes"):
            project._copy_file(repo, frozen, "model.rs")

    def test_missing_selected_dependency_closure_is_a_failure(self):
        with self.assertRaisesRegex(ValueError, "cargo-vendor"):
            project._required_trees({"trees": {"runtime-dependencies": {}}}, "swift-rust")
        with self.assertRaisesRegex(ValueError, "skie-maven"):
            project._required_trees({"trees": {"runtime-dependencies": {}}}, "swift-kotlin")
        project._required_trees({"trees": {"runtime-dependencies": {}, "cargo-vendor": {}, "python-runtime": {}}}, "kotlin-native-rust")

    def test_stale_gradle_resolution_source_is_rejected(self):
        source = write(self.root, "sdk/kotlin/skie/gradle/verification-metadata.xml", "original\n")
        expected = "sha256:" + hashlib.sha256(source.read_bytes()).hexdigest()
        write(self.root, "adapters/toolchains/kotlin_rust/skie-dependencies.json", json.dumps({"resolved_from": {"path": "sdk/kotlin/skie/gradle/verification-metadata.xml", "sha256": expected}}))
        project._verify_resolution_source(self.root, "swift-kotlin")
        source.write_text("changed\n")
        with self.assertRaisesRegex(ValueError, "different Gradle"):
            project._verify_resolution_source(self.root, "swift-kotlin")

    def test_runtime_protocol_author_documents_are_accepted(self):
        for variant in ("kotlin-jvm-rust", "kotlin-native-rust", "swift-kotlin"):
            with self.subTest(variant=variant):
                name = project.VARIANTS[variant]["protocol"]
                self.assertEqual(project._load_runtime_protocol(ROOT, variant), json.loads((ROOT / name).read_text()))

    def test_public_observation_rejects_missing_or_unknown_protocol_schema(self):
        name = "sdk/kotlin/skie/runtime-protocol.json"
        original = json.loads((ROOT / name).read_text())
        for schema in (None, "madb.native-runtime-protocol/2"):
            with self.subTest(schema=schema):
                value = dict(original)
                value.pop("schema")
                if schema is not None:
                    value["schema"] = schema
                write(self.root, name, json.dumps(value))
                with self.assertRaisesRegex(ValueError, "unsupported runtime protocol schema"):
                    project._public_observation(self.root, {}, {"variant": "swift-kotlin"})

    def test_runtime_protocol_rejects_missing_or_unknown_format_version(self):
        name = "sdk/kotlin/skie/runtime-protocol.json"
        original = json.loads((ROOT / name).read_text())
        for version in (None, "2.0.0"):
            with self.subTest(version=version):
                value = dict(original)
                value.pop("schema_version")
                if version is not None:
                    value["schema_version"] = version
                write(self.root, name, json.dumps(value))
                with self.assertRaisesRegex(ValueError, "unsupported C06 runtime protocol schema_version"):
                    project._load_runtime_protocol(self.root, "swift-kotlin")

    def test_runtime_protocol_rejects_missing_and_noninteger_budgets(self):
        name = "sdk/kotlin/skie/runtime-protocol.json"
        original = json.loads((ROOT / name).read_text())
        for bad in (None, True, 0, -1, 1.5, "1024"):
            with self.subTest(budget=bad):
                value = dict(original, budgets=dict(original["budgets"]))
                if bad is None:
                    value["budgets"].pop("max_payload_bytes")
                    diagnostic = "missing required budgets"
                else:
                    value["budgets"]["max_payload_bytes"] = bad
                    diagnostic = "budget must be a positive integer.*#budgets/max_payload_bytes"
                write(self.root, name, json.dumps(value))
                with self.assertRaisesRegex(ValueError, diagnostic):
                    project._load_runtime_protocol(self.root, "swift-kotlin")

    def test_public_observation_has_no_host_paths(self):
        for name, text in [
            ("sdk/kotlin/skie/runtime-protocol.json", (ROOT / "sdk/kotlin/skie/runtime-protocol.json").read_text()),
            ("adapters/toolchains/kotlin_rust/toolchain.json", '{"tools":"fixture"}'),
            ("sdk/kotlin/skie/gradle/verification-metadata.xml", "verified\n"),
            ("sdk/kotlin/skie/gradle.lockfile", "locked\n"),
        ]:
            write(self.root, name, text)
        hash_ = "sha256:" + "1" * 64
        observation = dict(target="aarch64-apple-darwin", system="Darwin", managed=dict(root="/private/tool-root", receipt_sha256=hash_, paths={"jdk": "/private/tool-root/jdk"}),
                           tools={"compiler": dict(id="compiler", version="1", sha256=hash_, size=12, path="/private/compiler")},
                           trees={"sdk": dict(root="/Applications/SDK", sha256=hash_, entries=2, size=20)}, xcode={"version":"27.0"},
                           python_runtime=dict(version="3.14.4", system_build="host-build", system_libraries=["/usr/lib/libSystem.B.dylib"],
                                               root="/private/python", search_paths=["/private/python/lib"], libraries=[{"path":"/private/native.dylib"}]))
        public = project._public_observation(self.root, observation, {"variant":"swift-kotlin", "test_support":True, "phase_timeout_seconds":900, "phase_termination_grace_ms":6000})
        encoded = json.dumps(public)
        self.assertNotIn("/private/", encoded)
        self.assertNotIn("/Applications/", encoded)
        self.assertEqual(public["managed_installation"]["receipt_sha256"], hash_)
        self.assertEqual(public["trees"][0]["entries"], 2)
        self.assertEqual(public["compiler"]["configuration"], "Release")
        self.assertEqual(public["execution_policy"], {"phase_timeout_seconds":900, "phase_termination_grace_ms":6000})
        self.assertEqual(set(public["python_runtime"]), {"version", "system_build", "system_libraries"})

    def test_metadata_environment_does_not_inherit_compiler_overrides(self):
        observation = dict(environment={"LC_ALL": "C", "RUSTUP_TOOLCHAIN": "1.99.1"}, sdk="/verified/sdk",
                           tools={"rustc": {"path": "/verified/rust/bin/rustc"}},
                           trees={"cargo-vendor": {"root": "/verified/vendor"}})
        with patch.dict("os.environ", {"RUSTC": "/unverified/rustc", "RUSTFLAGS": "--cfg injected", "CARGO_BUILD_TARGET": "wrong-target"}):
            environment = entry._cargo_environment(observation, self.root / "cargo-home", "profile")
        self.assertEqual(environment["RUSTC"], "/verified/rust/bin/rustc")
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
        self.assertNotIn("RUSTFLAGS", environment)
        self.assertNotIn("CARGO_BUILD_TARGET", environment)
        self.assertNotIn("/unverified", environment["PATH"])

    def test_declaration_helper_cannot_outlive_its_author_source(self):
        path = write(self.root, "sdk/helper.rs", "original parser\n")
        observation = dict(declaration_helpers={"metadata_v032": dict(inputs=[reference(self.root, "sdk/helper.rs")], features=[])})
        project._verify_helper_sources(self.root, observation, "swift-rust")
        path.write_text("changed parser\n")
        with self.assertRaisesRegex(ValueError, "stale for frozen author"):
            project._verify_helper_sources(self.root, observation, "swift-rust")

    def test_declaration_helper_requires_provenance(self):
        with self.assertRaisesRegex(ValueError, "no author source proof"):
            project._verify_helper_sources(self.root, {}, "kotlin-jvm-rust")

    def test_legacy_python_observation_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "complete Python runtime closure"):
            project._verify_python_runtime({"tools": {"python": {"version": "3.14.4"}}})

    def fixture_revision(self):
        repo, frozen = self.root / "repo", self.root / "frozen"
        repo.mkdir(); frozen.mkdir()
        write(repo, "sdk/model.udl", "namespace fixture {};\n")
        subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
        subprocess.run(["git", "add", "sdk/model.udl"], cwd=repo, check=True)
        subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], cwd=repo, check=True)
        write(frozen, "sdk/model.udl", "namespace fixture {};\n")
        source = reference(frozen, "sdk/model.udl")
        prepared = dict(author_inputs=[source], derived_inputs=[], lock={"schemas":[]}, read={"inputs":[source]})
        return repo, frozen, entry.revision(repo), prepared

    def test_formal_revision_proof_rejects_unlisted_input(self):
        repo, frozen, revision, prepared = self.fixture_revision()
        entry.verify_revision_inputs(repo, frozen, prepared, revision)
        prepared["read"]["inputs"].append(dict(path="untracked.json", sha256="sha256:"+"0"*64, size=0))
        with self.assertRaisesRegex(RuntimeError, "unverified input"):
            entry.verify_revision_inputs(repo, frozen, prepared, revision)

    def test_formal_revision_proof_rejects_copied_source_mutation(self):
        repo, frozen, revision, prepared = self.fixture_revision()
        write(frozen, "sdk/model.udl", "namespace substituted {};\n")
        with self.assertRaisesRegex(RuntimeError, "differs from producer revision"):
            entry.verify_revision_inputs(repo, frozen, prepared, revision)


if __name__ == "__main__":
    unittest.main()
