"""Build CRIT-12 negative DSOs from pinned, generated ordinary or test inputs.

This is producer test support, not a C04 candidate builder or target runner.
Compilation and linking belong to the unchanged worker frozen by the parent task.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import support

HEADER = "generated/native/identity.generated.hpp"
FACADE = "generated/facade/Boundary.ets"
WORKER = "adapters/toolchains/arkts/worker.py"
CASES = {
    "wrong-abi": ("GatewayAbi", 0, "AbiRejected", "AbiMismatch", 3),
    "wrong-profile": ("GatewayProfile", 1, "ProfileRejected", "ProfileMismatch", 4),
    "wrong-build": ("GatewayBuild", 2, "ArtifactRejected", "ArtifactMismatch", 5),
}
FIELDS = [
    "Format",
    "OpenAttempts",
    "IdentityMatched",
    "AbiRejected",
    "ProfileRejected",
    "ArtifactRejected",
    "Reservations",
    "ContextsCreated",
    "EnginesCreated",
    "WorkerStarted",
    "RefCreated",
    "RefDeleted",
    "TsfnCreated",
    "TsfnReleaseAccepted",
    "HookAdded",
    "HookRemoved",
]

ORDINARY_METHODS = [
    "runtimeOpen",
    "connect",
    "roundtrip",
    "submit",
    "cancel",
    "observe",
    "publish",
    "requestClose",
    "claimResult",
    "watchClose",
    "tryTake",
    "deliver",
    "requestRuntimeClose",
    "pollRuntime",
    "consumeRuntimeReceipt",
    "audit",
]
TEST_METHODS = ORDINARY_METHODS + [
    "testArmWork",
    "testReleaseWork",
    "testHoldPublication",
    "testReleasePublication",
    "testFailClone",
    "testFailCompletion",
    "testObserve",
    "testExportKey",
    "testInspect",
    "testObserveOperation",
    "testArmPartialOpenFailure",
    "testLastAdmissionAudit",
    "testDeliveryGate",
    "testHandleAdmission",
    "testBootstrapTotals",
    "testMemory",
]


def reference(path, logical=None):
    return {
        "path": logical or str(path),
        "sha256": support.sha(path),
        "size": path.stat().st_size,
    }


def save(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def constants(data):
    rows = re.findall(
        r'^inline constexpr const char\* (Gateway\w+)=("[^"\n]*");$', data.decode(), re.M
    )
    result = {name: json.loads(value) for name, value in rows}
    required = {
        "GatewayAbi",
        "GatewayProfile",
        "GatewayBuild",
        "GatewayIdentity",
        "GatewayRouteIdentity",
        "GatewayModuleName",
    }
    if set(result) != required or len(rows) != len(required):
        raise ValueError("generated identity constant shape changed")
    triple = [result[name] for name in ("GatewayAbi", "GatewayProfile", "GatewayBuild")]
    if any(re.fullmatch(r"[0-9a-f]{64}", value) is None for value in triple):
        raise ValueError("native identity must contain three lowercase 64hex digests")
    if result["GatewayIdentity"] != "madb-arkts/1:" + ":".join(triple):
        raise ValueError("native identity tuple and derived string disagree")
    return result


def derive_header(data, case):
    original = constants(data)
    name = CASES[case][0]
    changed = dict(original)
    # Change exactly one logical tuple member, with a valid deterministic digest.
    changed[name] = ("1" if original[name][0] == "0" else "0") + original[name][1:]
    changed["GatewayIdentity"] = "madb-arkts/1:" + ":".join(
        changed[key] for key in ("GatewayAbi", "GatewayProfile", "GatewayBuild")
    )
    route, separator, identity = original["GatewayRouteIdentity"].partition("/")
    if (
        not separator
        or route not in ("c", "cpp", "rust")
        or identity != original["GatewayIdentity"]
    ):
        raise ValueError("native route identity disagrees with original tuple")
    changed["GatewayRouteIdentity"] = route + "/" + changed["GatewayIdentity"]
    text = data.decode()
    edits = []
    for key in (name, "GatewayIdentity", "GatewayRouteIdentity"):
        old = (
            "inline constexpr const char* " + key + "=" + json.dumps(original[key]) + ";"
        )
        new = "inline constexpr const char* " + key + "=" + json.dumps(changed[key]) + ";"
        if text.count(old) != 1:
            raise ValueError("native identity replacement is not unique")
        text = text.replace(old, new)
        edits.append({"constant": key, "before": original[key], "after": changed[key]})
    return text.encode(), edits


def validate_source(task_path, task_sha256, inputs):
    if task_path.is_symlink() or support.sha(task_path) != task_sha256:
        raise ValueError("compile task differs from caller pin")
    task = support.read_json(task_path)
    if (
        task.get("schema") != "madb.build-task/1"
        or task.get("phase") != "compile"
        or task.get("dependencies") != []
        or task.get("selector", {}).get("target") != "aarch64-linux-ohos"
    ):
        raise ValueError("fixture requires an actual initial C04 compile task")
    refs = task["inputs"]
    names = support.verify_refs(inputs, refs)
    required = {
        HEADER,
        FACADE,
        WORKER,
        "backend.json",
        "identity.json",
        "declarations.json",
    }
    if not required <= names:
        raise ValueError("compile task lacks required generated route inputs")
    config = support.read_json(inputs / "backend.json")
    route = config.get("provider")
    variant = config.get("variant")
    if variant not in ("ordinary", "test") or route not in ("c", "cpp", "rust"):
        raise ValueError("identity fixtures require an ordinary or test native route")
    suffix = "test" if variant == "test" else ""
    if config["native_module"] != "libmadbc07sdk" + route + suffix + ".so":
        raise ValueError("fixture variant/module identity differs")
    if task["selector"].get("route") != "ARKTS-" + route.upper() + "-NAPI":
        raise ValueError("fixture task route differs from provider")
    expected_args = config["tools"]["python"].get("invocation_arguments", []) + [
        "../inputs/" + WORKER,
        "--config",
        "../inputs/backend.json",
    ]
    if task["arguments"] != expected_args:
        raise ValueError("compile task does not select the existing frozen worker")
    declared = support.read_json(inputs / "declarations.json")["gateway"]["methods"]
    methods = [method["name"] for method in declared]
    expected_methods = TEST_METHODS if variant == "test" else ORDINARY_METHODS
    if methods != expected_methods:
        raise ValueError(
            "fixture requires the actual "
            + str(len(expected_methods))
            + "-method "
            + variant
            + " declaration"
        )
    if variant == "test":
        layout_path = "sdk/arkts/test-support/contracts/bootstrap-totals-layout.json"
        if layout_path not in names:
            raise ValueError("test fixture lacks bootstrap totals contract")
        layout = support.read_json(inputs / layout_path)
        if (layout.get("format"), layout.get("length"), layout.get("fields")) != (
            1,
            16,
            {str(i): name for i, name in enumerate(FIELDS)},
        ):
            raise ValueError(
                "bootstrap totals contract changed; review the expectation explicitly"
            )
    units = config["build"]["units"]
    runtime = [
        unit["source"] for unit in units if unit["source"].endswith("/src/runtime.cpp")
    ]
    if len(runtime) != 1 or runtime[0] not in names:
        raise ValueError("fixture requires the pinned actual shared gateway runtime")
    text = support.input_path(inputs, runtime[0]).read_text()
    positions = []
    for constant, part, event, code, _ in CASES.values():
        if variant == "test":
            pattern = (
                r"if\s*\(parts\["
                + str(part)
                + r"\]\s*!=\s*"
                + constant
                + r"\)\s*\{\s*testBootstrapEvent\(BootstrapEvent::"
                + event
                + r'\);\s*return error\(env, "'
                + code
                + r'"\);\s*\}'
            )
        else:
            pattern = (
                r"if\s*\(parts\["
                + str(part)
                + r"\]\s*!=\s*"
                + constant
                + r'\)\s*return error\(env, "'
                + code
                + r'"\);'
            )
        found = list(re.finditer(pattern, text))
        if len(found) != 1:
            raise ValueError("bootstrap rejection code/event changed: " + constant)
        positions.append(found[0].start())
    if variant == "test":
        matched = text.index("testBootstrapEvent(BootstrapEvent::IdentityMatched);")
        reserved = text.index("testBootstrapEvent(BootstrapEvent::Reservation);")
        ordered = positions[-1] < matched < reserved
    else:
        # Ordinary has no diagnostic event calls. Its same gate must precede
        # the actual registry epoch/slot reservation, not a fabricated counter.
        reserved = text.index("s.epoch = epoch = ++nextEpoch;")
        ordered = positions[-1] < reserved
        if "testBootstrapEvent(" in text:
            raise ValueError("ordinary fixture contains companion instrumentation")
    if positions != sorted(positions) or not ordered:
        raise ValueError("bootstrap identity gates no longer precede reservation")
    native = constants((inputs / HEADER).read_bytes())
    identity = support.read_json(inputs / "identity.json")
    if (
        identity.get("expected_identity") != native["GatewayIdentity"]
        or native["GatewayRouteIdentity"] != route + "/" + native["GatewayIdentity"]
        or native["GatewayModuleName"] != config["native_module"][3:-3]
        or (inputs / FACADE)
        .read_bytes()
        .count(json.dumps(native["GatewayIdentity"]).encode())
        != 1
    ):
        raise ValueError("native tuple, module and unchanged facade expectation disagree")
    return task, config


def copy_inputs(source, destination, refs):
    destination.mkdir(parents=True, exist_ok=False)
    support.verify_refs(source, refs)
    for item in refs:
        support.copy_new(
            support.input_path(source, item["path"]), destination / item["path"]
        )
    support.verify_refs(destination, refs)
    support.verify_refs(source, refs)


def build_fixtures(task_path, task_sha256, inputs, output, *, prepare_only=False):
    inputs = inputs.resolve(strict=True)
    task, config = validate_source(task_path, task_sha256, inputs)
    if output.exists() or output.is_symlink():
        raise ValueError("fixture output must be a fresh separate directory")
    output = output.absolute()
    if output.is_relative_to(inputs):
        raise ValueError("fixture output must be a fresh separate directory")
    output.mkdir(parents=True, exist_ok=False)
    parent = reference(task_path)
    results = []
    for case, (_, _, event, code, counter) in CASES.items():
        root = output / case
        frozen = root / "compile/inputs"
        copy_inputs(inputs, frozen, task["inputs"])
        original = (frozen / HEADER).read_bytes()
        mutated, edits = derive_header(original, case)
        (frozen / HEADER).write_bytes(mutated)
        refs = [reference(frozen / item["path"], item["path"]) for item in task["inputs"]]
        changed = [
            new["path"]
            for old, new in zip(task["inputs"], refs)
            if (old["sha256"], old["size"]) != (new["sha256"], new["size"])
        ]
        if changed != [HEADER]:
            raise ValueError("fixture changed an input outside the identity header")
        delta = [0] * 16
        delta[1] = delta[counter] = 1
        receipt = dict(
            scope="CRIT-12 negative native fixture; never a candidate or target result",
            candidate=False,
            target_executed=False,
            variant=config["variant"],
            case=case,
            parent_task=parent,
            generation_id=task["generation_id"],
            selector=task["selector"],
            native_module=config["native_module"],
            changed_files=changed,
            logical_identity_component=CASES[case][0],
            native_constant_edits=edits,
            original_inputs=task["inputs"],
            fixture_inputs=refs,
            unchanged_facade=reference(frozen / FACADE, FACADE),
            expected_error=code,
            expected_rejection_event=event if config["variant"] == "test" else None,
            expected_bootstrap_delta=delta if config["variant"] == "test" else None,
            counter_scope="same-source companion only; ordinary has no diagnostic counters",
            expectation_scope=(
                "one public open in isolated env; Format stays 1; no target assertion performed"
                if config["variant"] == "test"
                else "unchanged ordinary facade; exact bootstrap stage/error and replaced member only; no counter or target assertion"
            ),
            build_status="not-run",
            phases=[],
        )
        try:
            if not prepare_only:
                worker = support.load_module(frozen / WORKER)
                previous = []
                for phase in ("compile", "link"):
                    phase_inputs = root / phase / "inputs"
                    if phase == "link":
                        copy_inputs(frozen, phase_inputs, refs)
                        for item in previous:
                            support.copy_new(
                                root / "compile/work" / item["path"],
                                phase_inputs / item["path"],
                            )
                        support.verify_refs(phase_inputs, previous)
                    work = root / phase / "work"
                    work.mkdir()
                    products = worker.run_phase(config, phase, phase_inputs, work)
                    support.verify_refs(phase_inputs, refs + previous)
                    previous = [
                        reference(support.input_path(work, name), name)
                        for name in products
                    ]
                    receipt["phases"].append(
                        dict(
                            phase=phase,
                            outputs=previous,
                            log=reference(work / ("logs/" + phase + "-commands.json")),
                        )
                    )
                library = root / "link/work/native" / config["native_module"]
                receipt["library"] = reference(library)
                receipt["build_status"] = "compiled-and-linked"
            support.verify_refs(inputs, task["inputs"])
            support.verify_refs(frozen, refs)
            if support.sha(task_path) != parent["sha256"]:
                raise ValueError("parent compile task changed during fixture build")
        except Exception as failure:
            receipt["build_status"] = "failed"
            receipt["error"] = str(failure)
            raise
        finally:
            save(root / "fixture-receipt.json", receipt)
        results.append(reference(root / "fixture-receipt.json"))
    save(
        output / "fixtures.json",
        dict(
            candidate=False,
            target_executed=False,
            variant=config["variant"],
            helper=reference(Path(__file__)),
            parent_task=parent,
            fixtures=results,
        ),
    )
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compile-task", required=True, type=Path)
    parser.add_argument("--compile-task-sha256", required=True)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    pin = "sha256:" + args.compile_task_sha256.removeprefix("sha256:")
    if re.fullmatch(r"sha256:[0-9a-f]{64}", pin) is None:
        parser.error("--compile-task-sha256 must be lowercase SHA256")
    result = build_fixtures(
        args.compile_task, pin, args.inputs, args.output, prepare_only=args.prepare_only
    )
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
