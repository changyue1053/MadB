"""Business worker behind the public AuthorSession Dart extension peer.

This worker owns only its isolated output domain. Control framing, cancellation,
process-group cleanup and final admission are provided by the public peer/Host.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wire
import reader


def directory(path):
    path = Path(path)
    if not path.is_absolute() or path != path.resolve(strict=True) or not path.is_dir():
        raise ValueError("canonical absolute directory required")
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError("symbolic directory is forbidden")
    return path


def run(operation, input_root, output_root, request_name):
    root, output = directory(input_root), directory(output_root)
    if root == output or root in output.parents or output in root.parents or list(output.iterdir()):
        raise ValueError("fresh separate output domain required")
    request = wire.read_json(wire.confined(root, request_name))
    expected = {"read": "madb.read-request/1", "generate": "madb.generate-request/1"}[operation]
    if request["schema"] != expected:
        raise ValueError("request schema differs from the selected port")
    config = wire.read_json(wire.confined(root, "backend.json"))
    scratch = Path(tempfile.mkdtemp(prefix="private-dart-" + operation + "-", dir=output))
    if operation == "read":
        value = reader.read(request, config, root, scratch)
        references = []
    else:
        import generate
        value = generate.generate(request, config, root, scratch)
        references = value["files"]
        wire.verify(scratch, references)
    wire.verify(root, request["inputs"] + [request["profile"], request["lock"]])
    if operation == "generate":
        wire.verify(root, [request["contract"]])
    data = (wire.canonical(value) + "\n").encode()
    if len(data) > wire.MAX_DOCUMENT:
        raise ValueError("result exceeds public frame document budget")
    for item in references:
        wire.write_new(output, item["path"], wire.confined(scratch, item["path"]).read_bytes())
    shutil.rmtree(scratch)
    wire.write_new(output, "contract.json" if operation == "read" else "outputs.json", data)
    if references:
        wire.verify(output, references)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--operation", choices=("read", "generate"), required=True)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    sys.stdout.buffer.write(run(args.operation, args.input_root, args.output_root, args.request))


if __name__ == "__main__":
    main()
