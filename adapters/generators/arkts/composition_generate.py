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
    # This guard separates entry points; full format, pins and fixed recipe
    # validation belong to the explicitly bound native bridge below.


def _complete(request, config, root, work):
    _profile(request, config, root)
    descriptor, seed, _ = driver._read(request, config, root, work)
    completed = support.command(
        config, "finalizer", ["metadata", Path(root).absolute()], cwd=work,
        stdin=driver.documents(root).canonical(seed).encode(),
        maximum=support.MAX_JSON,
    )
    return descriptor, json.loads(completed)


def metadata(request, config, root, work):
    return _complete(request, config, root, work)[1]


def generate(request, config, root, work):
    descriptor, contract = _complete(request, config, root, work)
    return driver._generate_from_metadata(request, config, root, work, descriptor, contract)


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
