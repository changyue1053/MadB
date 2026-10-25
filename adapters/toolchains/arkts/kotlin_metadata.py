"""Actual KSP frontend. Matching helper/cinterop/runtime compilation is an explicit input."""

from __future__ import annotations
import hashlib
import json
from pathlib import Path
from . import kotlin_dependencies as dependencies
from . import kotlin_project as project

support = dependencies.support
_ROOT = Path(__file__).resolve().parents[3]


def portable_facts(data, root, config):
    variant = "test" if config["kotlin"]["test_support"] else "ordinary"
    mappings = {
        str(root / "probe/src/ohosArm64Main/kotlin/com/madb/c07")
        + "/": project.SDK
        + "/src/common/",
        str(
            root
            / "probe/src"
            / ("testSupportMain" if variant == "test" else "ordinaryMain")
            / "kotlin/com/madb/c07"
        )
        + "/": project.SDK
        + "/src/"
        + variant
        + "/",
    }

    def visit(v):
        if isinstance(v, dict):
            return {k: visit(x) for k, x in v.items()}
        if isinstance(v, list):
            return [visit(x) for x in v]
        if isinstance(v, str):
            for before, after in mappings.items():
                v = v.replace(before, after)
            if str(root) in v:
                raise ValueError("Unnormalized KSP source location")
        return v

    return visit(data)


def collect(config, root, inputs, capture):
    generated = root / "probe/build/generated/ksp/ohosArm64/ohosArm64Main"
    resources = generated / "resources"
    facts = portable_facts(support.read_json(resources / "frontend-facts.json"), root, config)
    schema = portable_facts(support.read_json(resources / "record-schema.json"), root, config)
    policy = support.read_json(
        support.input_path(inputs, project.SDK + "/frontend-policy.json")
    )
    expected = policy["functions"] + (
        policy["development_test_functions"] if config["kotlin"]["test_support"] else []
    )
    if sorted(f["name"] for f in facts["functions"]) != sorted(expected):
        raise ValueError("Actual KSP exports differ from the selected Kotlin role policy")
    if facts["record"]["fields"] != [
        {"name": f["name"], "kotlinType": f["kotlin_type"]} for f in policy["record"]["fields"]
    ]:
        raise ValueError("Actual KSP record fields differ from the owned Snapshot profile")
    for interface in facts["interfaces"]:
        if any(f["returnType"] != "kotlin.Unit" for f in interface["methods"]):
            raise ValueError("Return-valued notification is unsupported")
    declaration = (resources / "record-export.d.ts").read_text()
    parsed = support.command(
        config,
        "node",
        [
            support.input_path(inputs, "adapters/generators/arkts/read_kotlin_exports.cjs"),
            support.verify_tool(config["tools"]["typescript"]),
        ],
        cwd=root,
        stdin=json.dumps(dict(facts=facts, declaration=declaration)).encode(),
        capture=capture,
    )
    descriptor = json.loads(parsed)
    outputs = []
    for p in sorted(generated.rglob("*")):
        if p.is_file():
            text = p.read_text()
            if p.suffix == ".json":
                text = (
                    json.dumps(
                        portable_facts(json.loads(text), root, config),
                        sort_keys=True,
                        separators=(",", ":"),
                    )
                    + "\n"
                )
            if str(root) in text:
                raise ValueError("Generated KSP output includes an unnormalized workspace path")
            outputs.append(
                dict(
                    path=p.relative_to(generated).as_posix(),
                    text=text,
                    sha256="sha256:" + hashlib.sha256(text.encode()).hexdigest(),
                    size=len(text.encode()),
                )
            )
    return dict(
        authority="actual CPF Kotlin KSP1 and akinterop 1.1.0; managed single-record profile",
        facts=facts,
        frontend_policy=policy,
        record_schema=schema,
        declaration=declaration,
        descriptor=descriptor,
        generated=outputs,
        test_support=config["kotlin"]["test_support"],
        typed_api_inferred=False,
        frontend_compile_scope="matched native helper and executor archives, cinterop and managed runtime KLIB; no application shared-library link",
        source_inputs=config["kotlin"]["source_inputs"],
        tools={
            k: {n: config["tools"][k][n] for n in ("sha256", "size", "version")}
            for k in ("cpf_java", "gradle_cli", "cpf_clang", "cpf_ar", "node", "typescript")
        },
        closure=dict(
            maven=config["kotlin"]["dependency_lock"]["sha256"],
            trees={
                **{k: v["summary"] for k, v in config["kotlin"]["trees"].items()},
                "maven": config["kotlin"]["maven_summary"],
            },
        ),
    )


def workspace_key(inputs, config, phase, identity=None):
    workspace = support.load_module(_ROOT / "adapters/toolchains/common/compiler_workspace.py")
    observation = dict(
        tools=config["tools"],
        trees={
            **{k: v["summary"] for k, v in config["kotlin"]["trees"].items()},
            "maven": config["kotlin"]["maven_summary"],
        },
        xcode={
            "required": False,
            "reason": "CPF standalone LLVM/sysroots; no Apple platform SDK compilation",
        },
    )
    key = workspace.identity(
        config["kotlin"]["source_inputs"],
        observation,
        identity["profile_digest"] if identity else phase,
        identity["abi_digest"] if identity else phase,
    )
    return workspace, key


def read_metadata(inputs, config, work, public_descriptor, *, capture=None):
    inputs, work = Path(inputs), Path(work)
    own_capture = capture is None
    capture = [] if own_capture else capture
    try:
        dependencies.verify(inputs, config)
        workspace, key = workspace_key(inputs, config, "kotlin-metadata")
        with workspace.acquire(
            key, base=Path(config["kotlin"]["compiler_workspace"]["root"])
        ) as stage:
            root = project.materialize(inputs, config, stage / "project")
            project.compile_native(inputs, config, root, capture)
            project.gradle(config, root, workspace, [":probe:kspKotlinOhosArm64"], capture)
            result = collect(config, root, inputs, capture)
            support.verify_refs(inputs, config["kotlin"]["source_inputs"])
            dependencies.verify(inputs, config)
            # Public descriptor is independently parsed, never used to invent KSP facts.
            return result
    finally:
        if own_capture:
            # Reuse the public driver's existing diagnostic transport. Failure
            # keeps the same raw commands in its retained private scratch.
            logdir = work / "port-commands"
            logdir.mkdir(exist_ok=True)
            index = 0
            for entry in capture:
                while (logdir / f"kotlin-{index:04}.json").exists():
                    index += 1
                with (logdir / f"kotlin-{index:04}.json").open("x") as output:
                    json.dump(entry, output, indent=2)
                index += 1
