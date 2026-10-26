#!/usr/bin/env python3
"""Finite I01 design examples only; not a wire/schema/host implementation."""
from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import sys


HERE = Path(__file__).resolve().parent
VECTORS = HERE / "project-init-decision-vectors.json"
EXIT = {"CheckFailed": 1, "InvalidInput": 2, "Host": 3, "Conflict": 4}


def select(profiles, default, profile, targets):
    if targets is not None:
        if not targets or len(targets) != len(set(targets)):
            raise ValueError("invalid-targets")
        targets = sorted(targets)
    if profile is not None:
        if profile not in profiles:
            raise ValueError("unknown-profile")
        families = sorted(set(profiles[profile]))
        if targets is not None and targets != families:
            raise ValueError("profile-target-mismatch")
        return {"profile": profile, "targets": families}
    if targets is None:
        if default not in profiles:
            raise ValueError("unknown-default")
        return {"profile": default, "targets": sorted(set(profiles[default]))}
    matches = sorted(p for p, families in profiles.items() if sorted(set(families)) == targets)
    if not matches:
        raise ValueError("no-match")
    if len(matches) == 1:
        return {"profile": matches[0], "targets": targets}
    if default in matches:
        return {"profile": default, "targets": targets}
    raise ValueError("ambiguous-profile")


def classification(case):
    # category is a trusted typed-stage observation, never read from diagnostic text.
    expected = 1 if case["cancelled"] else EXIT[case["category"]]
    return case["reported_exit"] == expected


def success(case):
    if not case["identity_complete"] or case["termination"] != "completed":
        return False
    if case["exit_code"] != 0 or not case["cleanup_complete"]:
        return False
    if case["reported_dry_run"] != case["frozen_dry_run"]:
        return False
    if case["open_requested"] != case["request_open"]:
        return False
    if case["frozen_dry_run"]:
        return (
            case["creation"] == "not-created"
            and not case["commit_observed"]
            and not case["checks"]
            and case["open_status"] == "skipped"
            and not case["execution_or_download_observed"]
        )
    if case["creation"] != "created" or not case["commit_observed"]:
        return False
    indexed = {}
    for item in case["checks"]:
        if item["key"] in indexed:
            return False
        indexed[item["key"]] = item
    for key in case["required_checks"]:
        item = indexed.get(key)
        if item is None or item["status"] != "passed" or item["cleanup"] != "complete":
            return False
    expected_open = "skipped" if case["request_open"] is None else "passed"
    return case["open_status"] == expected_open


def logical(path):
    if not isinstance(path, str) or not path or path.startswith("/"):
        return False
    if "\\" in path or ":" in path or any(ord(c) < 32 for c in path):
        return False
    return all(piece not in ("", ".", "..") for piece in path.split("/"))


def references(case):
    # This is a logical mapping/raw-byte example, NOT a symlink/race host check.
    root, original = case["input_root"], case["original_path"]
    if not logical(root) or not logical(original) or not logical(case["outer_path"]):
        return False
    if case["outer_path"] != root + "/" + original:
        return False
    source = case["original_text"].encode("utf-8")
    copied = case["copied_text"].encode("utf-8")
    if copied != source or len(source) != case["reference_size"]:
        return False
    if sha256(source).hexdigest() != case["reference_sha256"]:
        return False
    nested = case["nested_path"]
    return (
        logical(nested)
        and case["nested_lookup"] == root + "/" + nested
        and case["required_closure_present"]
        and case["package_namespace_preserved"]
    )


def sdk_gate(case):
    if case["legacy_requirements_sdks_present"]:
        return False
    if not case["renderer_available"]:
        return False
    if not case["stage_sdk_required"]:
        return True
    return case["known_recipe"] and case["descriptor_supported"] and case["sdk_observed"]


def main():
    data = json.loads(VECTORS.read_text())
    if data["status"] != "unadopted-design-fragments-not-wire":
        raise ValueError("not design fixture")
    root = Path(data["source_root"])
    for pin in data["source_pins"]:
        raw = (root / pin["path"]).read_bytes()
        if len(raw) != pin["size"] or sha256(raw).hexdigest() != pin["sha256"]:
            raise ValueError("source changed: " + pin["path"])
    ids = set()
    counts = {}
    for group, cases in data["cases"].items():
        for case in cases:
            if case["id"] in ids:
                raise ValueError("duplicate design-case id")
            ids.add(case["id"])
            if group == "selection":
                try:
                    actual = select(
                        data["profiles"], case["default"], case["profile"], case["targets"]
                    )
                except ValueError as error:
                    actual = {"error": str(error)}
            else:
                item = deepcopy(data["bases"].get(group, {}))
                item.update(case["input"])
                actual = {"classification": classification, "success": success,
                          "references": references, "sdk_gate": sdk_gate}[group](item)
            if actual != case["expected"]:
                raise AssertionError(f"{case['id']}: {actual!r} != {case['expected']!r}")
        counts[group] = len(cases)
    result = {
        "kind": "i01-design-check-result",
        "status": "passed-design-subsets-only",
        "command": [sys.executable, "-B", str(Path(__file__).resolve())],
        "source_root": str(root),
        "source_pin_count": len(data["source_pins"]),
        "case_count": len(ids),
        "groups": counts,
        "artifacts": [],
        "limits": ["not full schema admission", "not full request digest or JCS",
                   "not ProjectPlan implementation or test", "not real CommitNew or host path isolation",
                   "not SDK execution, IDE, device or I01 certification"],
    }
    for path in [Path(__file__).resolve(), VECTORS, HERE / "project-init-decision.md"]:
        raw = path.read_bytes()
        result["artifacts"].append({"path": str(path), "sha256": sha256(raw).hexdigest(), "size": len(raw)})
    output = HERE / "project-init-decision-check-result.json"
    output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"result": str(output), "case_count": len(ids), "groups": counts,
                      "source_pin_count": len(data["source_pins"])}, ensure_ascii=False))


if __name__ == "__main__":
    main()
