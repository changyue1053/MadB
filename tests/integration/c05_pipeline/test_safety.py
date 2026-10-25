"""Public packaging and pre-execution image rejection; no native compiler required."""
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.rust_c import pipeline, tools


# Expectations use actual bytes and independent hashes, never the producer's
# serialization or identity helpers. Fake executables leave real marker files.
def digest(path):
    return "sha256:" + hashlib.sha256(Path(path).read_bytes()).hexdigest()


def reference(root, name):
    path = root / name
    return {"path": name, "sha256": digest(path), "size": path.stat().st_size}


def write_executable(path, marker, output):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "#!" + sys.executable + "\nfrom pathlib import Path\n"
        + "Path(" + repr(str(marker)) + ").write_text('executed')\n"
        + "print(" + repr(output) + ")\n"
    )
    path.chmod(0o755)


class PackageRejectionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="madb-c05-package-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.inputs, self.work = self.root / "inputs", self.root / "work"
        self.work.mkdir()
        for name in ("adapters/artifacts/rust_c/pack.py", "adapters/toolchains/rust_c/common.py", "adapters/toolchains/common/documents.py"):
            target = self.inputs / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        header = self.inputs / "include/sdk.h"
        header.parent.mkdir()
        self.payload = b"typedef unsigned long FixtureHandle;\n"
        header.write_bytes(self.payload)
        self.inventory = [reference(self.inputs, "include/sdk.h")]
        self.request = json.loads((ROOT / "tests/fixtures/contracts/package-request.json").read_text())

    def package(self, inventory):
        (self.inputs / "package-files.json").write_text(json.dumps(inventory))
        return subprocess.run(
            [sys.executable, "../inputs/adapters/artifacts/rust_c/pack.py"],
            input=json.dumps(self.request), text=True, capture_output=True,
            cwd=self.work, env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}, timeout=15,
        )

    def assert_rejected(self, inventory, message):
        result = self.package(inventory)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertFalse((self.work / "candidate.json").exists())

    def test_valid_inventory_publishes_the_selected_bytes(self):
        result = self.package(self.inventory)
        self.assertEqual(result.returncode, 0, result.stderr)
        candidate = json.loads(result.stdout)
        self.assertEqual(candidate["schema"], "madb.candidate/1")
        self.assertEqual(candidate["files"], self.inventory)
        self.assertEqual(json.loads((self.work / "candidate.json").read_text()), candidate)
        self.assertEqual((self.work / "include/sdk.h").read_bytes(), self.payload)

    def test_wrong_inventory_digest_cannot_publish_a_candidate(self):
        inventory = deepcopy(self.inventory)
        inventory[0]["sha256"] = "sha256:" + "0" * 64
        self.assert_rejected(inventory, "package source changed")
        self.assertFalse((self.work / "include/sdk.h").exists())

    def test_wrong_inventory_size_cannot_publish_a_candidate(self):
        inventory = deepcopy(self.inventory)
        inventory[0]["size"] += 1
        self.assert_rejected(inventory, "package source changed")
        self.assertFalse((self.work / "include/sdk.h").exists())

    def test_parent_and_absolute_paths_do_not_escape_package_work(self):
        outside = self.root / "outside.bin"
        outside.write_bytes(b"must stay unchanged")
        for name in ("../outside.bin", str(outside)):
            with self.subTest(path=name):
                item = {"path": name, "sha256": digest(outside), "size": outside.stat().st_size}
                self.assert_rejected([item], "invalid logical package path")
                self.assertEqual(outside.read_bytes(), b"must stay unchanged")

    def test_parent_symlink_cannot_import_files_outside_frozen_inputs(self):
        outside = self.root / "external"
        outside.mkdir()
        payload = outside / "private.bin"
        payload.write_bytes(b"outside frozen inputs")
        (self.inputs / "escape").symlink_to(outside, target_is_directory=True)
        item = {"path": "escape/private.bin", "sha256": digest(payload), "size": payload.stat().st_size}
        self.assert_rejected([item], "package source escapes the frozen input root")
        self.assertFalse((self.work / "escape").exists())
        self.assertEqual(payload.read_bytes(), b"outside frozen inputs")

    def test_duplicate_and_recursive_inventories_cannot_publish(self):
        (self.inputs / "candidate.json").write_bytes(b"not a nested candidate")
        for inventory in (self.inventory * 2, [reference(self.inputs, "candidate.json")]):
            with self.subTest(paths=[item["path"] for item in inventory]):
                self.assert_rejected(inventory, "duplicate or recursive candidate inventory")
                self.assertEqual(list(self.work.iterdir()), [])


class HostImageRejectionTests(unittest.TestCase):
    def test_replaced_host_is_rejected_before_the_executable_can_run(self):
        with tempfile.TemporaryDirectory(prefix="madb-c05-host-") as temporary:
            root = Path(temporary)
            python = Path(sys.executable).resolve()
            prepared = {
                "config": {"tools": {"python": {"id": "python", "version": "test",
                    "path": str(python), "sha256": digest(python), "size": python.stat().st_size}}},
                "read": {}, "lock": {},
            }
            for same_size in (True, False):
                with self.subTest(same_size=same_size):
                    work = root / ("same-size" if same_size else "different-size")
                    work.mkdir()
                    host, marker = work / "host", work / "host-ran"
                    write_executable(host, marker, "original")
                    original = host.read_bytes()
                    subject = pipeline.Pipeline(ROOT, work, prepared, host)
                    if same_size:
                        host.write_bytes(original.replace(b"original", b"replaced"))
                        self.assertEqual(host.stat().st_size, len(original))
                    else:
                        host.write_bytes(original + b"# changed image size\n")
                    with self.assertRaisesRegex(RuntimeError, "locked MadB host image changed"):
                        subject.execute("build", {})
                    self.assertFalse(marker.exists(), "host executed before its stored identity was verified")


class ExplicitProcessBudgetTests(unittest.TestCase):
    def subject(self, root, config):
        python = Path(sys.executable).resolve()
        host = root / "host"
        host.write_text("#!" + str(python) + "\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n")
        host.chmod(0o755)
        prepared = {"config": {"tools": {"python": {"id": "python", "version": "test",
            "path": str(python), "sha256": digest(python), "size": python.stat().st_size}}, **config},
            "read": {}, "lock": {}}
        return pipeline.Pipeline(ROOT, root, prepared, host)

    def test_default_route_preserves_cli_budgets(self):
        with tempfile.TemporaryDirectory(prefix="madb-default-budget-") as temporary:
            args = json.loads(self.subject(Path(temporary), {}).execute("build", {}))
            self.assertNotIn("--process-timeout-seconds", args)
            self.assertNotIn("--process-termination-grace-ms", args)

    def test_explicit_route_budgets_reach_the_owner(self):
        with tempfile.TemporaryDirectory(prefix="madb-managed-budget-") as temporary:
            config = {"phase_timeout_seconds": 900, "phase_termination_grace_ms": 6000}
            args = json.loads(self.subject(Path(temporary), config).execute("build", {}))
            self.assertEqual(args[args.index("--process-timeout-seconds") + 1], "900")
            self.assertEqual(args[args.index("--process-termination-grace-ms") + 1], "6000")

    def test_invalid_budget_is_rejected_before_owner_launch(self):
        for key, values in (("phase_timeout_seconds", (0, 3601, True, "900")),
                            ("phase_termination_grace_ms", (-1, 30001, True, "6000"))):
            for value in values:
                with self.subTest(key=key, value=value), tempfile.TemporaryDirectory(prefix="madb-bad-budget-") as temporary:
                    root = Path(temporary)
                    with self.assertRaisesRegex(ValueError, "phase.*must be between"):
                        self.subject(root, {key: value}).execute("build", {})
                    self.assertFalse((root / "build-execution.log").exists())


class GeneratorReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="madb-c05-generator-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.repo = self.root / "repo"
        recipe_path = self.repo / "adapters/toolchains/rust_c/toolchain.json"
        recipe_path.parent.mkdir(parents=True)
        shutil.copyfile(ROOT / "adapters/toolchains/rust_c/toolchain.json", recipe_path)
        self.recipe = json.loads(recipe_path.read_text())
        self.host = "aarch64-apple-darwin"
        native = self.root / "native"
        self.native = {}
        for name in ("rustc", "clang", "ar", "nm"):
            path = native / "bin" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"environment query fixture; never executed")
            self.native[name] = path
        self.libclang = native / "lib/libclang.dylib"
        self.libclang.parent.mkdir()
        self.libclang.write_bytes(b"library identity fixture")
        finalizer = self.repo / "target/debug/madb-native-finalize"
        finalizer.parent.mkdir(parents=True)
        finalizer.write_bytes(b"finalizer identity fixture")
        self.sdk = self.root / "sdk"
        (self.sdk / "usr/lib").mkdir(parents=True)
        (self.sdk / "SDKSettings.json").write_bytes(b"{}")
        (self.sdk / "usr/lib/libSystem.tbd").write_bytes(b"SDK identity fixture")
        self.sysroot = self.root / "rust-sysroot"
        (self.sysroot / "lib/rustlib" / self.host / "lib").mkdir(parents=True)
        self.resource = self.root / "clang-resource"
        self.resource.mkdir()
        self.generators, self.markers = {}, {}
        install = self.root / "generators"
        self.receipt = {"rust": self.recipe["rust"], "generators": {}}
        for name in ("bindgen", "cbindgen"):
            path, marker = install / "bin" / name, self.root / (name + "-ran")
            write_executable(path, marker, name + " " + self.recipe["generators"][name]["version"])
            self.generators[name], self.markers[name] = path, marker
            self.receipt["generators"][name] = {
                **self.recipe["generators"][name], "binary_sha256": digest(path),
            }
        self.receipt_path = install / "receipt.json"
        self.real_query = tools.query

    def query_environment_or_execute_generator(self, *arguments):
        key = tuple(map(str, arguments))
        # Only system/toolchain discovery is simulated. If discover invokes either
        # generator before checking its receipt, the actual process leaves a marker.
        if key[0] in {str(path) for path in self.generators.values()}:
            return self.real_query(*arguments)
        answers = {
            ("rustup", "which", "--toolchain", self.recipe["rust"], "rustc"): str(self.native["rustc"]),
            (str(self.native["rustc"]), "-vV"): "host: " + self.host,
            (str(self.native["rustc"]), "--version"): "rustc " + self.recipe["rust"],
            (str(self.native["rustc"]), "--print", "sysroot"): str(self.sysroot),
            ("xcrun", "--show-sdk-path"): str(self.sdk),
            ("xcodebuild", "-version"): self.recipe["xcode"],
            ("xcrun", "--show-sdk-version"): self.recipe["sdk_version"],
            ("xcrun", "--show-sdk-build-version"): self.recipe["sdk_build"],
            (str(self.native["clang"]), "--version"): self.recipe["clang"],
            (str(self.native["clang"]), "--print-resource-dir"): str(self.resource),
        }
        answers.update({("xcrun", "--find", name): str(self.native[name]) for name in ("clang", "ar", "nm")})
        if key not in answers:
            raise AssertionError("unexpected environment discovery query: " + repr(key))
        return answers[key]

    def discover(self):
        self.receipt_path.write_text(json.dumps(self.receipt))
        with patch.object(tools.platform, "system", return_value="Darwin"), \
             patch.object(tools, "query", side_effect=self.query_environment_or_execute_generator):
            return tools.discover(self.repo, self.generators["bindgen"], self.generators["cbindgen"])

    def test_wrong_bindgen_image_receipt_does_not_execute_the_generator(self):
        self.receipt["generators"]["bindgen"]["binary_sha256"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(RuntimeError, "bindgen provenance/installed image receipt differs"):
            self.discover()
        self.assertFalse(self.markers["bindgen"].exists())
        self.assertFalse(self.markers["cbindgen"].exists())

    def test_wrong_bindgen_source_receipt_does_not_execute_the_generator(self):
        self.receipt["generators"]["bindgen"]["source"] = "https://invalid.example/unreviewed-source.crate"
        with self.assertRaisesRegex(RuntimeError, "bindgen provenance/installed image receipt differs"):
            self.discover()
        self.assertFalse(self.markers["bindgen"].exists())
        self.assertFalse(self.markers["cbindgen"].exists())

    def test_matching_receipts_allow_the_actual_version_probe_processes(self):
        observation = self.discover()
        self.assertEqual(observation["target"], self.host)
        for name in ("bindgen", "cbindgen"):
            self.assertEqual(self.markers[name].read_text(), "executed")
            self.assertEqual(observation["tools"][name]["sha256"], digest(self.generators[name]))


if __name__ == "__main__":
    unittest.main()
