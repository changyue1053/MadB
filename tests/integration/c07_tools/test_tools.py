"""Real ZIP/tree mismatch tests; synthetic tool bytes are never executed."""
from pathlib import Path
import copy
import hashlib
import json
import os
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
import warnings
import zipfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts import tools


class ToolInventoryTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.root = self.directory / "installed"
        self.root.mkdir(mode=0o755)
        self.archive = self.directory / "reviewed.zip"
        self.marker = self.directory / "launcher-executed"
        for name, relative in tools.LAUNCHERS.items():
            if name != "clang":
                self.write(relative, f"#!/bin/sh\ntouch '{self.marker}'\n", mode=0o755)
        self.write("sdk/default/openharmony/native/llvm/bin/clang-15", "fixture compiler image", mode=0o755)
        (self.root / tools.LAUNCHERS["clang"]).symlink_to("clang-15")
        self.write("version.txt", """# Command Line Tools(mac-arm64)
# Version: 26.0.0.851
releaseType: Release
hvigor: 6.26.8
ohpm: 26.0.0.630
HarmonyOS SDK: HarmonyOS 6.0.0 Release (include Ohos_sdk_public 26.0.0.105 (API Version 26 Release))
apiVersion: 26
platformVersion: 6.0.0
""")
        sdk = {"version": "26.0.0.105", "apiVersion": "26", "releaseType": "Release", "platformVersion": "6.0.0"}
        self.write("sdk/default/sdk-pkg.json", json.dumps({"data": sdk}))
        for component in ("ets", "native", "toolchains"):
            self.write(f"sdk/default/openharmony/{component}/oh-uni-package.json", json.dumps(sdk))
        for name, relative, version in (
            ("@ohos/hvigor", "hvigor/hvigor/package.json", "6.26.8"),
            ("@ohos/hvigor-ohos-plugin", "hvigor/hvigor-ohos-plugin/package.json", "6.26.8"),
            ("@ohos/ohpm", "ohpm/package.json", "26.0.0.630"),
        ):
            self.write(relative, json.dumps({"name": name, "version": version}))
        self.pack()

    def write(self, name, data, mode=0o644):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(data)
        path.chmod(mode)

    def entry(self, zipped, name, data=b"", mode=stat.S_IFREG | 0o644, compression=zipfile.ZIP_DEFLATED):
        info = zipfile.ZipInfo(name)
        info.create_system = 3
        info.external_attr = mode << 16
        info.compress_type = compression
        zipped.writestr(info, data)

    def pack(self):
        with zipfile.ZipFile(self.archive, "w") as zipped:
            for path in [self.root, *sorted(self.root.rglob("*"))]:
                name = "command-line-tools" + ("/" + path.relative_to(self.root).as_posix() if path != self.root else "")
                mode = path.lstat().st_mode
                data = os.readlink(path).encode() if path.is_symlink() else (b"" if path.is_dir() else path.read_bytes())
                self.entry(zipped, name + ("/" if stat.S_ISDIR(mode) else ""), data, mode)

    def append(self, name, data=b"extra", mode=stat.S_IFREG | 0o644, compression=zipfile.ZIP_DEFLATED):
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(self.archive, "a") as zipped:
                self.entry(zipped, name, data, mode, compression)

    def acquire(self):
        return tools.acquire_inventory(self.archive, expected_sha256=hashlib.sha256(self.archive.read_bytes()).hexdigest(),
                                       expected_size=self.archive.stat().st_size)

    def test_verified_tree_is_bound_to_archive_without_executing_tools_or_certifying(self):
        inventory = self.acquire()
        result = tools.verify_inventory(inventory, self.root)
        self.assertEqual(result["entries_sha256"], inventory["entries_sha256"])
        self.assertEqual(result["archive"]["sha256"], hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.assertEqual(result["metadata"]["command_line_tools"]["version"], "26.0.0.851")
        self.assertEqual(result["launchers"]["clang"]["link_target"], "clang-15")
        self.assertEqual(result["cpf"]["state"], "incomplete")
        self.assertFalse(result["certifies_c07"])
        self.assertFalse(result["archive"]["official_checksum_or_signature_verified"])
        self.assertEqual(result["archive"]["checksum_source"],
                         "caller-reviewed expected checksum; locally computed from the user-supplied archive")
        self.assertFalse(self.marker.exists())
        self.assertEqual(tools.verify_inventory(inventory, self.root), result)

    def test_modified_file_is_rejected_without_executing_it(self):
        inventory = self.acquire()
        self.write(tools.LAUNCHERS["node"], "different executable", mode=0o755)
        with self.assertRaisesRegex(ValueError, "changed: tool/node/bin/node"):
            tools.verify_inventory(inventory, self.root)
        self.assertFalse(self.marker.exists())

    def test_missing_file_is_rejected(self):
        inventory = self.acquire()
        (self.root / "version.txt").unlink()
        with self.assertRaisesRegex(ValueError, "missing: version.txt"):
            tools.verify_inventory(inventory, self.root)

    def test_extra_executable_and_empty_directory_are_rejected(self):
        inventory = self.acquire()
        self.write("extra-executable", "extra", mode=0o755)
        (self.root / "new-cache").mkdir()
        with self.assertRaises(ValueError) as raised:
            tools.verify_inventory(inventory, self.root)
        self.assertIn("extra: extra-executable", str(raised.exception))
        self.assertIn("extra: new-cache", str(raised.exception))

    def test_file_permission_change_is_rejected(self):
        inventory = self.acquire()
        (self.root / tools.LAUNCHERS["node"]).chmod(0o644)
        with self.assertRaisesRegex(ValueError, "changed: tool/node/bin/node"):
            tools.verify_inventory(inventory, self.root)

    def test_changed_link_escape_or_same_bytes_regular_file_are_rejected(self):
        inventory = self.acquire()
        link = self.root / tools.LAUNCHERS["clang"]
        data = link.read_bytes()
        link.unlink()
        link.symlink_to("/outside/clang")
        with self.assertRaisesRegex(ValueError, "changed: .*clang"):
            tools.verify_inventory(inventory, self.root)
        link.unlink()
        link.write_bytes(data)
        link.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "changed: .*clang"):
            tools.verify_inventory(inventory, self.root)

    def test_installed_directory_replaced_by_link_is_not_traversed(self):
        inventory = self.acquire()
        directory = self.root / "ohpm"
        outside = self.directory / "outside"
        directory.rename(outside)
        directory.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "changed: ohpm"):
            tools.verify_inventory(inventory, self.root)
        self.assertTrue((outside / "bin/ohpm").is_file())

    def test_archive_checksum_and_size_must_match_explicit_review(self):
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            tools.acquire_inventory(self.archive, expected_sha256="0" * 64, expected_size=self.archive.stat().st_size)
        with self.assertRaisesRegex(ValueError, "size differs"):
            tools.acquire_inventory(self.archive, expected_sha256="0" * 64, expected_size=1)

    def test_archive_and_installation_root_symlinks_are_rejected(self):
        inventory = self.acquire()
        link = self.directory / "link"
        link.symlink_to(self.root)
        with self.assertRaisesRegex(ValueError, "root must be a directory, not a symlink"):
            tools.verify_inventory(inventory, link)
        link.unlink()
        link.symlink_to(self.archive)
        with self.assertRaisesRegex(ValueError, "archive must be a regular file"):
            tools.acquire_inventory(link, expected_sha256="0" * 64, expected_size=1)

    def test_duplicate_and_case_colliding_members_are_rejected(self):
        for name in ("command-line-tools/version.txt", "command-line-tools/VERSION.TXT"):
            with self.subTest(name=name):
                self.pack()
                self.append(name)
                with self.assertRaisesRegex(ValueError, "duplicate or case/Unicode"):
                    self.acquire()

    def test_unsafe_archive_names_are_rejected(self):
        for name in ("/escape", "command-line-tools/../escape", "command-line-tools//escape",
                     "command-line-tools/./escape", "command-line-tools/evil\\name", "C:/escape"):
            with self.subTest(name=name):
                self.pack()
                self.append(name)
                with self.assertRaisesRegex(ValueError, "unsafe member path"):
                    self.acquire()

    def test_escaping_cyclic_and_dangling_archive_links_are_rejected(self):
        for target, message in (("../escape", "symlink escapes"), ("/escape", "unsafe symlink"),
                                ("link", "cyclic"), ("missing", "must end at an inventoried")):
            with self.subTest(target=target):
                self.pack()
                self.append("command-line-tools/link", target.encode(), stat.S_IFLNK | 0o777)
                with self.assertRaisesRegex(ValueError, message):
                    self.acquire()

    def test_members_below_symlink_or_absent_directory_are_rejected(self):
        for parent in ("absent", "alias"):
            with self.subTest(parent=parent):
                self.pack()
                if parent == "alias":
                    self.append("command-line-tools/alias", b"version.txt", stat.S_IFLNK | 0o777)
                self.append(f"command-line-tools/{parent}/child")
                with self.assertRaisesRegex(ValueError, "missing or non-directory archive parent"):
                    self.acquire()

    def test_link_targets_cannot_fold_away_invalid_traversed_components(self):
        for middle in ("version.txt", "file-link", "missing"):
            with self.subTest(middle=middle):
                self.pack()
                if middle == "file-link":
                    self.append("command-line-tools/file-link", b"version.txt", stat.S_IFLNK | 0o777)
                self.append("command-line-tools/nonlauncher-link", (middle + "/../version.txt").encode(), stat.S_IFLNK | 0o777)
                with self.assertRaisesRegex(ValueError, "symlink target traverses missing/non-directory"):
                    self.acquire()

    def test_all_safe_nonlauncher_links_are_resolved_against_the_closed_tree(self):
        (self.root / "nonlauncher-link").symlink_to("hvigor/../version.txt")
        (self.root / "chain-link").symlink_to("nonlauncher-link")
        self.pack()
        inventory = self.acquire()
        result = tools.verify_inventory(inventory, self.root)
        self.assertEqual(result["entry_count"], len(inventory["entries"]))
        self.assertEqual((self.root / "chain-link").read_bytes(), (self.root / "version.txt").read_bytes())
        self.assertFalse(self.marker.exists())

    def test_special_files_compression_and_limits_are_rejected(self):
        for mode, compression in ((stat.S_IFIFO | 0o644, zipfile.ZIP_STORED),
                                  (stat.S_IFREG | 0o644, zipfile.ZIP_BZIP2),
                                  (stat.S_IFREG | 0o4755, zipfile.ZIP_STORED)):
            with self.subTest(mode=mode, compression=compression):
                self.pack()
                self.append("command-line-tools/special", mode=mode, compression=compression)
                with self.assertRaisesRegex(ValueError, "unsupported ZIP member"):
                    self.acquire()
        self.pack()
        with patch.object(tools, "MAX_TOTAL_SIZE", 1):
            with self.assertRaisesRegex(ValueError, "bounded inventory limits"):
                self.acquire()

    def test_strict_duplicate_sdk_and_package_metadata_rejected_even_when_archive_matches(self):
        for name in ("sdk/default/openharmony/native/oh-uni-package.json", "ohpm/package.json"):
            with self.subTest(name=name):
                original = (self.root / name).read_text()
                self.write(name, original[:-1] + ',"version":"26.0.0.630"}')
                self.pack()
                with self.assertRaisesRegex(ValueError, "duplicate JSON metadata field"):
                    tools.verify_inventory(self.acquire(), self.root)
                self.write(name, original)

    def test_beta_and_conflicting_tool_metadata_are_rejected_when_archive_matches(self):
        path = self.root / "version.txt"
        original = path.read_text()
        for content, message in ((original.replace("mac-arm64", "mac-x64"), "tool_platform_unsupported"),
                                 (original.replace("releaseType: Release", "releaseType: Beta1"), "tool_not_release"),
                                 (original.replace("hvigor: 6.26.8", "hvigor: 6.26.9"), "tool_component_mismatch")):
            with self.subTest(message=message):
                path.write_text(content)
                self.pack()
                with self.assertRaisesRegex(ValueError, message):
                    tools.verify_inventory(self.acquire(), self.root)

    def test_mac_metadata_exclusions_are_exact(self):
        self.append("__MACOSX/", b"", stat.S_IFDIR | 0o755)
        self.append("__MACOSX/command-line-tools/", b"", stat.S_IFDIR | 0o755)
        self.append("__MACOSX/command-line-tools/._version.txt", bytes.fromhex("0005160700020000") + b"sidecar")
        self.append("command-line-tools/.DS_Store", b"metadata")
        inventory = self.acquire()
        self.write(".DS_Store", "local Finder metadata")
        result = tools.verify_inventory(inventory, self.root)
        self.assertEqual(result["ignored_installed_metadata"], [".DS_Store"])
        self.write("._real-executable", "unexpected", mode=0o755)
        with self.assertRaisesRegex(ValueError, "extra: ._real-executable"):
            tools.verify_inventory(inventory, self.root)

    def test_exclusion_names_cannot_hide_executables_links_or_arbitrary_mac_files(self):
        for name, data, mode in (("command-line-tools/.DS_Store", b"script", stat.S_IFREG | 0o755),
                                 ("command-line-tools/.DS_Store", b"version.txt", stat.S_IFLNK | 0o777),
                                 ("__MACOSX/command-line-tools/arbitrary", b"payload", stat.S_IFREG | 0o644),
                                 ("__MACOSX/command-line-tools/._version.txt", b"not AppleDouble", stat.S_IFREG | 0o644)):
            with self.subTest(name=name, mode=mode):
                self.pack()
                self.append(name, data, mode)
                with self.assertRaisesRegex(ValueError, "exclusion|AppleDouble"):
                    self.acquire()

    def test_inventory_change_cannot_silently_repin_current_tree(self):
        inventory = copy.deepcopy(self.acquire())
        next(row for row in inventory["entries"] if row["path"] == "version.txt")["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "invalid or changed acquisition inventory"):
            tools.verify_inventory(inventory, self.root)


if __name__ == "__main__":
    unittest.main()
