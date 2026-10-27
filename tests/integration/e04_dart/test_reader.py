"""Actual public-header reader checks; no private SDK implementation is loaded."""
from copy import deepcopy
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "adapters/generators/dart_ffi"))
import reader
import wire


class ReaderCases(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Test inputs are supplied explicitly, never silently searched on the
        # author's machine. The candidate is a public package, not SDK source.
        cls.candidate = Path(sys.argv_candidate).resolve(strict=True)
        cls.clang = Path(sys.argv_clang).resolve(strict=True)
        cls.sdk = Path(sys.argv_sdk).resolve(strict=True)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="madb-e04-reader-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        shutil.copytree(self.candidate, self.root / "provider")
        self.work = self.root / "work"
        self.work.mkdir()
        self.config = {"tools": {"clang": {
            "path": str(self.clang), "id": "e04-clang", "version": subprocess.check_output([str(self.clang), "--version"], text=True).splitlines()[0],
            "sha256": wire.digest(self.clang.read_bytes()), "size": self.clang.stat().st_size,
            "arguments": ["-isysroot", str(self.sdk)],
        }}}
        deps = subprocess.check_output([str(self.clang), "-x", "c", "-std=c11", "-isysroot", str(self.sdk), "-M", "-MT", "declarations", str(self.root / reader.CLIENT)], text=True)
        self.config["compiler_headers"] = []
        for name in shlex.split(deps.replace("\\\n", " ").split(":", 1)[1]):
            path = Path(name).resolve(strict=True)
            if not path.is_relative_to(self.root):
                self.config["compiler_headers"].append({"path": str(path), "sha256": wire.digest(path.read_bytes()), "size": path.stat().st_size})
        profile = {"schema": "madb.profile/2", "id": reader.PROFILE,
                   "required_features": [*reader.FEATURES, "core.extension-composition@1"]}
        (self.root / "profile.json").write_text(wire.canonical(profile))
        (self.root / "lock.json").write_text("{}")
        self.request = {"schema": "madb.read-request/1", "boundary_id": reader.BOUNDARY,
                        "profile": wire.reference(self.root, "profile.json", "madb.profile/2"),
                        "lock": wire.reference(self.root, "lock.json", "madb.lock/1"),
                        "selector": {"profile_id": reader.PROFILE, "profile_digest": "sha256:" + "1" * 64,
                                     "route": reader.ROUTE, "direction": "consumer-to-provider", "target": "aarch64-apple-darwin"},
                        "inputs": []}
        self.repin()

    def repin(self):
        self.request["inputs"] = [wire.reference(self.root, p.relative_to(self.root).as_posix(), "madb.source-input/1")
                                  for p in sorted((self.root / "provider").rglob("*")) if p.is_file()]

    def seed(self):
        return reader.seed(self.request, self.config, self.root, self.work)

    def test_real_sync_view_preserves_native_identity_and_uses_header_locations(self):
        old = (self.root / "provider/identity.json").read_bytes()
        result = self.seed()
        self.assertEqual(6, len(result["symbols"]))
        self.assertEqual(set(reader.FEATURES), set(result["required_features"]))
        self.assertTrue(all(s["location"]["path"] == reader.HEADER for s in result["symbols"]))
        self.assertTrue(all(s["source_key"]["module"] == reader.HEADER for s in result["symbols"]))
        self.assertNotIn("state_callback", [t["id"] for t in result["types"]])
        self.assertEqual(old, (self.root / "provider/identity.json").read_bytes())

    def test_declared_callback_request_is_rejected(self):
        profile = wire.read_json(self.root / "profile.json")
        profile["required_features"].append("core.callbacks@1")
        (self.root / "profile.json").write_text(wire.canonical(profile))
        self.request["profile"] = wire.reference(self.root, "profile.json", "madb.profile/2")
        with self.assertRaisesRegex(ValueError, "unsupported requested feature"):
            self.seed()

    def test_repin_changed_c_declaration_is_not_satisfied_by_old_contract(self):
        p = self.root / reader.HEADER
        p.write_text(p.read_text().replace("uint64_t *out_sum", "uint32_t *out_sum"))
        self.repin()
        with self.assertRaisesRegex(ValueError, "C declaration differs"):
            self.seed()

    def test_repin_alias_width_change_rejects_even_same_signature_spelling(self):
        p = self.root / reader.HEADER
        p.write_text(p.read_text().replace("typedef uint64_t MadbHandle;", "typedef uint32_t MadbHandle;"))
        self.repin()
        with self.assertRaisesRegex(ValueError, "finite Dart ABI"):
            self.seed()

    def test_missing_actual_compiler_header_is_rejected(self):
        self.config["compiler_headers"] = []
        with self.assertRaisesRegex(ValueError, "unrecorded header"):
            self.seed()

    def test_wrong_original_semantics_cannot_be_fixed_by_repin(self):
        p = self.root / "provider/contract.json"
        value = wire.read_json(p)
        next(t for t in value["types"] if t["id"] == "bytes")["definition"]["max_bytes"] = 65537
        p.write_text(wire.canonical(value))
        self.repin()
        with self.assertRaisesRegex(ValueError, "synchronous type semantics"):
            self.seed()

    def test_changed_input_bytes_reject_before_parser(self):
        p = self.root / reader.HEADER
        p.write_bytes(p.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "input byte identity differs"):
            self.seed()


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--clang", required=True)
    parser.add_argument("--sdk", required=True)
    args, remaining = parser.parse_known_args()
    sys.argv_candidate, sys.argv_clang, sys.argv_sdk = args.candidate, args.clang, args.sdk
    unittest.main(argv=[sys.argv[0], *remaining])
