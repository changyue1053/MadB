"""Observe an explicitly selected ArkTS environment; never certify C07."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts.preflight import inspect_environment


def write_new_json(path: Path, value: dict) -> None:
    """Publish complete bytes atomically; hard-link creation never overwrites."""
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix=".arkts-preflight-", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    layout = parser.add_mutually_exclusive_group(required=True)
    layout.add_argument("--deveco", type=Path, help="Explicit DevEco Contents directory")
    layout.add_argument("--command-line-tools", type=Path, help="Explicit mac-arm64 Command Line Tools directory")
    parser.add_argument("--sdk", required=True, type=Path, help="Application SDK package root containing sdk-pkg.json and openharmony/")
    parser.add_argument("--target", help="Existing hdc connect key; omission remains a failed environment check")
    parser.add_argument("--target-abi", default="arm64-v8a", help="Requested target ABI, default arm64-v8a; no profile is certified")
    parser.add_argument("--timeout", type=float, default=10, help="Per-command timeout in seconds, 0 < value <= 30")
    parser.add_argument("--json-output", type=Path, help="New output path in an existing directory; never overwrites")
    arguments = parser.parse_args(argv)
    try:
        report = inspect_environment(arguments.deveco, arguments.sdk, arguments.target,
                                     arguments.timeout, arguments.target_abi, command_line_tools=arguments.command_line_tools)
        if arguments.json_output is not None:
            write_new_json(arguments.json_output, report)
    except (OSError, ValueError) as error:
        parser.exit(2, f"ArkTS preflight failed: {error}\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["environment_checks_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
