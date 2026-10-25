"""Preflight failure semantics using synthetic tool fixtures, not ArkTS evidence."""
from pathlib import Path
import json
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts import preflight
from scripts.check_arkts_environment import write_new_json


class PreflightTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.deveco = self.root / "DevEco-Release/Contents"
        self.sdk = self.root / "Release-SDK"
        self.deveco.mkdir(parents=True)
        (self.deveco / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleVersion": "fixture-build", "CFBundleShortVersionString": "fixture-version"}))
        self.package = {"apiVersion": "23", "version": "fixture-sdk", "releaseType": "Release"}
        self.save(self.sdk / "sdk-pkg.json", {"data": self.package})
        for name in preflight.COMPONENTS:
            self.save(self.sdk / "openharmony" / name / "oh-uni-package.json", self.package)
        self.executable(self.deveco / "tools/node/bin/node", "print('v18.20.1')")
        self.executable(self.sdk / "openharmony/native/llvm/bin/clang", "print('fixture clang 15.0.4')")
        self.target = {key: "fixture" for key in preflight.TARGET_PARAMETERS}
        self.target.update({"const.ohos.apiversion": "23", "const.ohos.releasetype": "Release",
                            "const.product.model": "emulator", "const.product.cpu.abilist": "arm64-v8a"})
        self.target_file = self.root / "parameters.json"
        self.save(self.target_file, self.target)
        self.hdc = self.sdk / "openharmony/toolchains/hdc"
        self.executable(self.hdc, f"""
import json, pathlib, sys
if sys.argv[1:] == ['-v']:
    print('Ver: fixture-hdc')
elif sys.argv[1:6] == ['-p', '-t', 'fixture:5557', 'shell', 'param'] and sys.argv[6] == 'get':
    print(json.loads(pathlib.Path({str(self.target_file)!r}).read_text())[sys.argv[7]])
else:
    sys.exit(91)
""")
        for name, path, entry in (
            ("@ohos/hvigor", "tools/hvigor/hvigor/package.json", "tools/hvigor/bin/hvigorw.js"),
            ("@ohos/hvigor-ohos-plugin", "tools/hvigor/hvigor-ohos-plugin/package.json", None),
            ("@ohos/ohpm", "tools/ohpm/package.json", "tools/ohpm/bin/ohpm"),
        ):
            self.save(self.deveco / path, {"name": name, "version": "fixture-tools"})
            if entry:
                executable = self.deveco / entry
                executable.parent.mkdir(parents=True, exist_ok=True)
                executable.write_text("not executed by metadata-only check")

    def save(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value))

    def executable(self, path, body):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"#!{sys.executable}\n" + body + "\n")
        path.chmod(0o755)

    def inspect(self, **kwargs):
        return preflight.inspect_environment(self.deveco, self.sdk, kwargs.pop("target", "fixture:5557"), **kwargs)

    def codes(self, report):
        return {error["code"] for error in report["errors"]}

    def test_release_observation_does_not_certify_or_claim_physical_device(self):
        result = self.inspect()
        self.assertTrue(result["environment_checks_passed"], result["errors"])
        self.assertFalse(result["certifies_c07"])
        self.assertEqual(result["evidence_scope"], "environment_only")
        self.assertEqual(result["target"]["kind"], "emulator")
        self.assertFalse(result["sdk"]["cpf_sysroot_checked"])
        for observed in result["target"]["observations"]:
            self.assertEqual(observed["command"][1:3], ["-p", "-t"])

    def test_beta_sdk_named_release_still_fails(self):
        beta = {**self.package, "releaseType": "Beta1"}
        self.save(self.sdk / "sdk-pkg.json", {"data": beta})
        for name in preflight.COMPONENTS:
            self.save(self.sdk / "openharmony" / name / "oh-uni-package.json", beta)
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertFalse(result["sdk"]["is_formal"])
        self.assertIn("sdk_not_release", self.codes(result))

    def test_inconsistent_component_and_conflicting_stage_fail(self):
        self.save(self.sdk / "openharmony/ets/oh-uni-package.json",
                  {**self.package, "apiVersion": "24", "stage": "Beta1"})
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertFalse(result["sdk"]["components_consistent"])
        self.assertFalse(result["sdk"]["is_formal"])
        self.assertIn("sdk_component_mismatch", self.codes(result))

    def test_missing_and_malformed_sdk_metadata_are_observed_failures(self):
        (self.sdk / "openharmony/native/oh-uni-package.json").unlink()
        (self.sdk / "sdk-pkg.json").write_text("[]")
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertIn("sdk_metadata_invalid", self.codes(result))
        self.assertEqual(result["target"]["state"], "observed")

    def test_target_omission_keeps_sdk_observations_and_fails(self):
        result = self.inspect(target=None)
        self.assertTrue(result["sdk"]["is_formal"])
        self.assertFalse(result["environment_checks_passed"])
        self.assertEqual(result["target"]["state"], "not_checked")
        self.assertEqual(self.codes(result), {"target_not_checked"})

    def test_target_timeout_is_not_absence_and_client_is_reaped(self):
        pid_file = self.root / "client.pid"
        self.executable(self.hdc, f"""
import os, pathlib, sys, time
if sys.argv[1:] == ['-v']:
    print('Ver: fixture-hdc')
else:
    pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid()))
    time.sleep(30)
""")
        result = self.inspect(timeout=1.5)
        self.assertEqual(result["target"]["state"], "timeout")
        self.assertIn("target_timeout", self.codes(result))
        self.assertNotIn("target_unavailable", self.codes(result))
        with self.assertRaises(ProcessLookupError):
            os.kill(int(pid_file.read_text()), 0)

    def test_zero_exit_connection_failure_is_not_success(self):
        self.executable(self.hdc, "import sys\nprint('Ver: fixture-hdc' if sys.argv[1:] == ['-v'] else 'Connect server failed')")
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertEqual(result["target"]["state"], "unavailable")
        self.assertIn("target_unavailable", self.codes(result))

    def test_zero_exit_empty_target_output_is_not_success(self):
        self.executable(self.hdc, "import sys\nif sys.argv[1:] == ['-v']: print('Ver: fixture-hdc')")
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertEqual(result["target"]["state"], "query_failed")
        self.assertIn("target_query_failed", self.codes(result))

    def test_wrong_architecture_does_not_satisfy_requested_target(self):
        self.target["const.product.cpu.abilist"] = "x86_64"
        self.save(self.target_file, self.target)
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertEqual(result["requirements"]["target_abi"], "arm64-v8a")
        self.assertIn("target_abi_mismatch", self.codes(result))
        self.assertTrue(self.inspect(target_abi="x86_64")["environment_checks_passed"])

    def test_unusable_target_response_and_nonrelease_runtime_fail(self):
        self.target["const.ohos.apiversion"] = "not-an-api"
        self.target["const.ohos.releasetype"] = "Beta1"
        self.save(self.target_file, self.target)
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertIn("target_api_invalid", self.codes(result))
        self.assertIn("target_not_release", self.codes(result))

    def test_no_emulator_marker_does_not_prove_physical_hardware(self):
        self.target["const.product.model"] = "some-model"
        self.save(self.target_file, self.target)
        self.assertEqual(self.inspect()["target"]["kind"], "unverified_device")

    def test_missing_executable_and_hvigor_mismatch_fail(self):
        (self.deveco / "tools/node/bin/node").unlink()
        self.save(self.deveco / "tools/hvigor/hvigor-ohos-plugin/package.json",
                  {"name": "@ohos/hvigor-ohos-plugin", "version": "other-version"})
        result = self.inspect()
        self.assertFalse(result["environment_checks_passed"])
        self.assertIn("tool_probe_failed", self.codes(result))
        self.assertIn("hvigor_component_mismatch", self.codes(result))

    def test_invalid_timeout_is_rejected_before_probing(self):
        for timeout in (0, -1, 31, float("nan")):
            with self.subTest(timeout=timeout), self.assertRaises(ValueError):
                self.inspect(timeout=timeout)

    def test_atomic_output_refuses_overwrite_and_leaves_no_temporary_file(self):
        output = self.root / "report.json"
        value = {"environment_checks_passed": False, "certifies_c07": False}
        write_new_json(output, value)
        self.assertEqual(json.loads(output.read_text()), value)
        with self.assertRaises(FileExistsError):
            write_new_json(output, {"unexpected": True})
        self.assertEqual(json.loads(output.read_text()), value)
        self.assertEqual(list(self.root.glob(".arkts-preflight-*")), [])
        with self.assertRaises(FileNotFoundError):
            write_new_json(self.root / "absent/report.json", value)

    def test_cli_preserves_failed_observation_with_nonzero_exit(self):
        output = self.root / "cli.json"
        result = subprocess.run([sys.executable, str(ROOT / "scripts/check_arkts_environment.py"),
                                 "--deveco", str(self.deveco), "--sdk", str(self.sdk),
                                 "--json-output", str(output)], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads(output.read_text())
        self.assertEqual(self.codes(report), {"target_not_checked"})
        self.assertFalse(report["certifies_c07"])

    def command_line_tools_fixture(self):
        root = self.root / "command-line-tools"
        shutil.copytree(self.deveco / "tools/hvigor", root / "hvigor")
        shutil.copytree(self.deveco / "tools/ohpm", root / "ohpm")
        shutil.copytree(self.deveco / "tools/node", root / "tool/node")
        for name, path, version in (
            ("@ohos/hvigor", "hvigor/hvigor/package.json", "6.26.8"),
            ("@ohos/hvigor-ohos-plugin", "hvigor/hvigor-ohos-plugin/package.json", "6.26.8"),
            ("@ohos/ohpm", "ohpm/package.json", "26.0.0.630"),
        ):
            self.save(root / path, {"name": name, "version": version})
        sdk = {"apiVersion": "26", "version": "26.0.0.105", "releaseType": "Release", "platformVersion": "26.0.0"}
        self.save(self.sdk / "sdk-pkg.json", {"data": sdk})
        for name in preflight.COMPONENTS:
            self.save(self.sdk / "openharmony" / name / "oh-uni-package.json", sdk)
        (root / "version.txt").write_text("""# ======================
# Command Line Tools(mac-arm64)
# Version: 26.0.0.851
# ======================
hstack         : 6.1.0
codelinter     : 6.0.240
hvigor         : 6.26.8
releaseType    : release
ohpm           : 26.0.0.630
HarmonyOS SDK  : HarmonyOS 26.0.0 Release (include Ohos_sdk_public 26.0.0.105 (API Version 26 Release))
apiVersion     : 26
platformVersion: 26.0.0
""")
        return root

    def inspect_command_line_tools(self, root, *, target=None):
        return preflight.inspect_environment(None, self.sdk, target, command_line_tools=root)

    def test_command_line_tools_layout_has_its_own_identity(self):
        root = self.command_line_tools_fixture()
        result = self.inspect_command_line_tools(root, target="fixture:5557")
        self.assertTrue(result["environment_checks_passed"], result["errors"])
        self.assertFalse(result["certifies_c07"])
        self.assertNotIn("deveco", result["tools"])
        identity = result["tools"]["command_line_tools"]
        self.assertEqual(identity["version"], "26.0.0.851")
        self.assertEqual(identity["distribution_platform"], "mac-arm64")
        self.assertEqual(identity["version_source"], "version.txt")
        self.assertEqual(result["tools"]["node"]["path"], str(root / "tool/node/bin/node"))
        self.assertEqual(result["tools"]["@ohos/hvigor"]["version"], "6.26.8")
        self.assertEqual(result["sdk"]["product"]["version"], "26.0.0.105")

    def test_command_line_tools_without_target_retains_valid_metadata(self):
        result = self.inspect_command_line_tools(self.command_line_tools_fixture())
        self.assertEqual(self.codes(result), {"target_not_checked"})
        self.assertFalse(result["environment_checks_passed"])
        self.assertEqual(result["target"]["state"], "not_checked")
        self.assertNotIn("observations", result["target"])

    def test_command_line_tools_missing_and_bad_version_metadata_fail(self):
        root = self.command_line_tools_fixture()
        metadata = root / "version.txt"
        original = metadata.read_text()
        for text in (None, "# Version: broken\n", original + "hvigor: 6.26.8\n",
                     original + "# Version: 26.0.0.999\n", original.replace("apiVersion     : 26\n", ""),
                     original + "# Version: broken\n", original + "# Command Line Tools invalid\n",
                     original.replace("apiVersion     : 26", "apiVersion: zero")):
            with self.subTest(metadata=text):
                metadata.unlink(missing_ok=True)
                if text is not None:
                    metadata.write_text(text)
                result = self.inspect_command_line_tools(root)
                self.assertIn("tool_metadata_invalid", self.codes(result))
                self.assertFalse(result["environment_checks_passed"])

    def test_command_line_tools_wrong_architecture_is_explicit(self):
        root = self.command_line_tools_fixture()
        metadata = root / "version.txt"
        metadata.write_text(metadata.read_text().replace("mac-arm64", "mac-x64"))
        result = self.inspect_command_line_tools(root)
        self.assertIn("tool_platform_unsupported", self.codes(result))
        self.assertEqual(result["tools"]["command_line_tools"]["distribution_platform"], "mac-x64")

    def test_command_line_tools_beta_label_and_internal_conflict_fail(self):
        root = self.command_line_tools_fixture()
        metadata = root / "version.txt"
        metadata.write_text(metadata.read_text().replace("releaseType    : release", "releaseType: beta1"))
        result = self.inspect_command_line_tools(root)
        self.assertIn("tool_not_release", self.codes(result))
        self.assertIn("tool_metadata_conflict", self.codes(result))

    def test_command_line_tools_api_component_and_selected_sdk_conflicts_fail(self):
        root = self.command_line_tools_fixture()
        metadata = root / "version.txt"
        metadata.write_text(metadata.read_text().replace("apiVersion     : 26", "apiVersion: 25"))
        self.save(root / "ohpm/package.json", {"name": "@ohos/ohpm", "version": "26.0.0.629"})
        sdk_path = self.sdk / "openharmony/native/oh-uni-package.json"
        component = json.loads(sdk_path.read_text())
        component["platformVersion"] = "25.0.0"
        self.save(sdk_path, component)
        result = self.inspect_command_line_tools(root)
        self.assertTrue({"tool_metadata_conflict", "tool_component_mismatch", "tool_sdk_mismatch"} <= self.codes(result))

    def test_command_line_tools_missing_entry_does_not_fall_back_to_deveco(self):
        root = self.command_line_tools_fixture()
        (root / "tool/node/bin/node").unlink()
        result = self.inspect_command_line_tools(root)
        self.assertIn("tool_probe_failed", self.codes(result))
        self.assertEqual(result["tools"]["node"]["state"], "unavailable")
        self.assertTrue((self.deveco / "tools/node/bin/node").is_file())

    def test_duplicate_sdk_json_fields_cannot_hide_nonrelease_metadata(self):
        root = self.command_line_tools_fixture()
        paths = [("sdk", self.sdk / "sdk-pkg.json")]
        paths.extend((name, self.sdk / "openharmony" / name / "oh-uni-package.json")
                     for name in preflight.COMPONENTS)
        for component, path in paths:
            original = path.read_text()
            conflict = original.replace('"releaseType": "Release"',
                                        '"releaseType": "Beta1", "releaseType": "Release"')
            cases = [("releaseType", conflict)]
            if component == "sdk":
                # Also reject a duplicate outer data object, not only nested fields.
                cases.append(("data", '{"data": {"releaseType": "Beta1"}, ' + original[1:]))
            for field, text in cases:
                with self.subTest(component=component, field=field):
                    path.write_text(text)
                    result = self.inspect_command_line_tools(root, target="fixture:5557")
                    self.assertFalse(result["environment_checks_passed"])
                    self.assertFalse(result["sdk"]["is_formal"])
                    failures = [item for item in result["errors"]
                                if item["code"] == "sdk_metadata_invalid" and item["component"] == component]
                    self.assertEqual(len(failures), 1)
                    self.assertIn(f"duplicate JSON metadata field: {field}", failures[0]["message"])
                path.write_text(original)

    def test_duplicate_package_json_versions_cannot_hide_conflicts(self):
        root = self.command_line_tools_fixture()
        for name, relative in (("@ohos/hvigor", "hvigor/hvigor/package.json"),
                               ("@ohos/hvigor-ohos-plugin", "hvigor/hvigor-ohos-plugin/package.json"),
                               ("@ohos/ohpm", "ohpm/package.json")):
            with self.subTest(package=name):
                path = root / relative
                original = path.read_text()
                path.write_text(original.replace('"version":', '"version": "99.0.0.999", "version":'))
                result = self.inspect_command_line_tools(root, target="fixture:5557")
                self.assertFalse(result["environment_checks_passed"])
                self.assertNotIn("version", result["tools"][name])
                failures = [item for item in result["errors"]
                            if item["code"] == "tool_metadata_invalid" and item["component"] == name]
                self.assertEqual(len(failures), 1)
                self.assertIn("duplicate JSON metadata field: version", failures[0]["message"])
                path.write_text(original)

    def test_layout_selection_is_exclusive_in_python_and_cli(self):
        root = self.command_line_tools_fixture()
        for deveco, command_line_tools in ((None, None), (self.deveco, root)):
            with self.subTest(deveco=deveco), self.assertRaisesRegex(ValueError, "exactly one"):
                preflight.inspect_environment(deveco, self.sdk, None, command_line_tools=command_line_tools)
        for layouts in ([], ["--deveco", str(self.deveco), "--command-line-tools", str(root)]):
            result = subprocess.run([sys.executable, str(ROOT / "scripts/check_arkts_environment.py"),
                                     "--sdk", str(self.sdk), *layouts], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")

    def test_command_line_tools_cli_reports_missing_target_without_misidentifying_layout(self):
        root = self.command_line_tools_fixture()
        result = subprocess.run([sys.executable, str(ROOT / "scripts/check_arkts_environment.py"),
                                 "--command-line-tools", str(root), "--sdk", str(self.sdk)],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(self.codes(report), {"target_not_checked"})
        self.assertNotIn("deveco", report["tools"])
        self.assertEqual(report["tools"]["command_line_tools"]["version"], "26.0.0.851")


if __name__ == "__main__":
    unittest.main()
