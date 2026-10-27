"""Finite business adapter for the public E02 C/ArkTS reader/generator peer.

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


def run(operation, input_root, output_root, request_name):
    root, output = directory(input_root), directory(output_root)
    if root == output or root in output.parents or output in root.parents:
        raise ValueError("input and output domains must be separate")
    if list(output.iterdir()):
        raise ValueError("output domain must be empty")
    request = support.read_json(support.input_path(root, request_name))
    expected_schema = {"read": "madb.read-request/1", "generate": "madb.generate-request/1"}[operation]
    if request.get("schema") != expected_schema:
        raise ValueError("selected peer request schema differs")
    config = support.read_json(support.input_path(root, "backend.json"))
    scratch = Path(tempfile.mkdtemp(prefix="private-composition-" + operation + "-", dir=output))
    # The production function rereads real declarations and finalizes them; the
    # input ContractIR is not a substitute for actual parser execution.
    function = generator.metadata if operation == "read" else generator.generate
    value = function(request, config, root, scratch)
    generator.driver.verify(request, config, root)
    references = [request["profile"], request["lock"]]
    if operation == "generate":
        references.append(request["contract"])
        support.verify_refs(scratch, value["files"])
    support.verify_refs(root, references)
    data = (generator.driver.documents(root).canonical(value) + "\n").encode()
    result_file = "contract.json" if operation == "read" else "outputs.json"
    if len(data) > support.MAX_JSON:
        raise ValueError("component output exceeds budget")
    if operation == "generate" and value.get("manifest_path") != result_file:
        raise ValueError("generator output manifest is invalid")
    if operation == "read" and value.get("schema") != "madb.contract-ir/1":
        raise ValueError("reader did not produce ContractIR")
    support.publish_diagnostics(scratch)
    if operation == "generate":
        for item in value["files"]:
            support.copy_new(support.input_path(scratch, item["path"]), output / item["path"])
    shutil.rmtree(scratch)
    with (output / result_file).open("xb") as stream:
        stream.write(data)
    if operation == "generate":
        support.verify_refs(output, value["files"])
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--operation", choices=("read", "generate"), default="generate")
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    sys.stdout.buffer.write(run(args.operation, args.input_root, args.output_root, args.request))


if __name__ == "__main__":
    main()
