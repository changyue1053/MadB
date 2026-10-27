"""Finite business adapter for the public E02 ordinary C/ArkTS generator peer.

Control framing, cancellation and output admission stay with the Rust peer and
its outer Host. This process inherits that Host's process group. It never starts
an independent session or background worker.
"""
from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path
import shutil
import sys
import tempfile

_spec = importlib.util.spec_from_file_location(
    "_c07_composition_peer_support", Path(__file__).with_name("support.py")
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)
generator = support.load_module(Path(__file__).with_name("composition_generate.py"))


def directory(path):
    path = Path(path)
    if not path.is_absolute() or any(part in (".", "..") for part in path.parts):
        raise ValueError("absolute confined directory required")
    for current in (path, *path.parents):
        if current.is_symlink():
            raise ValueError("symbolic directory is not allowed")
    if not path.is_dir():
        raise ValueError("missing directory")
    return path


def generate(input_root, output_root, request_name):
    root, output = directory(input_root), directory(output_root)
    if root == output or root in output.parents or output in root.parents:
        raise ValueError("input and output domains must be separate")
    if list(output.iterdir()):
        raise ValueError("output domain must be empty")
    request = support.read_json(support.input_path(root, request_name))
    if request.get("schema") != "madb.generate-request/1":
        raise ValueError("ordinary peer requires a GenerateRequest")
    config = support.read_json(support.input_path(root, "backend.json"))
    scratch = Path(tempfile.mkdtemp(prefix="private-ordinary-generator-", dir=output))
    # The production function rereads real declarations and finalizes them; the
    # input ContractIR is not a substitute for actual parser execution.
    value = generator.generate(request, config, root, scratch)
    generator.driver.verify(request, config, root)
    support.verify_refs(root, [request["profile"], request["lock"], request["contract"]])
    support.verify_refs(scratch, value["files"])
    data = (generator.driver.documents(root).canonical(value) + "\n").encode()
    if len(data) > support.MAX_JSON or value.get("manifest_path") != "outputs.json":
        raise ValueError("ordinary generator output manifest is invalid")
    support.publish_diagnostics(scratch)
    for item in value["files"]:
        support.copy_new(support.input_path(scratch, item["path"]), output / item["path"])
    shutil.rmtree(scratch)
    with (output / "outputs.json").open("xb") as stream:
        stream.write(data)
    support.verify_refs(output, value["files"])
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    sys.stdout.buffer.write(generate(args.input_root, args.output_root, args.request))


if __name__ == "__main__":
    main()
