"""The standalone SDK gate must reject real Cargo dependency declarations."""
from pathlib import Path
import importlib.util
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("check_native_sdk", ROOT / "scripts/check_native_sdk.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class NativeSdkBoundaries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="madb-native-boundaries-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        shutil.copy2(ROOT / "rust-toolchain.toml", self.root / "rust-toolchain.toml")
        for directory in CHECK.SDK_PACKAGES:
            shutil.copytree(ROOT / directory, self.root / directory,
                            ignore=shutil.ignore_patterns("target", "__pycache__"))

    def inject_dependency(self, package, section, name="madb-host", alias="private_alias"):
        private = self.root / "crates" / name
        (private / "src").mkdir(parents=True)
        (private / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n')
        (private / "src/lib.rs").write_text("pub fn private_tool_entry() {}\n")
        manifest = self.root / "sdk/rust" / package / "Cargo.toml"
        optional = ", optional = true" if section.endswith("dependencies") and "dev-dependencies" not in section else ""
        with manifest.open("a") as output:
            output.write(f'\n[{section}]\n{alias} = {{ package = "{name}", path = "../../../crates/{name}"{optional} }}\n')
        return CHECK.inspect_boundaries(self.root)

    def test_reviewed_sdk_templates_are_standalone(self):
        self.assertEqual(CHECK.inspect_boundaries(self.root), [])

    def test_engine_rejects_aliased_host_dependency(self):
        errors = self.inject_dependency("engine", "dependencies")
        self.assertTrue(any("NATIVE003" in error and "madb-host as private_alias" in error for error in errors), errors)
        self.assertFalse(any("NATIVE000" in error for error in errors), errors)

    def test_c_client_rejects_build_dependency_on_app(self):
        errors = self.inject_dependency("c-client", "build-dependencies", "madb-app")
        self.assertTrue(any("NATIVE003" in error and "build / all targets" in error for error in errors), errors)

    def test_optional_other_target_does_not_hide_private_import(self):
        errors = self.inject_dependency("engine", 'target.\'cfg(target_os = "windows")\'.dependencies', "madb-cli")
        self.assertTrue(any("NATIVE003" in error and 'cfg(target_os = "windows")' in error for error in errors), errors)

    def test_dev_dependency_cannot_hide_private_port_import(self):
        errors = self.inject_dependency("c-client", "dev-dependencies", "private-ports", "ordinary_name")
        self.assertTrue(any("NATIVE003" in error and "private-ports as ordinary_name" in error for error in errors), errors)

    def test_sdk_cannot_join_host_workspace(self):
        manifest = self.root / "sdk/rust/engine/Cargo.toml"
        manifest.write_text(manifest.read_text().replace("[workspace]\n", ""))
        errors = CHECK.inspect_boundaries(self.root)
        self.assertTrue(any("NATIVE002" in error and "own empty [workspace]" in error for error in errors), errors)

    def test_missing_sdk_does_not_become_skipped_success(self):
        (self.root / "sdk/rust/c-client/Cargo.toml").unlink()
        errors = CHECK.inspect_boundaries(self.root)
        self.assertTrue(any("NATIVE001" in error for error in errors), errors)

    def test_c06_rejects_private_host_in_dev_dependencies(self):
        errors = self.inject_dependency("uniffi", "dev-dependencies", "madb-host", "innocent")
        self.assertTrue(any("NATIVE003" in error and "madb-host as innocent" in error for error in errors), errors)

    def test_c06_external_dependency_version_cannot_float(self):
        manifest = self.root / "sdk/rust/uniffi/Cargo.toml"
        manifest.write_text(manifest.read_text().replace('version = "=0.32.2"', 'version = "0.32.2"'))
        errors = CHECK.inspect_boundaries(self.root)
        self.assertTrue(any("NATIVE003" in error and "uniffi" in error for error in errors), errors)

    def test_shared_sdk_runtime_cannot_import_host(self):
        errors = self.inject_dependency("boundary-core", "dependencies", "madb-host")
        self.assertTrue(any("NATIVE003" in error and "madb-boundary-core" in error for error in errors), errors)

    def test_c07_provider_cannot_import_tool_host(self):
        errors = self.inject_dependency("arkts-provider", "dependencies", "madb-host", "hidden")
        self.assertTrue(any("NATIVE003" in error and "madb-host as hidden" in error for error in errors), errors)
        self.assertFalse(any("NATIVE000" in error for error in errors), errors)

    def test_c07_provider_cannot_import_other_sdk_runtime(self):
        errors = self.inject_dependency("arkts-provider", "dev-dependencies", "madb-boundary-core", "hidden")
        self.assertTrue(any("NATIVE003" in error and "madb-boundary-core as hidden" in error for error in errors), errors)


    def test_c07_bridge_rejects_host_dev_dependency(self):
        errors = self.inject_dependency("arkts-bridge", "dev-dependencies", "madb-host", "hidden")
        self.assertTrue(any("NATIVE003" in error and "madb-host as hidden" in error for error in errors), errors)

    def test_c07_bridge_cannot_enable_implicit_gc_frontend(self):
        manifest = self.root / "sdk/rust/arkts-bridge/Cargo.toml"
        manifest.write_text(manifest.read_text().replace('features = ["napi3"]', 'features = ["napi4"]'))
        errors = CHECK.inspect_boundaries(self.root)
        self.assertTrue(any("NATIVE003" in error and "napi-ohos" in error for error in errors), errors)

    def test_c07_bridge_cannot_import_c06_runtime(self):
        dependency = {"name": "madb-boundary-core", "kind": "normal", "source": None,
                      "uses_default_features": True, "features": [],
                      "path": str(self.root / "sdk/rust/boundary-core")}
        self.assertFalse(CHECK.approved_dependency(self.root, "madb-arkts-bridge", dependency))


if __name__ == "__main__":
    unittest.main()
