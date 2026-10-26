"""Public-candidate trust must fail before a native library is loaded."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("observation_check", ROOT / "scripts/check_swift_observation.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class CandidateAdmissionTests(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / "target/l04-candidate-unit"
        scratch.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.identity = {"schema": "madb.native-identity/1", "bootstrap_version": 1,
                         "profile_digest": "sha256:" + "1" * 64, "interface_digest": "sha256:" + "2" * 64}
        self.write("identity.json", json.dumps(self.identity).encode())
        self.write("toolchain.json", b"{}")
        self.write("framework/MadBSkie.framework/MadBSkie", b"synthetic non-executable test fixture")
        self.write("swift/MadBSkieConcurrency.swift", b"// synthetic test fixture")
        self.manifest = {"schema": "madb.candidate/1", "manifest_path": "candidate.json", "files": [],
                         "selector": {"route": "KN-SWIFT-OBJC", "target": "aarch64-apple-darwin",
                                      "direction": "consumer-to-provider", "profile_digest": self.identity["profile_digest"]},
                         "dependencies": [], "build_evidence": [], "public_symbols": [], "required_cases": []}
        self.seal()

    def write(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def seal(self):
        self.manifest["files"] = [{"path": path.relative_to(self.root).as_posix(),
                                   "sha256": check.digest(path.read_bytes()), "size": path.stat().st_size}
                                  for path in sorted(self.root.rglob("*")) if path.is_file() and path.name != "candidate.json"]
        self.trusted = check.candidate_digest(self.manifest)
        self.manifest["candidate_digest"] = self.trusted
        self.write("candidate.json", json.dumps(self.manifest).encode())

    def test_complete_pinned_inventory_is_admitted(self):
        result = check.verify_candidate(self.root, self.trusted)
        self.assertEqual(result["verified_files"], 4)
        self.assertEqual(result["identity"], self.identity)

    def test_changed_artifact_rejected_under_trusted_manifest(self):
        self.write("swift/MadBSkieConcurrency.swift", b"tampered facade")
        with self.assertRaisesRegex(ValueError, "file bytes differ"):
            check.verify_candidate(self.root, self.trusted)

    def test_self_consistent_replacement_is_not_independently_trusted(self):
        trusted = self.trusted
        self.write("swift/MadBSkieConcurrency.swift", b"replacement facade")
        self.seal()
        with self.assertRaisesRegex(ValueError, "explicitly trusted"):
            check.verify_candidate(self.root, trusted)

    def test_identity_must_match_selected_profile(self):
        self.identity["profile_digest"] = "sha256:" + "3" * 64
        self.write("identity.json", json.dumps(self.identity).encode())
        self.seal()
        with self.assertRaisesRegex(ValueError, "selected profile"):
            check.verify_candidate(self.root, self.trusted)

    def test_unlisted_file_rejected(self):
        self.write("unexpected.dylib", b"extra")
        with self.assertRaisesRegex(ValueError, "inventory differs"):
            check.verify_candidate(self.root, self.trusted)

    def test_symlink_rejected(self):
        (self.root / "alias").symlink_to("identity.json")
        with self.assertRaisesRegex(ValueError, "symlink"):
            check.verify_candidate(self.root, self.trusted)

    def test_parent_path_rejected_even_with_matching_manifest_digest(self):
        self.manifest["files"][0]["path"] = "../escape"
        trusted = check.candidate_digest(self.manifest)
        self.manifest["candidate_digest"] = trusted
        self.write("candidate.json", json.dumps(self.manifest).encode())
        with self.assertRaisesRegex(ValueError, "candidate path"):
            check.verify_candidate(self.root, trusted)

    def test_missing_candidate_is_an_error(self):
        with self.assertRaises(FileNotFoundError):
            check.verify_candidate(self.root / "missing", self.trusted)


if __name__ == "__main__":
    unittest.main()
