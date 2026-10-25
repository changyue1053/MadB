"""Render ordinary Kotlin facade bindings from independently read KSP facts."""

from __future__ import annotations
import json
from pathlib import Path
from . import kotlin_dependencies as dependencies
from . import kotlin_frame
from . import kotlin_test_binding

support = dependencies.support
SDK = "sdk/kotlin/arkts-owned"


def public_source(inputs, config, identity_string):
    """Actual Kotlin public source before KSP; no generated native API input."""
    template = support.input_path(inputs, "sdk/arkts/facade/Boundary.ets.in").read_text()
    for marker in ("@MADB_NATIVE_MODULE@", "@MADB_EXPECTED_IDENTITY@"):
        if template.count(marker) != 1:
            raise ValueError("Kotlin public facade marker differs")
    boundary = template.replace("@MADB_NATIVE_MODULE@", "'./KotlinPort'").replace(
        "@MADB_EXPECTED_IDENTITY@", json.dumps(identity_string)
    )
    if boundary.count("frame.length !== 54") != 1:
        raise ValueError("Kotlin frame integration seam differs")
    boundary = boundary.replace("frame.length !== 54", "frame.length !== 110")
    if config["kotlin"]["test_support"]:
        boundary = kotlin_test_binding.derive_boundary(boundary)
    return boundary.encode()


def render(inputs, config, descriptor, identity, work, *, capture=None):
    """Return real generated logical paths. Identity comes from the common driver."""
    inputs, work = Path(inputs), Path(work)
    capture = [] if capture is None else capture
    facts = descriptor["facts"]
    record = facts["record"]["qualifiedName"].rsplit(".", 1)[-1]

    def t(name):
        return {
            "kotlin.Int": "number",
            "kotlin.String": "string",
            "kotlin.Boolean": "boolean",
            "kotlin.Unit": "void",
        }.get(name, name.rsplit(".", 1)[-1])

    methods = [
        "  "
        + f["name"]
        + "("
        + ", ".join(p["name"] + ": " + t(p["kotlinType"]) for p in f["parameters"])
        + "): "
        + t(f["returnType"])
        + ";"
        for f in facts["functions"]
    ]
    native = (
        support.input_path(inputs, SDK + "/native/Bootstrap.d.ts.in")
        .read_text()
        .replace("@MADB_KOTLIN_RECORD@", record)
        .replace("@MADB_KOTLIN_METHODS@", "\n".join(methods))
    )
    port = (
        support.input_path(inputs, SDK + "/facade/KotlinPort.ets.in")
        .read_text()
        .replace("@MADB_KOTLIN_NATIVE_MODULE@", config["native_module"])
    )
    if record != "Snapshot":
        port = port.replace(
            "OwnedApi, Snapshot, RuntimeDispatcher",
            "OwnedApi, " + record + " as Snapshot, RuntimeDispatcher",
        )
    boundary = public_source(inputs, config, identity["expected_identity"]).decode()
    if config["kotlin"]["test_support"]:
        port = kotlin_test_binding.derive_port(port)
        native = kotlin_test_binding.derive_declaration(native)
    outputs = {
        **kotlin_frame.render(),
        "generated/facade/Boundary.ets": boundary.encode(),
        "generated/facade/KotlinPort.ets": port.encode(),
        "generated/native/Index.d.ts": native.encode(),
        "generated/native/record-export.d.ts": descriptor["declaration"].encode(),
        "generated/contract/kotlin-ksp-facts.json": (
            json.dumps(facts, sort_keys=True, indent=2) + "\n"
        ).encode(),
        "generated/contract/kotlin-record-schema.json": (
            json.dumps(descriptor["record_schema"], sort_keys=True, indent=2) + "\n"
        ).encode(),
    }
    if config["kotlin"]["test_support"]:
        for p in sorted(
            support.input_path(inputs, SDK + "/contracts/admission-frame.json").parent.glob(
                "*.json"
            )
        ):
            outputs["generated/contract/test/" + p.name] = p.read_bytes()
    for name, data in outputs.items():
        p = work / name
        p.parent.mkdir(parents=True, exist_ok=True)
        with p.open("xb") as f:
            f.write(data)
    public = json.loads(
        support.command(
            config,
            "node",
            [
                support.input_path(inputs, "adapters/generators/arkts/read_kotlin_public.cjs"),
                support.verify_tool(config["tools"]["typescript"]),
                work / "generated/facade/Boundary.ets",
            ],
            cwd=work,
            capture=capture,
        )
    )
    properties = {n for names in public["interfaces"].values() for n in names}
    properties.update(f["name"] for f in facts["functions"])
    for interface in facts["interfaces"]:
        properties.update(m["name"] for m in interface["methods"])
    properties.update(
        (
            "initialize",
            "commit",
            "abortAdmission",
            "poll",
            "audit",
            "consumeReceipt",
            "bootstrapAudit",
            "__admissionKey",
            "status",
        )
    )
    if config["kotlin"]["test_support"]:
        properties.update(
            ("testArmPartialOpen", "testPollAdmission", "testNapiAudit", "testMemoryAudit")
        )
    # The public interface AST includes SavedCalls.takeStatus; no obfuscator-private name is accepted.
    rules = "-keep-property-name\n" + "\n".join(sorted(properties)) + "\n"
    p = work / "generated/keep-rules.txt"
    p.write_text(rules)
    receipt = work / "generated/contract/kotlin-binding.json"
    receipt.write_text(
        json.dumps(
            dict(
                authority="actual KSP facts and emitted declaration; bootstrap has separate authored ABI",
                record=record,
                public_interfaces=public,
                identity=identity,
                ordinary_test_isolation=config["kotlin"]["test_support"],
            ),
            sort_keys=True,
            indent=2,
        )
        + "\n"
    )
    return [*outputs, "generated/keep-rules.txt", "generated/contract/kotlin-binding.json"]
