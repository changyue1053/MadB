"""Explicit local C07 ordinary profile/2 generation, without execution authority.

The fixed host bridge binds the actual composition inputs before completing IR.
The old driver/finalizer remains profile/1-only. This entry is not an E02 session,
C04 execution receipt, Source commit, native build or SDK certification.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile

_spec = importlib.util.spec_from_file_location(
    "_c07_composition_support", Path(__file__).with_name("support.py")
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)
driver = support.load_module(Path(__file__).with_name("driver.py"))


def _profile(request, config, root):
    if (config.get("provider"), config.get("variant")) != ("c", "ordinary"):
        raise ValueError("composition generator supports only fixed ordinary C/ArkTS")
    if config.get("project") != "project.json" or config.get("profile") != "profile.json":
        raise ValueError("composition bridge requires its fixed document layout")
    if request["profile"]["path"] != "profile.json" or request["profile"]["schema"] != "madb.profile/2":
        raise ValueError("composition generator requires an explicit profile/2 reference")
    if request["lock"]["path"] != "lock.json":
        raise ValueError("composition bridge requires lock.json")
    support.verify_refs(root, [request["profile"], request["lock"]])
    profile = support.read_json(support.input_path(root, "profile.json"))
    if profile["schema"] != "madb.profile/2" or "core.extension-composition@1" not in profile["required_features"]:
        raise ValueError("composition profile must require the adopted composition feature")
    name = profile["id"]
    modes = {"c07-c-arkts-composition-aarch64-ohos23": "ordinary", **{
        f"c07-c-arkts-{recipe}-{mode}-aarch64-ohos23": mode
        for recipe in ("chain", "source-semantics")
        for mode in ("ordinary", "enhanced", "disabled")
    }}
    if name not in modes:
        raise ValueError("unsupported fixed composition profile")
    # This guard separates entry points; full format, pins and fixed recipe
    # validation belong to the explicitly bound native bridge below.
    return modes[name]


def _complete(request, config, root, work):
    mode = _profile(request, config, root)
    _optional_sources(config, root, work, mode)
    descriptor, seed, _ = driver._read(request, config, root, work)
    completed = support.command(
        config, "finalizer", ["metadata", Path(root).absolute()], cwd=work,
        stdin=driver.documents(root).canonical(seed).encode(),
        maximum=support.MAX_JSON,
    )
    return descriptor, json.loads(completed)


OPTIONAL = (
    ("sdk/arkts/observation/StateObservation.ets", "1e26f847c8e37d7c65b236188d86c6ee3ad6dcc53c033d4304049a7ab7531534", "state"),
    ("sdk/arkts/observation/ArkUIStateModel.ets", "7e62d7352ded16641e560f9230b65576d524caa8ac847dc65715e7e877ad77ef", "ui"),
)

def _optional_sources(config, root, work, mode):
    if mode != "enhanced":
        return []
    result = []
    for name, digest, kind in OPTIONAL:
        source = support.input_path(root, name)
        data = source.read_bytes()
        if support.sha(source).removeprefix("sha256:") != digest:
            raise ValueError("unsupported optional SDK source: " + name)
        support.command(config, "node", [Path(__file__).with_name("read_observation.cjs"), config["tools"]["typescript"]["path"], name, kind], cwd=work, stdin=data)
        result.append(("generated/facade/" + source.name, data))
    return result


def metadata(request, config, root, work):
    return _complete(request, config, root, work)[1]


def generate(request, config, root, work):
    descriptor, contract = _complete(request, config, root, work)
    value = driver._generate_from_metadata(request, config, root, work, descriptor, contract)
    optional = _optional_sources(config, root, work, _profile(request, config, root))
    for name, data in optional:
        destination = Path(work) / name
        with destination.open("xb") as stream:
            stream.write(data)
        ref = driver.documents(root).ref(work, name)
        ref["sources"] = ["madb.c07.Engine.observeState.v1", "madb.c07.Subscription.close.v1", "madb.c07.Subscription.requestStop.v1"]
        value["files"].append(ref)
    if optional:
        value["files"].sort(key=lambda f: f["path"])
        value["generation_id"] = driver.documents(root).semantic("madb.generation/1", {k: value[k] for k in ("input_digest","task_digest","profile_digest","files")})
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("metadata", "generate"))
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    raw = sys.stdin.buffer.read(support.MAX_JSON + 1)
    if len(raw) > support.MAX_JSON:
        raise ValueError("request exceeds 1 MiB")
    request = json.loads(raw)
    work = Path.cwd()
    root = work.parent / "inputs"
    config = support.read_json(args.config)
    scratch = Path(tempfile.mkdtemp(prefix="private-composition-driver-", dir=work))
    value = (metadata if args.operation == "metadata" else generate)(request, config, root, scratch)
    data = (driver.documents(root).canonical(value) + "\n").encode()
    if len(data) > support.MAX_JSON:
        raise ValueError("response exceeds 1 MiB")
    support.publish_diagnostics(scratch)
    if args.operation == "generate":
        support.verify_refs(scratch, value["files"])
        for item in value["files"]:
            support.copy_new(support.input_path(scratch, item["path"]), work / item["path"])
    shutil.rmtree(scratch)
    name = "contract.json" if args.operation == "metadata" else "outputs.json"
    with (work / name).open("xb") as output:
        output.write(data)
    sys.stdout.buffer.write(data)


if __name__ == "__main__":
    main()
