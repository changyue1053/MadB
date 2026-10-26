"""Real E02 dialogue tests using the public Rust helper and CLI composition.

Build both examples first. The peer's BuildEvidence is controlled fixture data;
this suite certifies protocol mechanics, never a compiler or SDK route.
"""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))


class ExtensionSessions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.driver = TARGET / "debug/examples/extension_session_probe"
        cls.peer = TARGET / "debug/examples/extension_peer"
        cls.chain = TARGET / "debug/examples/extension_chain_probe"
        if not all(path.is_file() for path in (cls.driver, cls.peer, cls.chain)):
            raise RuntimeError("build the three E02 examples before running this suite")
        cls.output = Path(tempfile.mkdtemp(prefix="madb-e02-sessions-"))
        cls.counter = 0

    def run_mode(self, mode, *, peer=None, timeout=40):
        type(self).counter += 1
        case_root = self.output / f"{type(self).counter:02}-{mode}"
        result = subprocess.run([str(self.driver), str(peer or self.peer), mode, str(case_root)],
                                capture_output=True, text=True, timeout=timeout, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        data = json.loads(result.stdout)
        (self.output / f"{type(self).counter:02}-{mode}.json").write_text(json.dumps(data, indent=2))
        if "cleanup" in data:
            self.assertTrue(data["cleanup"]["complete"], data)
            self.assertFalse(data["pending_cleanup"], data)
        return data

    def rejected(self, mode, code=None):
        data = self.run_mode(mode)
        self.assertNotIn("error", data, data)
        self.assertFalse(data["admitted"], data)
        if code:
            self.assertEqual(data["diagnostic"]["code"], code, data)
        return data

    def test_real_ack_precedes_invoke_and_capture_is_admitted_after_clean_exit(self):
        data = self.run_mode("ok")
        self.assertTrue(data["admitted"], data)
        self.assertEqual(data["files"], ["build.log", "fixture.o", "result.json"])
        self.assertEqual(data["cancel_after_finish"], "AlreadyFinished")
        self.assertEqual([json.loads(s)["kind"] for s in data["stdout"].splitlines()],
                         ["hello_ack", "result"])

    def test_malformed_peer_is_rejected_without_repair(self):
        for mode in ["noise", "utf8", "oversize", "duplicate-key", "early-result"]:
            with self.subTest(mode=mode):
                self.rejected(mode, "ProtocolInvalid")

    def test_host_independently_checks_identity_versions_and_features(self):
        for mode, code in [("wrong-identity", "ExtensionIdentityMismatch"),
                           ("wrong-session", "ProtocolInvalid"),
                           ("wrong-version", "ProtocolIncompatible"),
                           ("extra-feature", "CapabilityUnsupported")]:
            with self.subTest(mode=mode):
                self.rejected(mode, code)
        self.assertIn("ExtensionIdentityMismatch", self.run_mode("wrong-recipe")["error"])

    def test_request_identity_and_single_terminal_are_enforced(self):
        for mode in ["wrong-request", "wrong-result-version", "duplicate-result"]:
            with self.subTest(mode=mode):
                self.rejected(mode, "ProtocolInvalid")

    def test_crash_eof_and_timeout_remain_distinct(self):
        for mode in ["crash-before-ack", "crash", "crash-after-result"]:
            with self.subTest(mode=mode):
                data = self.rejected(mode)
                self.assertIn("code: Some(23)", data["termination"])
        self.rejected("eof", "ToolFailed")
        for mode in ["handshake-timeout", "timeout", "no-exit"]:
            with self.subTest(mode=mode):
                self.rejected(mode, "ToolTimeout")

    def test_synchronous_terminal_observer_cannot_reset_or_escape_deadlines(self):
        for mode in ["slow-observer-phase", "slow-observer-global"]:
            with self.subTest(mode=mode):
                data = self.rejected(mode, "ToolTimeout")
                self.assertTrue(data["slow_observer_ran"], data)

    def test_cancellation_precedes_result_admission_and_does_not_invoke_after_ack(self):
        for mode in ["cancel", "cancel-ok", "ignore-cancel", "cancel-before-ack", "cancel-validation"]:
            with self.subTest(mode=mode):
                data = self.rejected(mode)
                self.assertEqual(data["admission"], "RejectedByCancellation", data)
                self.assertIsNone(data["harness_error"], data)
                if mode != "cancel-validation":
                    self.assertEqual(data["readiness"]["status"], "marker-observed", data)
                    self.assertIn(data["readiness"]["cancel_decision"],
                                  ["Requested", "AlreadyRequested"], data)
                if mode == "cancel":
                    self.assertEqual(json.loads(data["stdout"].splitlines()[-1])["status"], "cancelled")
                if mode == "cancel-before-ack":
                    self.assertEqual(len(data["stdout"].splitlines()), 1)
                    self.assertNotIn('"kind":"result"', data["stdout"])

    def test_peer_exit_before_marker_retains_actual_failure_and_wakes_watcher(self):
        peer = self.output / "exit-before-ready-peer"
        peer.write_text('#!/bin/sh\nprintf "controlled-exit-before-ready\\n" >&2\nexit 23\n')
        peer.chmod(0o755)
        # Fifteen seconds leaves substantial scheduling headroom but cannot
        # silently wait out the old twenty-second marker timeout.
        data = self.run_mode("cancel-ok", peer=peer, timeout=15)
        self.assertFalse(data["admitted"], data)
        self.assertFalse(data["execution_admitted"], data)
        self.assertEqual(data["admission"], "Unavailable", data)
        self.assertEqual(data["readiness"]["status"], "execution-finished-before-marker", data)
        self.assertEqual(data["harness_error"], "execution finished before readiness marker", data)
        self.assertEqual(data["diagnostic"]["code"], "ToolFailed", data)
        self.assertIn("code: Some(23)", data["termination"], data)
        self.assertIn("controlled-exit-before-ready", data["stderr"], data)
        self.assertEqual(data["cancel_after_finish"], "AlreadyFinished", data)

    def test_payload_and_final_output_must_match_the_terminal_observation(self):
        for mode in ["wrong-payload", "wrong-digest", "outside-output", "undeclared",
                     "late-write", "late-create", "business-failed"]:
            with self.subTest(mode=mode):
                expected = "ToolFailed" if mode == "business-failed" else "OutputInvalid"
                data = self.rejected(mode, expected)
                self.assertEqual([json.loads(s)["kind"] for s in data["stdout"].splitlines()],
                                 ["hello_ack", "result"], data)

    def test_structured_peer_diagnostic_and_descendant_cleanup(self):
        data = self.rejected("tool-error", "ToolFailed")
        self.assertEqual(data["peer_diagnostic"], {"code": "CustomCompilerFailure",
                         "message": "controlled error", "stage": "build", "source": "fixture.c:7"})
        data = self.run_mode("descendant")
        self.assertTrue(data["admitted"], data)
        self.assertIn("descendant=", data["stderr"])
        self.assertTrue(data["cleanup"]["group_kill_attempted"])

    def chain_mode(self, mode):
        result = subprocess.run([str(self.chain), str(self.peer), mode,
                                 str(self.output / f"chain-{mode}")],
                                capture_output=True, text=True, timeout=75, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        data = json.loads(result.stdout)
        (self.output / f"chain-{mode}.json").write_text(json.dumps(data, indent=2))
        for report in data["reports"]:
            self.assertTrue(report["cleanup"]["complete"], report)
            self.assertFalse(report["pending_cleanup"], report)
        return data

    def test_independent_component_sessions_pass_only_frozen_inputs_then_commit_once(self):
        for mode in ["ok", "upstream-mutated"]:
            with self.subTest(mode=mode):
                data = self.chain_mode(mode)
                self.assertTrue(data["committed"], data)
                self.assertEqual(data["after_revision"], data["before_revision"] + 1)
                self.assertEqual([r["component"] for r in data["reports"]], ["first", "second"])
                self.assertEqual([r["session"] for r in data["reports"]], ["first-session", "second-session"])
                self.assertEqual([r["request"] for r in data["reports"]], ["first-request", "second-request"])
                self.assertTrue(all(r["admitted"] for r in data["reports"]), data)
                self.assertEqual(data["transferred"]["references"], data["transferred"]["expected"])
                self.assertEqual(bytes(data["transferred"]["fixture_bytes"]), b"fixture")

    def test_required_component_failure_or_cancel_leaves_store_index_unchanged(self):
        for mode in ["first-failed", "second-failed", "second-cancel", "second-late-write", "cancel-commit"]:
            with self.subTest(mode=mode):
                data = self.chain_mode(mode)
                self.assertFalse(data["committed"], data)
                self.assertTrue(data["index_unchanged"], data)
                self.assertEqual(data["after_revision"], data["before_revision"])
                if mode == "first-failed":
                    self.assertEqual(len(data["reports"]), 1)
                    self.assertIsNone(data["transferred"])
                    self.assertEqual(data["reports"][0]["diagnostic"]["code"], "ToolFailed")
                    self.assertEqual(json.loads(data["reports"][0]["stdout"].splitlines()[-1])["kind"], "result")
                elif mode == "cancel-commit":
                    self.assertIn("STORE002", data["commit_error"])
                    self.assertTrue(all(r["admitted"] for r in data["reports"]), data)
                else:
                    self.assertTrue(data["reports"][0]["admitted"])
                    self.assertFalse(data["reports"][1]["admitted"])
                    second = data["reports"][1]
                    terminal = json.loads(second["stdout"].splitlines()[-1])
                    self.assertEqual(terminal["kind"], "result", data)
                    if mode == "second-cancel":
                        self.assertEqual(second["admission"], "RejectedByCancellation")
                        self.assertEqual(terminal["status"], "cancelled")
                    else:
                        self.assertEqual(second["diagnostic"]["code"],
                                         "ToolFailed" if mode == "second-failed" else "OutputInvalid")


if __name__ == "__main__":
    unittest.main()
