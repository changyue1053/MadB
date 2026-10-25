"""Compile actual Kotlin identity-negative DSOs through a pinned C04 worker.

These libraries are test fixtures, never publishable candidates. The original
facade remains unchanged. Lab chooses wrong bootstrap or wrong provider bytes
explicitly and verifies the documented rejection stage on the actual target.
"""

from __future__ import annotations

import argparse
from copy import deepcopy
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import support

COMMON = support.load_module(ROOT / "sdk/arkts/test-support/build_identity_fixtures.py")
WORKER = "adapters/toolchains/arkts/worker.py"
FACADE = "generated/facade/Boundary.ets"
BOOTSTRAP = "sdk/kotlin/arkts-owned/native/bootstrap.cpp.in"
COMPONENTS = {
    "wrong-abi": "abi_digest",
    "wrong-profile": "profile_digest",
    "wrong-build": "build_id",
}

ORDINARY_FUNCTIONS = [
    "ownedCancel",
    "ownedClaimResult",
    "ownedConnect",
    "ownedDeliver",
    "ownedObserve",
    "ownedOpen",
    "ownedPeek",
    "ownedPublish",
    "ownedRequestClose",
    "ownedRoundtrip",
    "ownedRuntimeClose",
    "ownedSubmit",
    "ownedTakeSnapshot",
    "ownedTakeStatus",
    "ownedWatchClose",
]
TEST_FUNCTIONS = sorted(
    ORDINARY_FUNCTIONS
    + [
        "ownedTestClone",
        "ownedTestDelivery",
        "ownedTestGate",
        "ownedTestHandle",
        "ownedTestInspect",
        "ownedTestPublication",
        "ownedTestReleasePublication",
        "ownedTestTransfer",
    ]
)


def identity_tuple(identity):
    if (
        identity.get("schema") != "madb.arkts-runtime-identity/1"
        or identity.get("bootstrap_version") != 1
    ):
        raise ValueError("unsupported Kotlin runtime identity format")
    values = [identity.get(name) for name in COMPONENTS.values()]
    if any(
        not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
        for value in values
    ):
        raise ValueError("identity requires three explicit lowercase digests")
    text = "madb-arkts/1:" + ":".join(value.removeprefix("sha256:") for value in values)
    if identity.get("expected_identity") != text:
        raise ValueError("derived expected identity differs from its tuple")
    return values


def derive_identity(identity, case):
    identity_tuple(identity)
    result = deepcopy(identity)
    component = COMPONENTS[case]
    value = result[component].removeprefix("sha256:")
    result[component] = "sha256:" + ("1" if value[0] == "0" else "0") + value[1:]
    result["expected_identity"] = "madb-arkts/1:" + ":".join(
        result[name].removeprefix("sha256:") for name in COMPONENTS.values()
    )
    identity_tuple(result)
    return result


def validate_source(task_path, task_sha256, inputs):
    if task_path.is_symlink() or support.sha(task_path) != task_sha256:
        raise ValueError("compile task differs from caller pin")
    task = support.read_json(task_path)
    if (
        task.get("schema") != "madb.build-task/1"
        or task.get("phase") != "compile"
        or task.get("dependencies") != []
        or task.get("selector", {}).get("target") != "aarch64-linux-ohos"
        or task["selector"].get("route") != "KN-ARKTS-NAPI"
    ):
        raise ValueError("fixture requires the original Kotlin C04 compile task")
    names = support.verify_refs(inputs, task["inputs"])
    if (
        not {
            WORKER,
            FACADE,
            BOOTSTRAP,
            "identity.json",
            "backend.json",
            "declarations.json",
        }
        <= names
    ):
        raise ValueError("Kotlin compile closure lacks identity/facade/backend inputs")
    config = support.read_json(inputs / "backend.json")
    variant = config.get("variant")
    expected_test = variant == "test"
    suffix = "test" if expected_test else ""
    if (
        config.get("provider") != "kotlin"
        or variant not in ("ordinary", "test")
        or config.get("kotlin", {}).get("test_support") is not expected_test
        or config.get("native_module") != "libmadbc07sdkkotlin" + suffix + ".so"
    ):
        raise ValueError("Kotlin identity fixture variant/module/test_support differs")
    expected_arguments = config["tools"]["python"].get("invocation_arguments", []) + [
        "../inputs/" + WORKER,
        "--config",
        "../inputs/backend.json",
    ]
    if task["arguments"] != expected_arguments:
        raise ValueError("compile task does not select its frozen C04 worker")
    declarations = support.read_json(inputs / "declarations.json")
    kotlin = declarations.get("kotlin", {})
    if (
        kotlin.get("test_support") is not expected_test
        or "c_seam" in declarations
        or "gateway" in declarations
    ):
        raise ValueError("Kotlin fixture requires actual KSP authority for its variant")
    names = [
        function.get("name") for function in kotlin.get("facts", {}).get("functions", [])
    ]
    expected = TEST_FUNCTIONS if expected_test else ORDINARY_FUNCTIONS
    if names != expected:
        raise ValueError(
            "Kotlin fixture requires actual "
            + str(len(expected))
            + "-function KSP inventory"
        )
    identity = support.read_json(inputs / "identity.json")
    identity_tuple(identity)
    if (inputs / FACADE).read_bytes().count(
        json.dumps(identity["expected_identity"]).encode()
    ) != 1:
        raise ValueError("unchanged facade expectation differs from the native identity")
    bootstrap = support.input_path(inputs, BOOTSTRAP).read_text()
    gates = [
        bootstrap.find(marker)
        for marker in (
            "std::memcmp(expected,MADB_KN_OWNED_IDENTITY,size)",
            "slot=Slot::Initializing;ownerEnv=env;++initAttempts;++epoch;",
            'dlopen("libmadb_kotlin_owned_runtime.so",RTLD_NOW|RTLD_LOCAL)',
            "std::strcmp(providerIdentity,MADB_KN_OWNED_IDENTITY)",
            "if(initialize(env,api,epoch,failStage)!=0)",
        )
    ]
    if any(index < 0 for index in gates) or gates != sorted(gates):
        raise ValueError("reviewed bootstrap/provider identity admission order changed")
    return task, config


def build_fixtures(task_path, task_sha256, inputs, output, *, prepare_only=False):
    inputs = inputs.resolve(strict=True)
    task, config = validate_source(task_path, task_sha256, inputs)
    output = output.absolute()
    if output.exists() or output.is_symlink() or output.is_relative_to(inputs):
        raise ValueError("fixture output must be a fresh separate directory")
    output.mkdir(parents=True)
    parent = COMMON.reference(task_path)
    receipts = []
    for case, component in COMPONENTS.items():
        root = output / case
        frozen = root / "compile/inputs"
        COMMON.copy_inputs(inputs, frozen, task["inputs"])
        original = support.read_json(frozen / "identity.json")
        changed = derive_identity(original, case)
        (frozen / "identity.json").write_text(
            json.dumps(changed, sort_keys=True, indent=2) + "\n"
        )
        refs = [
            COMMON.reference(frozen / item["path"], item["path"])
            for item in task["inputs"]
        ]
        modified = [
            a["path"]
            for a, b in zip(task["inputs"], refs)
            if (a["sha256"], a["size"]) != (b["sha256"], b["size"])
        ]
        if modified != ["identity.json"]:
            raise ValueError(
                "negative fixture changed more than the compiled identity input"
            )
        receipt = dict(
            scope="Kotlin CRIT-12 test fixture; not candidate or target evidence",
            candidate=False,
            target_executed=False,
            variant=config["variant"],
            case=case,
            parent_task=parent,
            generation_id=task["generation_id"],
            selector=task["selector"],
            native_module=config["native_module"],
            changed_files=modified,
            logical_identity_component=component,
            original_identity=original,
            fixture_identity=changed,
            original_inputs=task["inputs"],
            fixture_inputs=refs,
            unchanged_facade=COMMON.reference(frozen / FACADE, FACADE),
            bootstrap_error="IdentityMismatch",
            provider_only_error="ProviderIdentityMismatch",
            bootstrap_rejection_scope="before Kotlin dlopen/admission; original facade expectation",
            counter_scope="same-source test counterpart only; ordinary carries no counter claim",
            provider_rejection_scope="original bootstrap plus changed provider; before managed initialize",
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
                        COMMON.copy_inputs(frozen, phase_inputs, refs)
                        for item in previous:
                            support.copy_new(
                                root / "compile/work" / item["path"],
                                phase_inputs / item["path"],
                            )
                    support.verify_refs(phase_inputs, refs + previous)
                    work = root / phase / "work"
                    work.mkdir()
                    products = worker.run_phase(config, phase, phase_inputs, work)
                    support.verify_refs(phase_inputs, refs + previous)
                    previous = [
                        COMMON.reference(support.input_path(work, name), name)
                        for name in products
                    ]
                    receipt["phases"].append(
                        dict(
                            phase=phase,
                            outputs=previous,
                            log=COMMON.reference(
                                work / ("logs/" + phase + "-commands.json")
                            ),
                        )
                    )
                receipt["libraries"] = [
                    COMMON.reference(root / "link/work/native" / name)
                    for name in (
                        config["native_module"],
                        "libmadb_kotlin_owned_runtime.so",
                    )
                ]
                receipt["build_status"] = "compiled-and-linked"
            support.verify_refs(inputs, task["inputs"])
            support.verify_refs(frozen, refs)
            if support.sha(task_path) != parent["sha256"]:
                raise ValueError(
                    "parent compile task changed during negative fixture build"
                )
        except Exception as failure:
            receipt.update(build_status="failed", error=str(failure))
            raise
        finally:
            COMMON.save(root / "fixture-receipt.json", receipt)
        receipts.append(COMMON.reference(root / "fixture-receipt.json"))
    COMMON.save(
        output / "fixtures.json",
        dict(
            candidate=False,
            target_executed=False,
            variant=config["variant"],
            helper=COMMON.reference(Path(__file__)),
            common_helper=COMMON.reference(
                ROOT / "sdk/arkts/test-support/build_identity_fixtures.py"
            ),
            parent_task=parent,
            fixtures=receipts,
        ),
    )
    return receipts


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
        parser.error("compile-task SHA256 must be lowercase")
    print(
        json.dumps(
            build_fixtures(
                args.compile_task,
                pin,
                args.inputs,
                args.output,
                prepare_only=args.prepare_only,
            ),
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
