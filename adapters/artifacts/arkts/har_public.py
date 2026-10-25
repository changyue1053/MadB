"""Validate actual HAR compiler declarations against the frozen public AST."""

from pathlib import Path, PurePosixPath
import importlib.util
import json
import hashlib

_spec = importlib.util.spec_from_file_location(
    "_c07_port", Path(__file__).resolve().parents[3] / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)
lowering = support.load_module(
    Path(__file__).resolve().parents[3] / "adapters/generators/arkts/lowering.py"
)


def verify_public_har(raw, expected, config, work, har_module):
    members = {m.path: m.data for m in har_module.read_members(raw) if m.data is not None}
    entry = "package/Index.d.ets"
    if entry not in members:
        raise ValueError("HAR is missing its compiler declaration entry")
    ts = config["tools"]["typescript"]
    support.verify_tool(ts)
    groups = json.loads(
        support.command(
            config,
            "node",
            [
                Path(__file__).resolve().parents[3]
                / "adapters/generators/arkts/read_har_exports.cjs",
                ts["path"],
                entry,
            ],
            cwd=work,
            stdin=members[entry],
        )
    )["exports"]
    if len(groups) != 1:
        raise ValueError("restricted HAR requires one explicit public declaration module")
    group = groups[0]
    names = {
        r["name"] for key in ("records", "resources", "errors", "functions") for r in expected[key]
    }
    if set(group["names"]) != names or len(group["names"]) != len(names):
        raise ValueError("HAR public exports differ from frozen source declarations")
    module = group["module"]
    if (
        not module.startswith("./")
        or "\\" in module
        or "\0" in module
        or any(p in ("", ".", "..") for p in module[2:].split("/"))
    ):
        raise ValueError("HAR declaration import is not a closed relative module")
    suffix = "" if module.endswith(".d.ets") else ".d.ets"
    target = str(PurePosixPath(entry).parent / PurePosixPath(module[2:] + suffix))
    if target not in members:
        raise ValueError("HAR re-export target declaration is missing")
    actual = json.loads(
        support.command(
            config,
            "node",
            [
                Path(__file__).resolve().parents[3] / "adapters/generators/arkts/read_facade.cjs",
                ts["path"],
                target,
                "declaration-test" if config.get("variant") == "test" else "declaration",
            ],
            cwd=work,
            stdin=members[target],
        )
    )
    if lowering.projection(actual) != lowering.projection(expected):
        raise ValueError("compiled HAR declaration differs from frozen public source shape")
    support.verify_tool(ts)
    return {
        "scope": "actual compiler-emitted HAR public declarations compared with frozen source AST; not target execution",
        "entry": {
            "path": entry,
            "sha256": "sha256:" + hashlib.sha256(members[entry]).hexdigest(),
            "size": len(members[entry]),
        },
        "declarations": {
            "path": target,
            "sha256": "sha256:" + hashlib.sha256(members[target]).hexdigest(),
            "size": len(members[target]),
        },
        "public_exports": sorted(names),
        "source": expected["source"],
    }
