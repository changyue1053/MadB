"""Exercise Cargo-resolved positive and negative boundaries in isolated copies."""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECK = Path(__file__).with_name("check_dependencies.py")


class DependencyBoundaries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="madb-architecture-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
            shutil.copy2(ROOT / name, self.root / name)
        shutil.copytree(ROOT / "crates", self.root / "crates")
        shutil.copytree(ROOT / "adapters", self.root / "adapters")

    def check_dependency(self, owner, section, declaration, expected):
        directory = self.root / "adapters/contracts" if owner == "madb-contracts" else self.root / "crates" / owner
        manifest = directory / "Cargo.toml"
        content = manifest.read_text()
        heading = f"[{section}]"
        if heading in content:
            content = content.replace(heading, heading + "\n" + declaration, 1)
        else:
            content += f"\n{heading}\n{declaration}\n"
        manifest.write_text(content)
        subprocess.run(
            ["cargo", "generate-lockfile", "--offline"], cwd=self.root,
            check=True, capture_output=True, text=True,
        )
        result = subprocess.run(
            [sys.executable, str(CHECK), "--root", str(self.root)],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0 if expected is None else 1, result.stderr)
        if expected:
            self.assertIn(expected, result.stderr)
            self.assertNotIn("ARCH000", result.stderr)

    def test_declared_domain_dependency_is_allowed(self):
        self.check_dependency("madb-plan", "dependencies",
                              'madb-model = { path = "../madb-model" }', None)

    def test_core_cannot_import_wire_types(self):
        self.check_dependency("madb-model", "dependencies",
                              'madb-protocol = { path = "../madb-protocol" }',
                              "ARCH003: madb-model -> madb-protocol")

    def test_build_dependency_does_not_bypass_boundary(self):
        self.check_dependency("madb-rules", "build-dependencies",
                              'madb-protocol = { path = "../madb-protocol" }',
                              "ARCH003: madb-rules -> madb-protocol")

    def test_optional_target_alias_does_not_bypass_boundary(self):
        self.check_dependency("madb-app", 'target.\'cfg(target_os = "windows")\'.dependencies',
                              'wire_alias = { package = "madb-protocol", path = "../madb-protocol", optional = true }',
                              "ARCH003: madb-app -> madb-protocol")

    def test_dev_dependency_does_not_hide_a_private_import(self):
        self.check_dependency("madb-plan", "dev-dependencies",
                              'madb-app = { path = "../madb-app" }',
                              "ARCH003: madb-plan -> madb-app")

    def test_core_cannot_import_contract_adapter(self):
        self.check_dependency("madb-plan", "dev-dependencies",
                              'madb-contracts = { path = "../../adapters/contracts" }',
                              "ARCH003: madb-plan -> madb-contracts")

    def test_adapter_cannot_import_composition_root(self):
        self.check_dependency("madb-contracts", "dependencies",
                              'madb-cli = { path = "../../crates/madb-cli" }',
                              "ARCH003: madb-contracts -> madb-cli")


if __name__ == "__main__":
    unittest.main()
