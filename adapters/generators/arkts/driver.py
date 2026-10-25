"""Real C07 C04 metadata/generation: public AST + C seam + authored policy."""

from __future__ import annotations
import argparse
import importlib.util
import json
import shutil
from pathlib import Path
import sys
import tempfile

_spec = importlib.util.spec_from_file_location("_c07_port", Path(__file__).with_name("support.py"))
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)
lowering = support.load_module(Path(__file__).with_name("lowering.py"))
c_reader = support.load_module(Path(__file__).with_name("read_c_seam.py"))


def documents(root):
    return support.load_module(Path(root) / "adapters/toolchains/common/documents.py")


def render_template(data, module, identity):
    if (
        not isinstance(module, str)
        or not module.startswith("lib")
        or not module.endswith(".so")
        or any(
            c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_."
            for c in module
        )
    ):
        raise ValueError("unsupported native module filename")
    if (
        not isinstance(identity, str)
        or not identity.isascii()
        or "\0" in identity
        or len(identity) > 256
    ):
        raise ValueError("invalid generated identity literal")
    text = data.decode("utf-8")
    for marker, value in [("@MADB_NATIVE_MODULE@", module), ("@MADB_EXPECTED_IDENTITY@", identity)]:
        if text.count(marker) != 1:
            raise ValueError("facade template marker must occur exactly once: " + marker)
        text = text.replace(marker, json.dumps(value))
    return text.encode()


def verify(request, config, root, *, tree=True):
    if config.get("schema") != "madb.private.c07-native-backend/1" or config["boundary_id"] not in (
        "c-arkts",
        "cpp-arkts",
        "rust-arkts",
    ):
        raise ValueError("unsupported C07 native backend")
    if (
        request["selector"]["target"] != "aarch64-linux-ohos"
        or request["selector"]["direction"] != "consumer-to-provider"
    ):
        raise ValueError("C07 target/direction mismatch")
    if (
        request["selector"]["route"] != "ARKTS-" + config["provider"].upper() + "-NAPI"
        or config["boundary_id"] != config["provider"] + "-arkts"
    ):
        raise ValueError("C07 provider/route identity differs")
    if config.get("composition") is not None and (
        config["composition"]["route"] != config["provider"]
        or config["composition"]["variant"] != config["variant"]
    ):
        raise ValueError("native composition differs from selected profile")
    names = support.verify_refs(root, request["inputs"])
    for name in (
        config["public_template"],
        config["public_source"],
        config["gateway_source"],
        config["provider_source"],
        config["policy"],
        config["cli_inventory"],
        config["frame_layout"],
        config["runtime_frame"],
    ):
        if name not in names:
            raise ValueError("declaration/policy/tool inventory omitted from read inputs: " + name)
    for name in config.get("delivery_inputs", []):
        if name not in names:
            raise ValueError("delivery input omitted from source closure: " + name)
    notices = config["runtime_notices"]
    expected_notices = {
        ("third-party/harmony/native-NOTICE.txt", "sdk/default/openharmony/native/NOTICE.txt"),
        ("third-party/harmony/llvm-NOTICE.txt", "sdk/default/openharmony/native/llvm/NOTICE"),
    }
    if len(notices) != 2 or {(n["source"], n["sdk_path"]) for n in notices} != expected_notices:
        raise ValueError("runtime notice inventory differs from the fixed Native SDK recipe")
    if any(n["source"] not in names for n in notices):
        raise ValueError("runtime notice omitted from original read inputs")
    for tool in config["tools"].values():
        support.verify_tool(tool)
    support.load_module(
        Path(__file__).resolve().parents[3] / "adapters/toolchains/arkts/toolchain.py"
    ).verify_runtime(config, root)
    policy = support.read_json(support.input_path(root, config["policy"]))
    if config["variant"] == "test":
        lengths = {
            "testObserve": ("test-frame-layout.json", "total_length"),
            "testInspect": ("test-frame-layout.json", "total_length"),
            "testLastAdmissionAudit": ("test-frame-layout.json", "total_length"),
            "testObserveOperation": ("test-frame-layout.json", "operation_fields"),
            "testObserveFacade": ("facade-frame-layout.json", "length"),
            "testBootstrapTotals": ("bootstrap-totals-layout.json", "length"),
            "testDeliveryGate": ("ports-layout.json", "delivery"),
        }
        for method, (filename, key) in lengths.items():
            name = "sdk/arkts/test-support/contracts/" + filename
            if name not in names:
                raise ValueError("test layout omitted from source inputs")
            layout = support.read_json(support.input_path(root, name))[key]
            size = (
                len(layout)
                if isinstance(layout, list)
                else (layout["length"] if isinstance(layout, dict) else layout)
            )
            slot = policy["value_slots"][method + ".return"]
            if (
                slot["layout"] != name
                or slot["length_elements"] != size
                or policy["test_types"][slot["type_id"]]["max_bytes"] != size * 8
            ):
                raise ValueError(
                    "test logical array bound differs from actual frozen frame: " + method
                )
    authority = policy["authority"]
    support.verify_refs(root, [authority])
    if authority["path"] not in names:
        raise ValueError("explicit policy authority omitted from read inputs")
    expected = render_template(
        support.input_path(root, config["public_template"]).read_bytes(),
        config["native_module"],
        "madb-declaration-projection-not-runtime",
    )
    if support.input_path(root, config["public_source"]).read_bytes() != expected:
        raise ValueError("public declaration projection differs from authored template")
    if tree:
        inventory = support.read_json(
            support.input_path(root, config["cli_inventory"]), 64 * support.MAX_JSON
        )
        verifier = support.load_module(Path(root) / "adapters/toolchains/arkts/tools.py")
        verifier.verify_inventory(inventory, Path(config["cli_root"]))
    return {r["path"]: r for r in request["inputs"]}


def _read(request, config, root, work):
    refs = verify(request, config, root)
    composition = config.get("composition")
    if composition is not None:
        native = support.load_module(Path(root) / "adapters/toolchains/arkts/native.py")
        original = [
            p.relative_to(root).as_posix()
            for p in native.source_paths(
                Path(root), route=composition["route"], variant=composition["variant"]
            )
        ]
        if original != composition["original_inputs"] or not set(original) <= set(refs):
            raise ValueError("native original source closure changed")
        folder = Path(tempfile.mkdtemp(prefix="checked-composition-", dir=work))
        out = folder / "sources"
        value = native.compose_native(
            Path(root), out, route=composition["route"], variant=composition["variant"]
        )
        derived = [
            composition["prefix"] + "/" + f.path.relative_to(out).as_posix()
            for f in value.derived_files
        ]
        if derived != composition["derived_inputs"]:
            raise ValueError("native derivation output closure changed")
        for name, file in zip(derived, value.derived_files):
            if (
                name not in refs
                or refs[name]["sha256"] != "sha256:" + file.sha256
                or refs[name]["size"] != file.size
            ):
                raise ValueError("native derived source differs from original recipe: " + name)
        shutil.rmtree(folder)
    ts = config["tools"]["typescript"]
    node = config["tools"]["node"]
    base = Path(__file__).parent
    public = json.loads(
        support.command(
            config,
            "node",
            [
                base / "read_facade.cjs",
                ts["path"],
                config["public_source"],
                "source-test" if config["variant"] == "test" else "source",
            ],
            cwd=work,
            stdin=support.input_path(root, config["public_source"]).read_bytes(),
        )
    )
    gr = refs[config["gateway_source"]]
    gateway = json.loads(
        support.command(
            config,
            "node",
            [
                base / "read_arkts.cjs",
                root,
                gr["path"],
                gr["sha256"],
                gr["size"],
                ts["path"],
                ts["sha256"],
                ts["size"],
                node["sha256"],
                node["size"],
            ],
            cwd=work,
        )
    )
    seam_config = {
        "tools": {"clang": config["tools"]["clang"]},
        "native_sdk": config["native_sdk"],
        "clang_arguments": config["seam_arguments"],
        "environment": {},
    }
    seam = c_reader.read_header(root, root, refs[config["provider_source"]], seam_config)
    policy = support.read_json(support.input_path(root, config["policy"]))
    seed, policies = lowering.lower(public, gateway, seam, policy, request, config)
    d = documents(root)
    source_refs = sorted(
        [{k: r[k] for k in ("path", "sha256", "size")} for r in request["inputs"]],
        key=lambda x: x["path"],
    )
    seed["source_digest"] = d.semantic("madb.source/1", source_refs)
    descriptor = {
        "public": public,
        "gateway": gateway,
        "c_seam": seam,
        "public_derivation": {
            "template": refs[config["public_template"]],
            "compiler_input": refs[config["public_source"]],
            "native_module": config["native_module"],
            "identity": "declaration-only, not a runtime handshake",
        },
    }
    verify(request, config, root, tree=False)
    return descriptor, seed, policies


def metadata_seed(request, config, root, work):
    Path(work).mkdir(parents=True, exist_ok=True)
    return _read(request, config, root, work)[1]


def finalize(seed, config, root, work):
    d = documents(root)
    value = {
        "contract": seed,
        "project": support.read_json(support.input_path(root, config["project"])),
        "profile": support.read_json(support.input_path(root, config["profile"])),
    }
    return json.loads(
        support.command(config, "finalizer", [], cwd=work, stdin=d.canonical(value).encode())
    )


def metadata(request, config, root, work):
    return finalize(metadata_seed(request, config, root, work), config, root, work)


def generate(request, config, root, work):
    descriptor, seed, _ = _read(request, config, root, work)
    contract = finalize(seed, config, root, work)
    expected = support.read_json(support.input_path(root, request["contract"]["path"]))
    if contract != expected or any(
        request[k] != contract[v]
        for k, v in [
            ("contract_digest", "structure_digest"),
            ("policy_digest", "policy_digest"),
            ("input_digest", "source_digest"),
        ]
    ):
        raise ValueError("fresh declarations/source/policy differ from checked C04 ContractIR")
    d = documents(root)
    files = []

    def emit(name, data):
        path = Path(work) / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as f:
            f.write(data)
        files.append(
            dict(d.ref(work, name), sources=sorted(s["public_id"] for s in contract["symbols"]))
        )

    def emit_json(name, value):
        emit(name, (d.canonical(value) + "\n").encode())

    abi = d.semantic(
        "madb.c07-abi/1",
        {k: lowering.projection(descriptor[k]) for k in ("public", "gateway", "c_seam")},
    )
    # All author/template/recipe/tool-inventory inputs are in source_digest. No
    # artifact hash or later generation_id enters its own compiled identity.
    build = d.semantic(
        "madb.c07-build-inputs/1",
        {
            "source_digest": contract["source_digest"],
            "profile_digest": request["selector"]["profile_digest"],
            "lock_digest": request["lock"]["sha256"],
            "component": request["component"],
        },
    )
    profile = request["selector"]["profile_digest"]
    identity = "madb-arkts/1:" + ":".join(x.removeprefix("sha256:") for x in (abi, profile, build))
    emit_json(
        "identity.json",
        {
            "schema": "madb.arkts-runtime-identity/1",
            "bootstrap_version": 1,
            "abi_digest": abi,
            "profile_digest": profile,
            "build_id": build,
            "expected_identity": identity,
        },
    )
    emit_json("declarations.json", descriptor)
    emit(
        "generated/contract/runtime-policy.json",
        support.input_path(root, config["policy"]).read_bytes(),
    )
    emit(
        "generated/contract/frame-layout.json",
        support.input_path(root, config["frame_layout"]).read_bytes(),
    )
    emit(
        "generated/contract/inventory-format.json",
        support.input_path(root, "adapters/toolchains/arkts/inventory-format.json").read_bytes(),
    )
    for name in config.get("delivery_inputs", []):
        emit("generated/contract/" + name, support.input_path(root, name).read_bytes())
    for notice in config["runtime_notices"]:
        emit(
            "generated/notices/" + Path(notice["source"]).name,
            support.input_path(root, notice["source"]).read_bytes(),
        )
    emit(
        "generated/facade/Boundary.ets",
        render_template(
            support.input_path(root, config["public_template"]).read_bytes(),
            config["native_module"],
            identity,
        ),
    )
    emit(
        "generated/facade/RuntimeFrame.ets",
        support.input_path(root, config["runtime_frame"]).read_bytes(),
    )
    if config["variant"] == "test":
        for name in config["test_contracts"]:
            emit(
                "generated/contract/test/" + Path(name).name,
                support.input_path(root, name).read_bytes(),
            )
        for name in config["test_facade_inputs"]:
            emit("generated/facade/" + Path(name).name, support.input_path(root, name).read_bytes())
        test_layout = support.read_json(
            support.input_path(root, "sdk/arkts/test-support/contracts/test-frame-layout.json")
        )
        fields = test_layout["fields"]
        if list(map(int, fields)) != list(range(54, 102)):
            raise ValueError("test trace layout differs from the frozen 48 fields")
        emit(
            "generated/native/test-frame.generated.hpp",
            (
                "#pragma once\n#include <cstddef>\nnamespace gateway { enum TestFrame : size_t {\n"
                + ",\n".join(v + "=" + str(int(k) - 54) for k, v in fields.items())
                + ",\nTestCount=48\n}; }\n"
            ).encode(),
        )
        refs = {r["path"]: r for r in request["inputs"]}
        emit_json(
            "generated/contract/composition.json",
            {
                "route": config["provider"],
                "variant": "test",
                "ordinary_facade": refs["sdk/arkts/facade/Boundary.ets.in"],
                "original_inputs": [refs[n] for n in config["composition"]["original_inputs"]],
                "derived_inputs": [refs[n] for n in config["composition"]["derived_inputs"]],
                "scope": "strict ordinary baseline plus companion patches; separate source/profile/build identity",
            },
        )
    emit("generated/native/exports.map", b"{ local: *; };\n")
    emit(
        "generated/native/Index.d.ts",
        support.input_path(root, config["gateway_source"]).read_bytes(),
    )
    methods = descriptor["gateway"]["methods"]
    lines = ["napi_property_descriptor methods[] = {"]
    lines += [
        '{"'
        + m["name"]
        + '",nullptr,js_'
        + m["name"]
        + ",nullptr,nullptr,nullptr,napi_default,nullptr},"
        for m in methods
    ]
    emit("generated/native/exports.generated.inc", ("\n".join(lines + ["};"]) + "\n").encode())
    names = {m["name"] for m in methods}
    for key in ("public", "gateway"):
        for record in descriptor[key]["records"]:
            names.update(f["name"] for f in record["fields"])
    for resource in descriptor["public"]["resources"]:
        names.update(m["name"] for m in resource["methods"])
    for error in descriptor["public"]["errors"]:
        names.update(f["name"] for f in error["fields"])
    emit(
        "generated/keep-rules.txt",
        ("-keep-property-name\n" + "\n".join(sorted(names)) + "\n").encode(),
    )
    layout = support.read_json(support.input_path(root, config["frame_layout"]))
    if list(map(int, layout)) != list(range(len(layout))) or len(set(layout.values())) != len(
        layout
    ):
        raise ValueError("frame layout must have contiguous unique slots")
    emit(
        "generated/native/frame.generated.hpp",
        (
            "#pragma once\n#include <cstddef>\nnamespace gateway { enum Frame : size_t {\n"
            + ",\n".join(v + "=" + k for k, v in layout.items())
            + "\n}; }\n"
        ).encode(),
    )
    constants = {
        "GatewayAbi": abi.removeprefix("sha256:"),
        "GatewayProfile": profile.removeprefix("sha256:"),
        "GatewayBuild": build.removeprefix("sha256:"),
        "GatewayIdentity": identity,
        "GatewayRouteIdentity": config["provider"] + "/" + identity,
    }
    constants["GatewayModuleName"] = config["native_module"][3:-3]
    factory = config.get("provider_factory", "mp_c_create")
    if factory not in ("mp_c_create", "mp_cpp_create", "mp_rust_create"):
        raise ValueError("unsupported native provider factory")
    emit(
        "generated/native/identity.generated.hpp",
        (
            '#pragma once\n#include "provider_abi.h"\nnamespace gateway {\n'
            + "".join(
                "inline constexpr const char* " + k + "=" + json.dumps(v) + ";\n"
                for k, v in constants.items()
            )
            + "inline constexpr auto GatewayProviderCreate = ::"
            + factory
            + ";\n}\n"
        ).encode(),
    )
    # Re-read the actual generated facade and require exactly the authored public
    # structure, excluding only source locations/provenance and import module.
    generated = json.loads(
        support.command(
            config,
            "node",
            [
                Path(__file__).with_name("read_facade.cjs"),
                config["tools"]["typescript"]["path"],
                "generated/facade/Boundary.ets",
                "source-test" if config["variant"] == "test" else "source",
            ],
            cwd=work,
            stdin=(Path(work) / "generated/facade/Boundary.ets").read_bytes(),
        )
    )
    if lowering.projection(generated) != lowering.projection(descriptor["public"]):
        raise ValueError("identity rendering changed the public interface")
    verify(request, config, root, tree=False)
    value = {
        "schema": "madb.output-set/1",
        "input_digest": request["input_digest"],
        "task_digest": request["task_digest"],
        "profile_digest": profile,
        "manifest_path": "outputs.json",
        "files": sorted(files, key=lambda x: x["path"]),
    }
    value["generation_id"] = d.semantic(
        "madb.generation/1",
        {k: value[k] for k in ("input_digest", "task_digest", "profile_digest", "files")},
    )
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=["metadata", "generate"])
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    raw = sys.stdin.buffer.read(support.MAX_JSON + 1)
    if len(raw) > support.MAX_JSON:
        raise ValueError("request exceeds 1 MiB")
    request = json.loads(raw)
    root = Path.cwd().parent / "inputs"
    work = Path.cwd()
    config = support.read_json(args.config)
    # Tool scratch and logs are not public Metadata/Generate products. A failed
    # invocation retains its scratch in the failed run for diagnosis.
    scratch = Path(tempfile.mkdtemp(prefix="private-driver-", dir=work))
    value = (metadata if args.operation == "metadata" else generate)(request, config, root, scratch)
    output = (documents(root).canonical(value) + "\n").encode()
    if len(output) > support.MAX_JSON:
        raise ValueError("response exceeds 1 MiB")
    support.publish_diagnostics(scratch)
    if args.operation == "generate":
        support.verify_refs(scratch, value["files"])
        for item in value["files"]:
            support.copy_new(support.input_path(scratch, item["path"]), work / item["path"])
    shutil.rmtree(scratch)
    with (work / ("contract.json" if args.operation == "metadata" else "outputs.json")).open(
        "xb"
    ) as f:
        f.write(output)
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        print("C07 declaration: " + str(error), file=sys.stderr)
        raise SystemExit(2) from error
