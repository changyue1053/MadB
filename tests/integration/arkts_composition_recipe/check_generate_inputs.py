"""Construct untrusted real inputs for the independently fixed Rust probe recipe.

This test builder does not serialize host requirements or an acceptance flag.
No v1 profile, IR or OutputSet is renamed into v2.
"""
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import driver, support
from adapters.toolchains.arkts import native, project
from adapters.toolchains.common import documents as d

PACKAGE = "madb.local.c07-composition"
VERSION = "0.1.0"
PREFIX = f"packages/{PACKAGE}/{VERSION}"
COMPONENT = PACKAGE + "/generator"
PROFILE = "c07-c-arkts-composition-aarch64-ohos23"
RECIPE = "madb.local.c07-c-arkts/ordinary@1"
FEATURE = "core.extension-composition@1"
ENTRY = "adapters/generators/arkts/composition_generate.py"
PAYLOADS = sorted([
    "identity.json", "declarations.json", "generated/contract/runtime-policy.json",
    "generated/contract/frame-layout.json", "generated/contract/inventory-format.json",
    "generated/notices/native-NOTICE.txt", "generated/notices/llvm-NOTICE.txt",
    "generated/facade/Boundary.ets", "generated/facade/RuntimeFrame.ets",
    "generated/native/exports.map", "generated/native/Index.d.ts",
    "generated/native/exports.generated.inc", "generated/keep-rules.txt",
    "generated/native/frame.generated.hpp", "generated/native/identity.generated.hpp",
])
POLICY = "adapters/generators/arkts/profiles/snapshot-v1.json"
AUTHORITY = "docs/adr/0013-arkts-runtime-and-completion.md"
NATIVE = "sdk/arkts/native-runtime/src/protocol.cpp"


def bridge_command(bridge, operation, root, *extra, data=None):
    result = subprocess.run([str(bridge), operation, str(root), *map(str, extra)],
                            input=data, capture_output=True, timeout=60,
                            env=support.clean_environment())
    if len(result.stdout) > support.MAX_JSON or len(result.stderr) > support.MAX_JSON:
        raise ValueError("bridge output exceeds budget")
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def source_schema(name):
    return {"composition.json": "madb.composition/1",
            "toolchain.json": "madb.native-toolchain/1"}.get(name, "madb.source-input/1")


def provided_roles():
    """Claims to be checked against Rust's separately fixed required set."""
    call = "madb.c07.Engine.roundtrip.v1"
    observe = "madb.c07.Engine.observeState.v1"
    stop = "madb.c07.Subscription.requestStop.v1"
    close = "madb.c07.Subscription.close.v1"
    claims = []
    for subject, stage, scope, anchor, layer, owner in [
        (call, "projection", "call", call, "facade", "facade"),
        (observe, "subscription", "subscription", observe, "source", "native-runtime"),
        (stop, "stop", "subscription", observe, "source", "native-runtime"),
        (close, "drain", "subscription", observe, "source", "native-runtime"),
        (close, "release", "subscription", observe, "facade", "facade"),
    ]:
        claims.append({"boundary": "c-arkts", "subject": {"kind": "public-symbol", "public_id": subject},
                       "stage": stage, "scope": {"kind": scope, "anchor": {"kind": "public-symbol", "public_id": anchor}, "layer": layer},
                       "owner": {"kind": "sdk-asset", "package": PACKAGE, "asset": owner}})
    return claims


def prepare(root, observation_path, bridge):
    """Build a new isolated tree from current source and real installed tool pins.

    A saved observation is only input: the production reader subsequently checks
    its installed inventory/runtime trees and every image before and after use.
    """
    root = Path(root).absolute()
    if root.exists() or root.is_symlink():
        raise ValueError("fixture input root must be new")
    root.mkdir(parents=True)
    observation = support.read_json(observation_path, 64 * support.MAX_JSON)
    config = deepcopy(observation["config"])
    bridge = Path(bridge).resolve(strict=True)
    config["tools"]["finalizer"] = {
        "path": str(bridge), "id": "c07-finalizer", "version": "0.1.0",
        "sha256": support.sha(bridge), "size": bridge.stat().st_size,
        "source": "locally built fixed composition test root; exact image, not installed extension admission",
    }
    source_names = set(project.HELPERS) | set(project.SHARED) | {
        p.relative_to(ROOT).as_posix() for p in native.source_paths(ROOT, route="c", variant="ordinary")
    } | {ENTRY, POLICY, AUTHORITY, "adapters/generators/arkts/composition_recipe.py",
         "adapters/generators/arkts/read_observation.cjs", ".node-version"}
    for name in sorted(source_names):
        source = support.input_path(ROOT, name)
        expected = d.ref(ROOT, name)
        support.copy_new(source, root / name)
        support.verify_refs(ROOT, [expected])
        support.verify_refs(root, [expected])
    # Package originals retain their own local paths. Host logical copies used
    # by the existing reader are separately frozen with exactly these bytes.
    for name in sorted(source_names):
        support.copy_new(root / name, root / PREFIX / name)
    derived = root / "derived/native"
    composed = native.compose_native(root, derived, route="c", variant="ordinary")
    names = source_names | {f.path.relative_to(root).as_posix() for f in composed.derived_files}
    support.copy_new(root / POLICY, root / "runtime-policy.json")
    names.add("runtime-policy.json")
    inventory_ref = observation["inventory"]
    inventory = support.read_json(support.verify_tool(inventory_ref), 64 * support.MAX_JSON)
    support.copy_new(inventory_ref["path"], root / "tools/cli-inventory.json")
    names.add("tools/cli-inventory.json")
    notices = []
    for sdk_path, filename in [
        ("sdk/default/openharmony/native/NOTICE.txt", "native-NOTICE.txt"),
        ("sdk/default/openharmony/native/llvm/NOTICE", "llvm-NOTICE.txt"),
    ]:
        entries = [e for e in inventory["entries"] if e["path"] == sdk_path and e["kind"] == "file"]
        if len(entries) != 1:
            raise ValueError("fixed SDK notice inventory differs")
        name = "third-party/harmony/" + filename
        support.copy_new(Path(config["cli_root"]) / sdk_path, root / name)
        support.verify_refs(root, [{"path": name, "sha256": "sha256:" + entries[0]["sha256"], "size": entries[0]["size"]}])
        names.add(name)
        notices.append({"source": name, "sdk_path": sdk_path})
    config.update(schema="madb.private.c07-native-backend/1", provider="c", variant="ordinary",
                  boundary_id="c-arkts", project="project.json", profile="profile.json",
                  policy="runtime-policy.json", package_name="madbc07sdkc", source_package="madbc07sdkc",
                  provider_factory="mp_c_create", native_module="libmadbc07sdkc.so",
                  public_template="derived/native/Boundary.ets.in",
                  public_source="derived/Boundary.declarations.ets",
                  gateway_source="derived/native/Index.d.ts",
                  provider_source="sdk/arkts/native-runtime/include/provider_abi.h",
                  runtime_frame="sdk/arkts/facade/RuntimeFrame.ets", frame_layout="sdk/arkts/facade/frame-layout.json",
                  cli_inventory="tools/cli-inventory.json", runtime_notices=notices,
                  delivery_inputs=[], additional_notices=[], features=driver.lowering.FEATURES,
                  phase_timeout_seconds=900, phase_termination_grace_ms=6000,
                  composition={"route": "c", "variant": "ordinary", "prefix": "derived/native",
                               "original_inputs": [p.relative_to(root).as_posix() for p in native.source_paths(root, route="c", variant="ordinary")],
                               "derived_inputs": [f.path.relative_to(root).as_posix() for f in composed.derived_files]})
    projection = driver.public_projection(root, config, "madb-declaration-projection-not-runtime")
    (root / config["public_source"]).write_bytes(projection)
    names.add(config["public_source"])
    public_sdk = deepcopy(observation["public"])
    public_sdk["schema"] = "madb.native-toolchain/1"
    public_sdk["tools"] = [{k: t[k] for k in ("id", "version", "sha256", "size", "source")}
                           for _, t in sorted(config["tools"].items())]
    d.save(root, "toolchain.json", public_sdk)
    d.save(root, "backend.json", config)
    names |= {"toolchain.json", "backend.json"}
    manifest = {"schema": "madb.extension/1", "id": PACKAGE, "version": VERSION,
                "compatibility": {"protocol": ["1.0"], "ir": ["1.0"]}, "languages": [],
                "components": [{"id": "generator", "port": "generator", "supported_features": sorted([*driver.lowering.FEATURES, FEATURE]),
                                "entrypoint": {"kind": "process", "path": ENTRY, "args": []}}],
                "dependencies": [], "routes": [], "assets": [{"kind": "sdk", "path": "sdk"}],
                "execution": {"transport": "stdio-jsonl", "network": "none", "env": []}}
    d.save(root / PREFIX, "extension.json", manifest)
    package_paths = sorted(source_names | {"extension.json"})
    package_files = [d.ref(root / PREFIX, p) for p in package_paths]
    package_digest = d.semantic("madb.source/1", package_files)
    component = {"id": COMPONENT, "version": VERSION, "sha256": package_digest}
    names |= {PREFIX + "/" + p for p in package_paths}
    selection = {"profile_id": PROFILE, "route": "ARKTS-C-NAPI", "direction": "consumer-to-provider", "target": "aarch64-linux-ohos"}
    composition = {"schema": "madb.composition/1", "id": "c07-c-arkts-ordinary-composition", "recipe": RECIPE,
                   "selection": selection, "packages": [{"id": PACKAGE, "version": VERSION,
                       "manifest": d.ref(root / PREFIX, "extension.json", "madb.extension/1"),
                       "source": {"kind": "file-set", "files": package_files}, "source_digest": package_digest}],
                   "components": [component], "assets": [
                       {"id": "facade", "package": PACKAGE, "source": {"kind": "generated", "producer": COMPONENT, "output_slot": "ordinary-source", "paths": PAYLOADS}},
                       {"id": "native-runtime", "package": PACKAGE, "source": {"kind": "source", "files": [d.ref(root / PREFIX, NATIVE)]}}],
                   "dependencies": [{"from": "facade", "to": "native-runtime", "scope": "target-runtime"}],
                   "responsibilities": provided_roles()}
    d.save(root, "composition.json", composition)
    names.add("composition.json")
    composition_digest = bridge_command(bridge, "composition-digest", root).decode().strip()
    public_tools = [{**{k: t[k] for k in ("id", "version", "sha256")}, "host": "aarch64-apple-darwin",
                     "target": selection["target"], "source": t["source"], "arguments": t.get("invocation_arguments", [])}
                    for _, t in sorted(config["tools"].items())]
    profile = dict(schema="madb.profile/2", id=PROFILE, route=selection["route"],
                   provider={"language": "c", "runtime": "native"}, consumer={"language": "arkts", "runtime": "harmonyos"},
                   direction=selection["direction"], target=selection["target"], abi="harmonyos-napi8-c07-snapshot",
                   artifact_kind="har", build_system="clang-hvigor", deployment_target="API23", tools=public_tools,
                   sdk=d.ref(root, "toolchain.json", "madb.native-toolchain/1"), macros=[],
                   required_features=sorted([*driver.lowering.FEATURES, FEATURE]), supported_features=sorted([*driver.lowering.FEATURES, FEATURE]),
                   restrictions=[], defaults={"values": {"ownership": "copy"}, "origin": {"authority": "profile-default", "id": "c07.copy-values", "location": {"path": "profile.json", "line": 1, "column": 1}}},
                   backend_config=None, composition={"id": composition["id"], "file": d.ref(root, "composition.json", "madb.composition/1"), "semantic_digest": composition_digest})
    d.save(root, "profile.json", profile)
    profile_digest = bridge_command(bridge, "profile-digest", root).decode().strip()
    selector = dict(selection, profile_digest=profile_digest)
    inputs = [d.ref(root, n, source_schema(n)) for n in sorted(names)]
    preliminary = {"schema": "madb.read-request/1", "boundary_id": "c-arkts", "selector": selector, "inputs": inputs}
    scratch = root.parent / "prepare-read"
    scratch.mkdir()
    _, _, policies = driver._read(preliminary, config, root, scratch)
    project_doc = {"schema": "madb.project/1", "project": "madb-local-c07-composition", "sdk_namespace": "madb",
                   "boundaries": [{"id": "c-arkts", "route": selection["route"], "direction": selection["direction"],
                                   "source": {"kind": "arkts-compiler-declarations", "path": config["public_source"]},
                                   "profile": d.ref(root, "profile.json", "madb.profile/2"), "target": selection["target"], "output": "generated"}],
                   "policies": policies, "policy_families": [], "artifacts": {"package": "madbc07sdkc", "manifest": "candidate.json"}}
    d.save(root, "project.json", project_doc)
    schemas = []
    for name in ("common", "project", "lock", "profile-v2", "composition", "extension-manifest", "contract-ir", "read-request", "generate-request", "output-set"):
        name = "schemas/" + name + ".schema.json"
        support.copy_new(ROOT / "docs" / name, root / name)
        schemas.append(d.ref(root, name))
    lock = {"schema": "madb.lock/1", "project_digest": d.project_digest(project_doc),
            "profiles": [{"id": PROFILE, "semantic_digest": profile_digest, "file": d.ref(root, "profile.json", "madb.profile/2")}],
            "tools": public_tools, "components": [component],
            "inputs": [{k: r[k] for k in ("path", "sha256", "size")} for r in inputs], "schemas": schemas}
    d.save(root, "lock.json", lock)
    read = dict(preliminary, component=component, lock=d.ref(root, "lock.json", "madb.lock/1"),
                profile=d.ref(root, "profile.json", "madb.profile/2"), output_root="metadata-step")
    d.save(root, "read-request.json", read)
    support.copy_new(config["tools"]["python"]["path"], root / "tools/python")
    return config, read
