"""Project checked ArkTS value declarations; no numeric ABI or native lowering.

The source remains the implementation authority. This frontend emits a barrel
and a declaration view, without inferring ownership or extending a C07 profile.
"""
from copy import deepcopy
import hashlib
import json
import re


def generate(descriptor: dict, source: bytes) -> dict[str, bytes]:
    """Return a complete fresh output set. The caller owns publication and I/O."""
    if (descriptor.get("schema") != "madb.private.arkts-public-declarations.v1"
            or descriptor.get("authority") != "exported-arkts-declarations"
            or descriptor.get("resources") or descriptor.get("errors") or descriptor.get("imports")):
        raise ValueError("expected checked source value declarations")
    identity = descriptor["source"]
    digest = "sha256:" + hashlib.sha256(source).hexdigest()
    if identity["sha256"] != digest or identity["size"] != len(source) or len(source) > 1024 * 1024:
        raise ValueError("declaration/source identity differs")
    records, functions = descriptor["records"], descriptor["functions"]
    if not records or not functions or len(records) + len(functions) > 64:
        raise ValueError("value projection needs bounded records and functions")

    def identifier(name):
        if not isinstance(name, str) or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) is None:
            raise ValueError("invalid declaration identifier")
        return name

    names = [identifier(item["name"]) for item in records + functions]
    if len(set(names)) != len(names):
        raise ValueError("duplicate declaration")
    record_names = {item["name"] for item in records}

    def value_type(value, *, result=False):
        if value == {"tag": "number"} or value == {"tag": "string"}:
            return value["tag"]
        if result and value == {"tag": "void"}:
            return "void"
        if set(value) == {"tag", "name"} and value["tag"] == "reference" and value["name"] in record_names:
            return identifier(value["name"])
        raise ValueError("unsupported value projection type")

    declarations = ["// Generated value declarations. No numeric ABI or native contract is inferred."]
    exports = ["// Generated from the checked implementation; do not edit."]
    origins = []
    for item in records + functions:
        name = item["name"]
        location = deepcopy(item["location"])
        if location["path"] != identity["path"] or any(type(location[k]) is not int or location[k] < 1 for k in ("line", "column")):
            raise ValueError("declaration location differs from source")
        first_line = len(declarations) + 1
        if item in records:
            fields = item["fields"]
            if not fields or len(fields) > 64 or len({f["name"] for f in fields}) != len(fields):
                raise ValueError("invalid record fields")
            declarations.append("export interface " + name + " {")
            for field in fields:
                if field["type"] not in ({"tag": "number"}, {"tag": "string"}):
                    raise ValueError("record projection requires scalar fields")
                declarations.append("  " + identifier(field["name"]) + ": " + value_type(field["type"]) + ";")
            declarations.append("}")
        else:
            if item["owner"] is not None or len(item["params"]) > 8:
                raise ValueError("only bounded free functions can be projected")
            params = item["params"]
            if len({p["name"] for p in params}) != len(params):
                raise ValueError("duplicate parameter")
            arguments = ", ".join(identifier(p["name"]) + ": " + value_type(p["type"]) for p in params)
            declarations.append("export declare function " + name + "(" + arguments + "): " + value_type(item["returns"], result=True) + ";")
        exports.append("export { " + name + " } from './Business';")
        origins.append({"name": name, "source": location,
                        "declaration_lines": [first_line, len(declarations)], "export_line": len(exports)})
    return {"Business.ets": source,
            "Index.ets": ("\n".join(exports) + "\n").encode(),
            "Index.d.ts": ("\n".join(declarations) + "\n").encode(),
            "origins.json": (json.dumps({"source": identity, "declarations": origins}, indent=2) + "\n").encode()}
