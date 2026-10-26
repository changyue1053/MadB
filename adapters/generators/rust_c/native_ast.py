"""Read the locked Clang AST of an explicit, restricted C ABI surface."""
from __future__ import annotations

import json
from pathlib import Path
import re

from support import NativeError, run_tool


_SCALARS = {
    "void": "void", "unsigned char": "u8", "signed char": "i8",
    "unsigned short": "u16", "short": "i16", "unsigned int": "u32", "int": "i32",
    "unsigned long long": "u64", "long long": "i64",
    "unsigned long": "u64", "long": "i64",
    "uint8_t": "u8", "int8_t": "i8", "uint16_t": "u16", "int16_t": "i16",
    "uint32_t": "u32", "int32_t": "i32", "uint64_t": "u64", "int64_t": "i64",
}


def abi_type(spelling, aliases, trail=(), *, value_abi=False):
    spelling = " ".join(spelling.split())
    array = re.fullmatch(r"(.+)\[64\]", spelling) if value_abi else None
    if array:
        element = abi_type(array[1].strip(), aliases, trail, value_abi=True)
        if element != "u8":
            raise NativeError("NATIVE004", "only the finite value UTF-8 byte array is supported")
        return "u8[64]"
    if spelling in _SCALARS:
        return _SCALARS[spelling]
    if spelling.startswith("struct "):
        return spelling[7:]
    callback = re.fullmatch(r"(.+) \(\*\)\((.*)\)", spelling)
    if callback:
        parameters = callback[2].split(",") if callback[2] not in ("", "void") else []
        return "fn(" + ",".join(abi_type(p.strip(), aliases, trail, value_abi=value_abi) for p in parameters) + ")->" + abi_type(callback[1], aliases, trail, value_abi=value_abi)
    if spelling.endswith(" *") or spelling.endswith("*"):
        pointee = spelling[:-1].rstrip()
        return abi_type(pointee, aliases, trail, value_abi=value_abi) + " *"
    if spelling.startswith("const "):
        return "const " + abi_type(spelling[6:], aliases, trail, value_abi=value_abi)
    if spelling in aliases and spelling not in trail:
        return abi_type(aliases[spelling], aliases, (*trail, spelling), value_abi=value_abi)
    raise NativeError("NATIVE004", f"unsupported C ABI type: {spelling}")


def _location(node, header):
    loc = node.get("loc", {})
    if "spellingLoc" in loc or "expansionLoc" in loc:
        raise NativeError("NATIVE004", "macro-generated public declaration is outside this reader")
    offset = loc.get("offset")
    data = header.read_bytes()
    if not isinstance(offset, int) or offset < 0 or offset >= len(data):
        raise NativeError("NATIVE006", f"missing declaration offset: {node.get('name')}")
    prefix = data[:offset]
    return {"path": str(header), "line": prefix.count(b"\n") + 1,
            "column": len(prefix.rsplit(b"\n", 1)[-1]) + 1}


def read_header(header: Path, config: dict, *, value_abi=False, constant_names=None, record_sizes=None) -> dict:
    """Return actual declaration types/locations; no policy or logical IR inference."""
    header = Path(header).resolve(strict=True)
    arguments = config["clang_arguments"]
    if not isinstance(arguments, list) or any(not isinstance(a, str) for a in arguments):
        raise NativeError("NATIVE001", "clang_arguments must be a fixed string array")
    output = run_tool(config, "clang", [*arguments, "-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c", header])
    try:
        tree = json.loads(output)
    except (ValueError, UnicodeError) as error:
        raise NativeError("NATIVE003", "Clang did not produce a JSON AST") from error
    all_aliases = {}
    public = []
    last_file = None
    for node in tree.get("inner", []):
        loc = node.get("loc", {})
        if "file" in loc:
            last_file = Path(loc["file"]).resolve()
        if node.get("kind") == "TypedefDecl":
            all_aliases[node["name"]] = node["type"]["qualType"]
        if last_file == header and not node.get("isImplicit"):
            public.append(node)
    surface = {"functions": {}, "records": {}, "aliases": {}, "layouts": {}, "constants": {}}
    anonymous = {}
    layout_names = {}
    for node in public:
        if node["kind"] == "TypedefDecl":
            for item in node.get("inner", []):
                owned = item.get("ownedTagDecl")
                if owned and not owned.get("name"):
                    anonymous[owned["id"]] = node["name"]
    for node in public:
        kind, name = node["kind"], node.get("name", "")
        if kind == "FunctionDecl":
            if node.get("variadic") or node.get("storageClass") == "static":
                raise NativeError("NATIVE004", f"unsupported public function: {name}")
            if name in surface["functions"]:
                raise NativeError("NATIVE004", f"duplicate public function declaration: {name}")
            if any(item.get("kind") == "CompoundStmt" for item in node.get("inner", [])):
                raise NativeError("NATIVE004", f"inline implementation is outside header contract: {name}")
            if any(item.get("kind") not in ("ParmVarDecl", "FullComment") for item in node.get("inner", [])):
                raise NativeError("NATIVE004", f"function attributes/alternate conventions are unsupported: {name}")
            spelling = node["type"]["qualType"]
            result = spelling.split(" (", 1)[0]
            parameters = [{"name": item.get("name", ""), "type": abi_type(item["type"]["qualType"], all_aliases, value_abi=value_abi)}
                          for item in node.get("inner", []) if item.get("kind") == "ParmVarDecl"]
            if any(not p["name"] for p in parameters):
                raise NativeError("NATIVE004", f"public parameters must be named: {name}")
            surface["functions"][name] = {"result": abi_type(result, all_aliases, value_abi=value_abi), "parameters": parameters,
                                           "location": _location(node, header)}
        elif kind == "RecordDecl" and node.get("completeDefinition"):
            if node.get("tagUsed") != "struct":
                raise NativeError("NATIVE004", "unions are unsupported")
            if not name:
                name = anonymous.get(node["id"])
                if not name:
                    raise NativeError("NATIVE004", "anonymous record has no stable typedef")
                loc = _location(node, header)
                label = f"(unnamed at {header}:{loc['line']}:{loc['column']})"
            else:
                label = name
            layout_names[label] = name
            fields = []
            for field in node.get("inner", []):
                if field.get("kind") == "FieldDecl":
                    if field.get("isBitfield"):
                        raise NativeError("NATIVE004", "bitfields are unsupported")
                    fields.append({"name": field["name"], "type": abi_type(field["type"]["qualType"], all_aliases, value_abi=value_abi)})
            surface["records"][name] = fields
        elif kind == "TypedefDecl":
            surface["aliases"][name] = abi_type(node["type"]["qualType"], all_aliases, value_abi=value_abi)
        elif kind not in ("EmptyDecl", "StaticAssertDecl"):
            raise NativeError("NATIVE004", f"unsupported public declaration: {kind} {name}")
    if surface["records"]:
        layouts = run_tool(config, "clang", [*arguments, "-Xclang", "-fdump-record-layouts-complete",
                                             "-fsyntax-only", "-x", "c", header]).decode()
        surface["layouts"] = (_value_record_layouts(layouts, surface["records"], layout_names, record_sizes or {})
                              if value_abi else _record_layouts(layouts, surface["records"], layout_names))
    macros = run_tool(config, "clang", [*arguments, "-dM", "-E", "-x", "c", header]).decode()
    if value_abi:
        if constant_names is None:
            raise NativeError("NATIVE001", "value ABI reading requires an exact constant-name set")
        declared = set()
        text = header.read_text()
        guards = set(re.findall(r"#ifndef ([A-Za-z_][A-Za-z_0-9]*)\s*\n#define \1\s*\n", text))
        for line in text.splitlines():
            match = re.match(r"#define\s+([A-Za-z_][A-Za-z_0-9]*)(.*)", line)
            if match and match[1] not in guards:
                if match[1] not in constant_names or not match[2].startswith(" "):
                    raise NativeError("NATIVE004", "unexpected value ABI macro: " + match[1])
                declared.add(match[1])
        if declared != set(constant_names):
            raise NativeError("NATIVE005", "value ABI constant declarations differ from the source plan")
        for line in macros.splitlines():
            match = re.fullmatch(r"#define ([A-Za-z_][A-Za-z_0-9]*) (.+)", line)
            if match and match[1] in declared:
                value = match[2].strip("()")
                if not re.fullmatch(r"[0-9]+[uUlL]*", value):
                    raise NativeError("NATIVE004", "unsupported value ABI integer macro: " + match[1])
                surface["constants"][match[1]] = int(value.rstrip("uUlL"))
        return surface
    for line in macros.splitlines():
        if re.match(r"#define MADB_[A-Z0-9_]+\(", line):
            raise NativeError("NATIVE004", "public function-like macros are unsupported")
        match = re.fullmatch(r"#define (MADB_[A-Z0-9_]+) (.+)", line)
        if match:
            value = match[2].strip("()")
            if not re.fullmatch(r"[0-9]+[uUlL]*", value):
                raise NativeError("NATIVE004", f"unsupported public integer macro: {match[1]}")
            surface["constants"][match[1]] = int(value.rstrip("uUlL"))
    return surface


def _record_layouts(text, records, names):
    result = {}
    for block in text.split("*** Dumping AST Record Layout"):
        start = re.search(r"^\s*0 \| struct ([^\n]+)\s*$", block, re.MULTILINE)
        if not start or start[1].strip() not in names:
            continue
        name = names[start[1].strip()]
        size = re.search(r"\[sizeof=(\d+), align=(\d+)\]", block)
        if not size:
            raise NativeError("NATIVE006", f"missing compiler layout: {name}")
        fields = []
        for field in records[name]:
            location = re.search(r"^\s*(\d+) \|\s+[^\n]+\b" + re.escape(field["name"]) + r"\s*$", block, re.MULTILINE)
            if not location or field["type"] not in ("u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64"):
                raise NativeError("NATIVE004", f"unsupported record layout field: {name}.{field['name']}")
            fields.append({"name": field["name"], "offset": int(location[1]), "size": int(field["type"][1:]) // 8})
        result[name] = {"size": int(size[1]), "alignment": int(size[2]), "fields": fields}
    if set(result) != set(records):
        raise NativeError("NATIVE006", "Clang omitted a public record layout")
    return result



def _value_record_layouts(text, records, names, external_sizes):
    """Opt-in finite values: direct fields only; nested primitive storage is sized by Clang."""
    blocks = {}
    sizes = dict(external_sizes)
    for block in text.split("*** Dumping AST Record Layout"):
        start = re.search(r"^\s*0 \| struct ([^\n]+)\s*$", block, re.MULTILINE)
        if not start or start[1].strip() not in names:
            continue
        name = names[start[1].strip()]
        size = re.search(r"\[sizeof=(\d+), align=(\d+)\]", block)
        if not size or name in blocks:
            raise NativeError("NATIVE006", "missing/duplicate value record layout: " + name)
        blocks[name] = (block, int(size[1]), int(size[2]))
        sizes[name] = int(size[1])
    if set(blocks) != set(records):
        raise NativeError("NATIVE006", "Clang omitted a value record layout")
    result = {}
    for name, fields in records.items():
        block, size, alignment = blocks[name]
        observed = []
        for field in fields:
            matches = re.findall(r"^\s*(\d+) \|   [^ \n][^\n]*\b" + re.escape(field["name"]) + r"\s*$", block, re.MULTILINE)
            kind = field["type"]
            width = 64 if kind == "u8[64]" else sizes.get(kind)
            if kind in ("u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64"):
                width = int(kind[1:]) // 8
            if len(matches) != 1 or width is None:
                raise NativeError("NATIVE004", "unsupported value record layout field: " + name + "." + field["name"])
            observed.append(dict(name=field["name"], offset=int(matches[0]), size=width))
        result[name] = dict(size=size, alignment=alignment, fields=observed)
    return result


def abi_projection(surface):
    """Locations and process-specific Clang node IDs never enter ABI identity."""
    return {
        "functions": [{"name": name, "result": fn["result"], "parameters": fn["parameters"]}
                      for name, fn in sorted(surface["functions"].items())],
        "records": [{"name": name, "fields": fields} for name, fields in sorted(surface["records"].items())],
        "aliases": [{"name": name, "type": kind} for name, kind in sorted(surface["aliases"].items())],
        "layouts": [{"name": name, **layout} for name, layout in sorted(surface.get("layouts", {}).items())],
        "constants": [{"name": name, "value": value} for name, value in sorted(surface.get("constants", {}).items())],
    }
