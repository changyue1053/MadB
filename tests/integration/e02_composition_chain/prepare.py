"""Untrusted input assembly for the fixed two-session composition root.

The reused fixture reads declarations to assemble project policies. It does not
produce finalized IR: contract.json and GenerateRequest are deliberately absent
until the actual public reader session has returned its checked result.
"""
from copy import deepcopy
from pathlib import Path
import hashlib
import shutil
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/integration/arkts_composition_recipe"))
import check_generate_inputs as base
from adapters.toolchains.common import documents as d

ENTRY = "crates/madb-cli/examples/arkts_composition_peer.rs"
PEER_SOURCES = (
    ENTRY, "crates/madb-cli/examples/arkts_composition_peer/business.rs",
    "adapters/generators/arkts/composition_peer.py",
)
OPTIONAL = {
    "state": ("sdk/arkts/observation/StateObservation.ets", "1e26f847c8e37d7c65b236188d86c6ee3ad6dcc53c033d4304049a7ab7531534"),
    "arkui": ("sdk/arkts/observation/ArkUIStateModel.ets", "7e62d7352ded16641e560f9230b65576d524caa8ac847dc65715e7e877ad77ef"),
}
MODES = ("ordinary", "enhanced", "disabled")
READER = base.PACKAGE + "/reader"

# These are deliberately rejected mutations of the actual C07 baseline. They
# are not X03/X07/X08 manifests, resolved profiles or supported backends.
ADMISSION_GATES = {
    "unvalidated-backend-config": ("CAP005", "backend_config"),
    "unselected-real-recipe": ("LOCK001", "selection"),
    "missing-component-closure": ("InputInvalid", "components"),
    "missing-entrypoint-source": ("FILE003", base.COMPONENT),
    "changed-execution-image": ("LOCK004", "image"),
}


def optional_roles():
    claims = []
    for asset, member, stage, anchor, layer in (
        ("state", "StateObservation.requestStop", "stop", "StateObservation.attach", "receiver"),
        ("state", "StateObservation.close", "async-bridge", "StateObservation.attach", "receiver"),
        ("arkui", "ArkUIStateModel.attach", "projection", "ArkUIStateModel.attach", "view-model"),
    ):
        claims.append({"boundary": "c-arkts", "subject": {"kind": "sdk-export", "asset": asset, "name": member},
                       "stage": stage, "scope": {"kind": "receiver", "anchor": {"kind": "sdk-export", "asset": asset, "name": anchor}, "layer": layer},
                       "owner": {"kind": "sdk-asset", "package": base.PACKAGE, "asset": asset}})
    return claims


def repin(root, bridge, read):
    """Update byte and semantic links after a deliberate untrusted mutation."""
    composition = base.support.read_json(root / "composition.json")
    package = composition["packages"][0]
    package["manifest"] = d.ref(root / base.PREFIX, "extension.json", "madb.extension/1")
    package["source"]["files"] = [d.ref(root / base.PREFIX, f["path"]) for f in package["source"]["files"]]
    package["source_digest"] = d.semantic("madb.source/1", package["source"]["files"])
    for component in composition["components"]:
        component["sha256"] = package["source_digest"]
    d.save(root, "composition.json", composition)
    profile = base.support.read_json(root / "profile.json")
    profile["composition"] = {"id": composition["id"], "file": d.ref(root, "composition.json", "madb.composition/1"),
                              "semantic_digest": base.bridge_command(bridge, "composition-digest", root).decode().strip()}
    d.save(root, "profile.json", profile)
    profile_digest = base.bridge_command(bridge, "profile-digest", root).decode().strip()
    project = base.support.read_json(root / "project.json")
    project["boundaries"][0]["profile"] = d.ref(root, "profile.json", "madb.profile/2")
    d.save(root, "project.json", project)
    read = deepcopy(read)
    read["inputs"] = [d.ref(root, f["path"], f["schema"]) for f in read["inputs"]]
    read["selector"] = dict(composition["selection"], profile_digest=profile_digest)
    read["component"] = next(c for c in composition["components"] if c["id"] == READER)
    lock = base.support.read_json(root / "lock.json")
    lock.update(project_digest=d.project_digest(project), tools=profile["tools"], components=composition["components"],
                inputs=[{k: f[k] for k in ("path", "sha256", "size")} for f in read["inputs"]],
                profiles=[{"id": profile["id"], "semantic_digest": profile_digest,
                           "file": d.ref(root, "profile.json", "madb.profile/2")}])
    d.save(root, "lock.json", lock)
    read.update(lock=d.ref(root, "lock.json", "madb.lock/1"), profile=d.ref(root, "profile.json", "madb.profile/2"))
    d.save(root, "read-request.json", read)
    return read


def prepare(args, root, mode):
    if mode not in MODES:
        raise ValueError("unknown fixture mode")
    _, read = base.prepare(root, args.observation, args.bridge)
    added = list(PEER_SOURCES)
    if mode == "enhanced":
        for name, digest in OPTIONAL.values():
            if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != digest:
                raise ValueError("fixed optional source differs")
            added.append(name)
    for name in added:
        base.support.copy_new(ROOT / name, root / name)
        base.support.copy_new(ROOT / name, root / base.PREFIX / name)
    manifest = base.support.read_json(root / base.PREFIX / "extension.json")
    feature_names = manifest["components"][0]["supported_features"]
    manifest["components"] = [
        {"id": name, "port": port, "supported_features": feature_names,
         "entrypoint": {"kind": "process", "path": ENTRY, "args": [operation]}}
        for name, port, operation in (("reader", "contract-reader", "read"), ("generator", "generator", "generate"))
    ]
    d.save(root / base.PREFIX, "extension.json", manifest)
    composition = base.support.read_json(root / "composition.json")
    profile_id = f"c07-c-arkts-chain-{mode}-aarch64-ohos23"
    composition.update(id=f"c07-c-arkts-chain-{mode}-composition", recipe=f"madb.local.c07-c-arkts/chain-{mode}@1")
    composition["selection"]["profile_id"] = profile_id
    package = composition["packages"][0]
    package_names = sorted({f["path"] for f in package["source"]["files"]} | set(added))
    package["source"]["files"] = [d.ref(root / base.PREFIX, p) for p in package_names]
    composition["components"].append(dict(composition["components"][0], id=READER))
    if mode == "enhanced":
        composition["assets"][0]["source"]["paths"] = sorted([*base.PAYLOADS, *(
            "generated/facade/" + Path(name).name for name, _ in OPTIONAL.values())])
        for asset, (name, _) in OPTIONAL.items():
            composition["assets"].append({"id": asset, "package": base.PACKAGE,
                                          "source": {"kind": "source", "files": [d.ref(root / base.PREFIX, name)]}})
        composition["dependencies"].extend([
            {"from": "state", "to": "facade", "scope": "target-runtime"},
            {"from": "arkui", "to": "state", "scope": "target-runtime"},
        ])
        composition["responsibilities"].extend(optional_roles())
    d.save(root, "composition.json", composition)
    image = args.peer.read_bytes()
    tool_digest = "sha256:" + hashlib.sha256(image).hexdigest()
    profile = base.support.read_json(root / "profile.json")
    profile["id"] = profile_id
    profile["tools"].extend([
        {"id": "madb-arkts-composition-" + name, "version": "0.1.0", "sha256": tool_digest,
         "host": "aarch64-apple-darwin", "target": "aarch64-linux-ohos",
         "source": "local-fixed-composition-source", "arguments": [operation]}
        for name, operation in (("reader", "read"), ("generator", "generate"))
    ])
    d.save(root, "profile.json", profile)
    schemas = {f["path"]: f["schema"] for f in read["inputs"]}
    schemas["project.json"] = "madb.project/1"
    schemas.update({p: "madb.source-input/1" for name in added for p in (name, base.PREFIX + "/" + name)})
    read["inputs"] = [d.ref(root, p, schema) for p, schema in sorted(schemas.items())]
    read["output_root"] = "outputs/read-1"
    repin(root, args.bridge, read)
    tools = root.parent / "tools"
    tools.mkdir()
    shutil.copy2(args.peer, tools / "arkts-peer")
    if (root / "contract.json").exists() or (root / "generate-request.json").exists():
        raise AssertionError("fixture must not manufacture reader or generator results")
    return tools


def prepare_rejection(args, baseline_root, baseline_tools, output, gate):
    if gate not in ADMISSION_GATES:
        raise ValueError("unknown fixed admission gate")
    root = output / "inputs"
    shutil.copytree(baseline_root, root)
    read = base.support.read_json(root / "read-request.json")
    composition = base.support.read_json(root / "composition.json")
    tools = baseline_tools
    if gate == "unvalidated-backend-config":
        profile = base.support.read_json(root / "profile.json")
        schema = base.support.read_json(root / "backend.json")["schema"]
        profile["backend_config"] = d.ref(root, "backend.json", schema)
        d.save(root, "profile.json", profile)
    elif gate == "unselected-real-recipe":
        composition["recipe"] = base.RECIPE
    elif gate == "missing-component-closure":
        composition["components"] = [c for c in composition["components"] if c["id"] != base.COMPONENT]
    elif gate == "missing-entrypoint-source":
        package = composition["packages"][0]
        package["source"]["files"] = [f for f in package["source"]["files"] if f["path"] != ENTRY]
        qualified = base.PREFIX + "/" + ENTRY
        (root / qualified).unlink()
        read["inputs"] = [f for f in read["inputs"] if f["path"] != qualified]
    else:
        tools = output / "tools"
        shutil.copytree(baseline_tools, tools)
        path = tools / "arkts-peer"
        changed = bytearray(path.read_bytes())
        changed[-1] ^= 1
        path.chmod(path.stat().st_mode | 0o200)
        path.write_bytes(changed)
    d.save(root, "composition.json", composition)
    repin(root, args.bridge, read)
    return root, tools
