"""P01 process-level handshake acceptance. Full lifecycle executor belongs to C04/E02."""
from pathlib import Path
import json
import subprocess
import sys
import unittest

from jsonschema import Draft202012Validator, ValidationError

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/contract"))
from test_formats import strict_decode

SCHEMA = Draft202012Validator(json.loads((ROOT / "docs/schemas/extension-wire.schema.json").read_text()))
HELLO = {"schema": "madb.extension-wire/1", "session_id": "p01-session", "kind": "hello",
         "extension_id": "madb.p01-peer", "extension_version": "0.1.0-probe", "component_id": "reader",
         "manifest_digest": "sha256:" + "1" * 64, "protocol_versions": ["1.0"], "ir_versions": ["1.0"],
         "required_features": ["core.call.sync@1"]}


def handshake(mode="ok", **changes):
    """Bounded controlled-peer experiment, not an untrusted-extension sandbox."""
    hello = {**HELLO, **changes}
    SCHEMA.validate(hello)
    try:
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("helper.py")), mode],
                                input=(json.dumps(hello) + "\n").encode(), capture_output=True,
                                timeout=0.5 if mode == "timeout" else 5, check=False)
    except subprocess.TimeoutExpired:
        return "ToolTimeout"
    if result.returncode:
        return f"ToolFailed:{result.returncode}"
    # subprocess.run is bounded here by trusted finite-size fixtures. Production
    # C04 must stream-cap stdout/stderr and terminate/reap the whole process tree.
    if len(result.stdout) > 1024 * 1024 or result.stdout.count(b"\n") != 1:
        return "ProtocolInvalid"
    try:
        frame = strict_decode(result.stdout.decode("utf-8"))
        SCHEMA.validate(frame)
    except (ValueError, UnicodeError, ValidationError):
        return "ProtocolInvalid"
    if frame["session_id"] != hello["session_id"]:
        return "ProtocolInvalid"
    if frame["kind"] == "reject":
        return frame["error"]["code"]
    if frame["kind"] != "hello_ack":
        return "ProtocolInvalid"
    if any(frame[key] != hello[key] for key in ("extension_id", "extension_version", "component_id", "manifest_digest")):
        return "ExtensionIdentityMismatch"
    if frame["protocol"] not in hello["protocol_versions"] or frame["ir"] not in hello["ir_versions"]:
        return "ProtocolIncompatible"
    if set(frame["features"]) - {"core.call.sync@1"} or set(hello["required_features"]) - set(frame["features"]):
        return "CapabilityUnsupported"
    return "ok"


class HandshakeProbe(unittest.TestCase):
    def test_real_peer_accepts_the_locked_handshake(self):
        self.assertEqual(handshake(), "ok")

    def test_unshared_protocol_and_ir_are_rejected_independently(self):
        for field in ("protocol_versions", "ir_versions"):
            with self.subTest(field=field):
                self.assertEqual(handshake(**{field: ["2.0"]}), "ProtocolIncompatible")

    def test_missing_required_feature_is_rejected(self):
        self.assertEqual(handshake(required_features=["core.unknown@1"]), "CapabilityUnsupported")

    def test_wrong_identity_and_capability_claims_are_rejected(self):
        for mode, expected in [("wrong-identity", "ExtensionIdentityMismatch"), ("wrong-session", "ProtocolInvalid"),
                               ("unoffered-version", "ProtocolIncompatible"), ("extra-feature", "CapabilityUnsupported")]:
            with self.subTest(mode=mode):
                self.assertEqual(handshake(mode), expected)

    def test_controlled_helper_error_crash_and_timeout_stay_distinct(self):
        for mode, expected in [("tool-error", "ToolFailed"), ("crash", "ToolFailed:23"), ("timeout", "ToolTimeout")]:
            with self.subTest(mode=mode):
                self.assertEqual(handshake(mode), expected)

    def test_malformed_stdout_is_not_accepted_as_success(self):
        for mode in ("noise", "duplicate-key", "duplicate-frame", "oversize", "invalid-utf8"):
            with self.subTest(mode=mode):
                self.assertEqual(handshake(mode), "ProtocolInvalid")


if __name__ == "__main__":
    unittest.main()
