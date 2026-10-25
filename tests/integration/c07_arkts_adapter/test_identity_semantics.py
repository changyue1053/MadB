"""ABI hashes type/protocol semantics; Source/build retain byte provenance."""

from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts.lowering import abi_projection


class SemanticIdentity(unittest.TestCase):
    def setUp(self):
        # Small typed test vectors, not a claim of compiler/target observations.
        self.contract = {
            "types": [
                {"id": "Handle", "definition": {"kind": "resource", "owner": "owner-a"}}
            ],
            "symbols": [
                {
                    "public_id": "call",
                    "shape": {
                        "link_name": "call",
                        "parameters": [],
                        "result": "Handle",
                        "throws": "Error",
                    },
                    "location": {"line": 1},
                }
            ],
            "policies": [
                {
                    "symbol_id": "call",
                    "slot": {"kind": "call"},
                    "values": {
                        "threading": "runtime-environment",
                        "cancellation": "request-only",
                    },
                    "provenance": [],
                }
            ],
            "required_features": ["core.callbacks@1"],
            "source_digest": "old-source",
            "selector": {"profile_digest": "old-profile"},
        }
        self.policy = {
            "calls": {"call": {"public_id": "call"}},
            "limits": {"waiters": 32},
            "value_slots": {"command": {"allowed_integers": [0, 1]}},
            "execution_domain": "main-env-only",
            "runtime_reopen": {
                "same_env": True,
                "requires_physical_ack": True,
                "requires_receipt_consumed": True,
                "epoch_reuse": False,
            },
            "authority": {"sha256": "old"},
            "notes": "documentation",
        }
        self.descriptor = {
            "public": {
                "records": [
                    {
                        "name": "Snapshot",
                        "fields": [{"name": "value", "type": {"tag": "number"}}],
                    }
                ],
                "source": {"sha256": "old"},
            },
            "gateway": {
                "methods": [{"name": "call", "returns": {"tag": "string"}}],
                "tools": {"node": "old"},
            },
            "c_seam": {
                "records": [{"name": "owned", "size": 16}],
                "source": {"sha256": "old"},
            },
            "rust": {
                "macro": {
                    "descriptor": {
                        "exports": [
                            {
                                "name": "call",
                                "arity": 1,
                                "parameters": [{"name": "value", "rawType": "unknown"}],
                                "rawReturn": "unknown",
                                "backendRecord": 0,
                                "location": {"start": 0},
                            }
                        ],
                        "generatedDeclarationSha256": "old",
                    },
                    "raw_sha256": "old",
                    "records": ["generated text"],
                },
                "resolved_packages": [
                    {
                        "name": "napi-ohos",
                        "features": ["napi3"],
                        "version": "1.2.0",
                        "source": "registry",
                    }
                ],
                "inputs": [{"sha256": "old"}],
                "tools": {"rustc": "old"},
                "compiler_host": {"tree": "old"},
                "frontend_evidence": {"expanded_rust": "implementation"},
            },
            "kotlin": {
                "facts": {
                    "functions": [
                        {
                            "name": "call",
                            "returnType": "kotlin.Int",
                            "source": {"line": 1},
                        }
                    ]
                },
                "descriptor": {
                    "functions": [
                        {"name": "call", "returnType": "number", "location": {"start": 0}}
                    ]
                },
                "record_schema": {
                    "qualifiedName": "Snapshot",
                    "fields": [
                        {
                            "name": "value",
                            "kotlinType": "kotlin.Int",
                            "location": "line 1",
                        }
                    ],
                    "generator": "old-tool",
                },
                "frontend_policy": {
                    "notification_interfaces": {
                        "return_type": "kotlin.Unit",
                        "synchronous_cross_thread_return": False,
                    },
                    "parameters": {"owned_interface_maximum": 1},
                },
                "source_inputs": [{"sha256": "old"}],
                "tools": {"compiler": "old"},
                "generated": [{"text": "implementation"}],
                "closure": {"tree": "old"},
            },
        }

    def digest(self, descriptor=None, contract=None, policy=None):
        value = abi_projection(
            descriptor or self.descriptor,
            contract or self.contract,
            policy or self.policy,
        )
        return hashlib.sha256(
            json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()

    def test_source_tool_and_generated_bytes_do_not_change_abi(self):
        before = self.digest()
        changed = deepcopy(self.descriptor)
        changed["public"]["source"] = {"sha256": "changed"}
        changed["gateway"]["tools"] = {"node": "changed"}
        changed["c_seam"]["source"] = {"sha256": "changed"}
        rust = changed["rust"]
        for key in ("inputs", "tools", "compiler_host", "frontend_evidence"):
            rust[key] = {"changed": True}
        rust["macro"]["raw_sha256"] = "changed"
        rust["macro"]["records"] = ["new generated text"]
        rust["macro"]["descriptor"]["generatedDeclarationSha256"] = "changed"
        rust["macro"]["descriptor"]["exports"][0]["location"] = {"start": 999}
        rust["macro"]["descriptor"]["exports"][0]["backendRecord"] = 9
        kotlin = changed["kotlin"]
        for key in ("source_inputs", "tools", "generated", "closure"):
            kotlin[key] = {"changed": True}
        kotlin["facts"]["functions"][0]["source"] = {"line": 999}
        kotlin["descriptor"]["functions"][0]["location"] = {"start": 999}
        kotlin["record_schema"]["fields"][0]["location"] = "line 999"
        kotlin["record_schema"]["generator"] = "new-tool-bytes"
        contract = deepcopy(self.contract)
        contract["source_digest"] = "changed-source"
        contract["selector"] = {"profile_digest": "changed-profile"}
        contract["symbols"][0]["location"] = {"line": 999}
        contract["policies"][0]["provenance"] = ["changed location"]
        policy = deepcopy(self.policy)
        policy["authority"]["sha256"] = "changed"
        policy["notes"] = "edited documentation"
        self.assertEqual(before, self.digest(changed, contract, policy))

    def test_contract_names_errors_cancellation_thread_owner_and_limits_change_abi(self):
        cases = [
            ("contract", ["symbols", 0, "public_id"], "other-call"),
            ("contract", ["symbols", 0, "shape", "link_name"], "other-name"),
            ("contract", ["symbols", 0, "shape", "throws"], "OtherError"),
            ("contract", ["policies", 0, "values", "cancellation"], "terminal-result"),
            ("contract", ["policies", 0, "values", "threading"], "calling-thread"),
            ("contract", ["types", 0, "definition", "owner"], "owner-b"),
            ("policy", ["limits", "waiters"], 31),
            ("policy", ["value_slots", "command", "allowed_integers"], [0, 1, 2]),
            ("policy", ["execution_domain"], "any-env"),
            ("policy", ["runtime_reopen", "same_env"], False),
            ("policy", ["runtime_reopen", "requires_physical_ack"], False),
            ("policy", ["runtime_reopen", "requires_receipt_consumed"], False),
            ("policy", ["runtime_reopen", "epoch_reuse"], True),
        ]
        self.check_mutations(cases)

    def test_actual_backend_type_and_protocol_changes_change_abi(self):
        self.check_mutations(
            [
                ("descriptor", ["c_seam", "records", 0, "size"], 24),
                (
                    "descriptor",
                    ["rust", "macro", "descriptor", "exports", 0, "rawReturn"],
                    "void",
                ),
                ("descriptor", ["rust", "macro", "descriptor", "exports", 0, "arity"], 2),
                ("descriptor", ["rust", "resolved_packages", 0, "features"], ["napi4"]),
                (
                    "descriptor",
                    ["kotlin", "facts", "functions", 0, "returnType"],
                    "kotlin.Long",
                ),
                (
                    "descriptor",
                    ["kotlin", "record_schema", "fields", 0, "kotlinType"],
                    "kotlin.Long",
                ),
                (
                    "descriptor",
                    [
                        "kotlin",
                        "frontend_policy",
                        "notification_interfaces",
                        "synchronous_cross_thread_return",
                    ],
                    True,
                ),
            ]
        )

    def check_mutations(self, cases):
        baseline = self.digest()
        for name, path, value in cases:
            with self.subTest(path=path):
                parts = {
                    "descriptor": deepcopy(self.descriptor),
                    "contract": deepcopy(self.contract),
                    "policy": deepcopy(self.policy),
                }
                target = parts[name]
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = value
                self.assertNotEqual(baseline, self.digest(**parts))


if __name__ == "__main__":
    unittest.main()
