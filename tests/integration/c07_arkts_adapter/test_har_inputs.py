"""Focused Package-boundary tests; synthetic archive fixtures are not SDK evidence."""

import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from adapters.artifacts.arkts import har_inputs
from adapters.toolchains.common.documents import ref
from adapters.artifacts.arkts.har import read_members


def archive(files):
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode="w:gz") as tar:
        parents = set()
        for name in files:
            parents.update(str(p) for p in Path(name).parents if str(p) != ".")
        for name in sorted(parents):
            t = tarfile.TarInfo(name)
            t.type = tarfile.DIRTYPE
            t.mode = 0o755
            tar.addfile(t)
        for name, data in files.items():
            t = tarfile.TarInfo(name)
            t.mode = 0o644
            t.size = len(data)
            t.mtime = 12345
            tar.addfile(t, io.BytesIO(data))
    return out.getvalue()


class HarInputs(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "build").mkdir()
        (self.root / "generated").mkdir()
        (self.root / "build/libx.so").write_bytes(b"test-only-native-bytes")
        (self.root / "generated/index.d.ts").write_bytes(b"export declare function f(): void;")
        self.native = {"package/libs/arm64-v8a/libx.so": ref(self.root, "build/libx.so")}
        self.declarations = {"package/index.d.ts": ref(self.root, "generated/index.d.ts")}
        self.files = {
            "package/oh-package.json5": b'{"name":"test"}',
            "package/libs/arm64-v8a/libx.so": (self.root / "build/libx.so").read_bytes(),
            "package/index.d.ts": (self.root / "generated/index.d.ts").read_bytes(),
        }
        self.raw = archive(self.files)

    def bind(self, raw=None, native=None, declarations=None):
        return har_inputs.bind_har(
            self.raw if raw is None else raw,
            self.root,
            self.native if native is None else native,
            self.declarations if declarations is None else declarations,
        )

    def test_keeps_members_and_normalizes(self):
        result = self.bind()
        self.assertEqual(read_members(result), read_members(self.raw))
        self.assertEqual(result, self.bind(result))

    def test_native_drift_before_normalize(self):
        (self.root / "build/libx.so").write_bytes(b"changed")
        with patch.object(har_inputs, "canonicalize", side_effect=AssertionError("must not reach")):
            with self.assertRaisesRegex(ValueError, "frozen package input changed"):
                self.bind()

    def test_source_drift_during_normalize(self):
        original = har_inputs.canonicalize

        def changed(data):
            result = original(data)
            (self.root / "build/libx.so").write_bytes(b"changed")
            return result

        with patch.object(har_inputs, "canonicalize", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "frozen package input changed"):
                self.bind()

    def test_har_native_transformation_rejected(self):
        files = dict(self.files)
        files["package/libs/arm64-v8a/libx.so"] = b"silently-stripped"
        with self.assertRaisesRegex(ValueError, "member differs"):
            self.bind(archive(files))

    def test_native_closure_extra_and_missing(self):
        for extra in (True, False):
            with self.subTest(extra=extra):
                files = dict(self.files)
                if extra:
                    files["package/libs/arm64-v8a/libunexpected.so"] = b"extra"
                else:
                    del files["package/libs/arm64-v8a/libx.so"]
                with self.assertRaisesRegex(ValueError, "library closure"):
                    self.bind(archive(files))

    def test_native_extension_or_location_cannot_hide_payload(self):
        for name, data in [
            ("package/libs/arm64-v8a/libhidden.so.1", b"versioned"),
            ("package/libs/arm64-v8a/hidden", b"unknown-native"),
            ("package/resources/hidden.data", b"\x7fELFpayload"),
        ]:
            with self.subTest(name=name):
                files = dict(self.files)
                files[name] = data
                with self.assertRaisesRegex(ValueError, "library closure"):
                    self.bind(archive(files))

    def test_ordinary_resource_remains_permitted(self):
        files = dict(self.files)
        files["package/resources/readme.so.1"] = b"plain documentation"
        result = self.bind(archive(files))
        self.assertEqual(read_members(result), read_members(archive(files)))

    def test_declaration_change_rejected(self):
        files = dict(self.files)
        files["package/index.d.ts"] = b"export declare function other(): void;"
        with self.assertRaisesRegex(ValueError, "member differs"):
            self.bind(archive(files))

    def test_bad_reference_and_escaping_paths(self):
        for change in (
            {"path": "../escape"},
            {"path": "/absolute"},
            {"size": True},
            {"sha256": "0" * 64},
        ):
            with self.subTest(change=change):
                r = dict(next(iter(self.native.values())), **change)
                with self.assertRaises(ValueError):
                    self.bind(native={next(iter(self.native)): r})

    def test_symlink_chain_rejected(self):
        (self.root / "alias").symlink_to(self.root / "build", target_is_directory=True)
        r = dict(next(iter(self.native.values())), path="alias/libx.so")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.bind(native={next(iter(self.native)): r})

    def test_native_cannot_hide_as_passthrough(self):
        with self.assertRaises(ValueError):
            self.bind(declarations=self.native)
        with self.assertRaises(ValueError):
            self.bind(native={})

    def test_input_root_and_bindings_are_explicit(self):
        for declarations in ([], 0, "bad"):
            with self.subTest(declarations=declarations):
                with self.assertRaises(ValueError):
                    har_inputs.bind_har(self.raw, self.root, self.native, declarations)


if __name__ == "__main__":
    unittest.main()
