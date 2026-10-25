#!/usr/bin/env python3
"""Read actual Objective-C export and SKIE Swift declarations; never infer APIs from filenames."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def run(arguments: list[str], executable: str | None = None) -> str:
    result = subprocess.run(arguments, executable=executable, text=True, capture_output=True, check=False, timeout=120)
    if result.returncode:
        raise RuntimeError(f"compiler API extraction failed ({result.returncode}): {result.stderr.strip()}")
    return result.stdout


def stable(node):
    if isinstance(node, dict):
        return {key: stable(value) for key, value in node.items() if key not in {"location", "uri"}}
    if isinstance(node, list):
        return [stable(item) for item in node]
    return node


def objc_declarations(ast: dict, header: Path) -> list[dict]:
    declarations = []
    for node in ast.get("inner", []):
        loc = node.get("loc", {})
        if node.get("kind") not in {"ObjCInterfaceDecl", "ObjCProtocolDecl"} or loc.get("includedFrom"):
            continue
        if loc.get("file") and Path(loc["file"]).resolve() != header.resolve():
            continue
        members = []
        for member in node.get("inner", []):
            if member.get("kind") == "ObjCMethodDecl":
                members.append({"kind": "method", "name": member["name"], "instance": member.get("instance", False),
                    "return_type": member["returnType"]["qualType"],
                    "parameters": [{"name": p.get("name"), "type": p["type"]["qualType"]}
                        for p in member.get("inner", []) if p.get("kind") == "ParmVarDecl"]})
            elif member.get("kind") == "ObjCPropertyDecl":
                members.append({"kind": "property", "name": member["name"], "type": member["type"]["qualType"],
                    "readonly": member.get("readonly", False)})
        declarations.append({"kind": node["kind"], "name": node["name"],
            "super": node.get("super", {}).get("name"),
            "members": sorted(members, key=lambda member: (member["name"], member["kind"]))})
    return sorted(declarations, key=lambda item: (item["name"], item["kind"]))


def extract(framework: Path, target: str, sdk: Path, roots: set[str] | None = None, *, tools: dict | None = None) -> dict:
    framework = framework.resolve()
    module = framework.stem
    header = framework / "Headers" / (module + ".h")
    interfaces = sorted((framework / "Modules" / (module + ".swiftmodule")).glob("*.swiftinterface"))
    public_interfaces = [path for path in interfaces if ".private." not in path.name]
    if not header.is_file() or not public_interfaces:
        raise ValueError("Expected a distributable generated framework with its Objective-C header and public Swift interface")
    clang = [tools["clang"]["path"]] if tools else ["xcrun", "clang"]
    symbolgraph = ["swift-symbolgraph-extract"] if tools else ["xcrun", "swift-symbolgraph-extract"]
    ast = json.loads(run(clang + [ "-x", "objective-c", "-fsyntax-only", "-fblocks", "-target", target,
        "-isysroot", str(sdk), "-Xclang", "-ast-dump=json", str(header)]))
    with tempfile.TemporaryDirectory(prefix="madb-skie-api-") as temporary:
        run(symbolgraph + [ "-module-name", module, "-F", str(framework.parent),
            "-target", target, "-sdk", str(sdk), "-output-dir", temporary, "-minimum-access-level", "public", "-module-cache-path", str(Path(temporary) / "module-cache")],
            executable=tools["swift_symbolgraph_extract"]["path"] if tools else None)
        graph = json.loads((Path(temporary) / (module + ".symbols.json")).read_text())
    symbols = graph["symbols"]
    if roots is not None:
        found = {symbol["pathComponents"][0] for symbol in symbols}
        missing = roots - found
        if missing:
            raise ValueError("Requested public export roots absent in compiler output: " + ", ".join(sorted(missing)))
        symbols = [symbol for symbol in symbols if symbol["pathComponents"][0] in roots]
    symbols = sorted((stable(symbol) for symbol in symbols), key=lambda item: item["identifier"]["precise"])
    return {"schema_version": 1, "source_kind": "compiled-kotlin-native-objc-skie", "module": module, "target": target,
        "header_digest": digest(header),
        "swift_interfaces": [{"name": path.name, "digest": digest(path)} for path in public_interfaces],
        "objc_declarations": objc_declarations(ast, header), "swift_symbols": symbols}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("framework", type=Path)
    parser.add_argument("--target", default="arm64-apple-macosx14.0")
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--export-root", action="append")
    parser.add_argument("--tools", type=Path, help="JSON file containing the observed tool image map")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = extract(args.framework, args.target, args.sdk, set(args.export_root) if args.export_root else None,
        tools=json.loads(args.tools.read_text()) if args.tools else None)
    args.output.write_text(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n")


if __name__ == "__main__":
    main()
