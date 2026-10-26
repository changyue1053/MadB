"""Execute actual companion derivations; native behavior is a host test model."""
from pathlib import Path
import argparse
import json
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts.native import _derive
from adapters.toolchains.arkts.kotlin_test_binding import derive_boundary
from adapters.toolchains.arkts.kotlin_frame import render


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--typescript", type=Path, required=True,
                        help="actual locked compiler API; no module fallback")
    args = parser.parse_args()
    ordinary = (ROOT / "sdk/arkts/facade/Boundary.ets.in").read_text()
    support = ROOT / "sdk/arkts/test-support/facade"
    native = _derive({"Boundary.ets.in": ordinary},
                     (support / "companion.patch").read_text(),
                     json.loads((support / "baseline.json").read_text()))["Boundary.ets.in"]
    kotlin = derive_boundary(ordinary.replace("frame.length !== 54", "frame.length !== 110"))
    assert "testBeforeRuntimeReceipt" not in ordinary
    with tempfile.TemporaryDirectory(prefix="madb-c09-receipt-") as directory:
        output = Path(directory)
        for name, source, frame in (
            ("native", native, (ROOT / "sdk/arkts/facade/RuntimeFrame.ets").read_bytes()),
            ("kotlin", kotlin, render()["generated/facade/RuntimeFrame.ets"]),
        ):
            boundary = output / (name + ".ets")
            boundary.write_text(source.replace("@MADB_NATIVE_MODULE@", "'./native'"))
            frame_path = output / (name + "-frame.ets")
            frame_path.write_bytes(frame)
            subprocess.run(["node", str(Path(__file__).with_name("check.cjs")),
                            str(boundary), str(frame_path), name,
                            str(args.typescript.resolve(strict=True))], check=True)


if __name__ == "__main__":
    main()
