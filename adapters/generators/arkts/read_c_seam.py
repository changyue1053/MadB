"""Private C07 C seam reader: compiler facts, never pointer ownership inference.

The C05 parser/policy stays unchanged. Reuse its checked compiler invocation and
scalar/pointer/callback spelling normalizer, but separately admit C07 enums and
pointer/callback record layouts from Clang's actual target layout dump.
"""

from __future__ import annotations
import importlib.util
import hashlib
import json
from pathlib import Path
import re
import shlex
import sys


def _c05(repo):
    directory = Path(repo) / "adapters/generators/rust_c"
    saved = sys.modules.get("support")

    def module(name, path):
        spec = importlib.util.spec_from_file_location(name, path)
        value = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(value)
        return value

    support = module("_madb_c07_c05_support", directory / "support.py")
    try:
        sys.modules["support"] = support
        reader = module("_madb_c07_c05_native_ast", directory / "native_ast.py")
    finally:
        if saved is None:
            sys.modules.pop("support", None)
        else:
            sys.modules["support"] = saved
    return reader


def read_header(repo, root, reference, config):
    """Read one original C04 FileReference with fixed aarch64 OHOS API23 C17 arguments.

    SDK dependency roots must belong to a separately checked, frozen SDK tree.
    This function checks the actual header dependency paths and identities of
    tool/source; it does not itself certify that whole SDK tree.
    """
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("C seam source root must be an ordinary directory")
    root = root.resolve(strict=True)
    reader = _c05(repo)
    run = reader.run_tool
    name = reference["path"]
    if (
        not isinstance(name, str)
        or not name
        or "\\" in name
        or "\0" in name
        or Path(name).is_absolute()
        or any(x in ("", ".", "..") for x in name.split("/"))
    ):
        raise ValueError("invalid C seam source path")
    path = root
    for part in name.split("/"):
        path = path / part
        if path.is_symlink():
            raise ValueError("C seam source symlink is unsupported")

    def verify():
        b = path.read_bytes()
        if (
            type(reference.get("size")) is not int
            or len(b) != reference["size"]
            or "sha256:" + hashlib.sha256(b).hexdigest() != reference.get("sha256")
        ):
            raise ValueError("frozen C seam source changed")
        if len(b) > 1024 * 1024:
            raise ValueError("C seam source budget exceeded")
        return b

    verify()
    native = Path(config["native_sdk"]).resolve(strict=True)
    expected = [
        "--target=aarch64-linux-ohos23.0.0",
        "--sysroot=" + str(native / "sysroot"),
        "-std=c17",
        "-O2",
        "-fPIC",
        "-fvisibility=hidden",
    ]
    if config.get("clang_arguments") != expected:
        raise ValueError("C07 seam requires its explicit OHOS API23 C17 arguments")
    args = expected
    # Clang reports every opened header. No undeclared transitive author header.
    deps = run(config, "clang", [*args, "-M", "-MT", "decl", "-x", "c", path]).decode()
    paths = shlex.split(deps.replace("\\\n", " ").split(":", 1)[1])
    dependency_paths = sorted({Path(p).resolve(strict=True) for p in paths})
    allowed = [(native / "sysroot").resolve(), (native / "llvm/lib/clang").resolve()]
    if any(p != path and not any(p.is_relative_to(d) for d in allowed) for p in dependency_paths):
        raise ValueError("C seam has an undeclared transitive author dependency")
    dependency_refs = [
        {
            "path": p.relative_to(native).as_posix(),
            "sha256": "sha256:" + hashlib.sha256(p.read_bytes()).hexdigest(),
            "size": p.stat().st_size,
        }
        for p in dependency_paths
        if p != path
    ]
    tree = json.loads(
        run(config, "clang", [*args, "-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c", path])
    )
    layouts = run(
        config,
        "clang",
        [
            *args,
            "-Xclang",
            "-fdump-record-layouts-simple",
            "-Xclang",
            "-fdump-record-layouts-complete",
            "-fsyntax-only",
            "-x",
            "c",
            path,
        ],
    ).decode()

    def location(node):
        result = reader._location(node, path)
        result["path"] = name
        return result

    def fail(node, message):
        loc = location(node)
        raise ValueError(f"{name}:{loc['line']}:{loc['column']}: {message}")

    aliases, public, last = {}, [], None
    for node in tree.get("inner", []):
        loc = node.get("loc", {})
        if "file" in loc:
            last = Path(loc["file"]).resolve()
        if node["kind"] == "TypedefDecl":
            aliases[node["name"]] = node["type"]["qualType"]
        if last == path and not node.get("isImplicit"):
            public.append(node)
    if not public or len(public) > 256:
        raise ValueError("C seam declaration count is out of bounds")
    anonymous = {}
    for node in public:
        if node["kind"] == "TypedefDecl":
            for child in node.get("inner", []):
                owned = child.get("ownedTagDecl")
                if owned and not owned.get("name"):
                    anonymous[owned["id"]] = node["name"]
    result = {
        "schema": "madb.private.c-seam-declarations.v1",
        "authority": "clang-c-declarations",
        "source": reference,
        "target": "aarch64-linux-ohos",
        "records": [],
        "enums": [],
        "aliases": [],
        "functions": [],
    }
    labels, complete = {}, set()

    def normalized(node):
        try:
            return reader.abi_type(node["type"]["qualType"], aliases)
        except reader.NativeError as error:
            fail(node, str(error))

    for node in public:
        kind = node["kind"]
        identifier = node.get("name")
        if kind == "RecordDecl":
            if node.get("tagUsed") != "struct":
                fail(node, "union is unsupported")
            if not node.get("completeDefinition"):
                fail(node, "forward-only public record is outside this seam")
            record = identifier or anonymous.get(node["id"])
            if not record or record in complete:
                fail(node, "missing/duplicate record identity")
            complete.add(record)
            fields = []
            for field in node.get("inner", []):
                if (
                    field["kind"] != "FieldDecl"
                    or field.get("isBitfield")
                    or field.get("inner")
                    or not field.get("name")
                ):
                    fail(field, "record attributes/anonymous fields/bitfields are unsupported")
                fields.append(
                    {
                        "name": field["name"],
                        "type": normalized(field),
                        "spelling": field["type"]["qualType"],
                        "location": location(field),
                    }
                )
            if not fields or len(fields) > 64:
                fail(node, "record field count is out of bounds")
            loc = location(node)
            label = identifier or f"(unnamed at {path}:{loc['line']}:{loc['column']})"
            labels[label] = record
            result["records"].append({"name": record, "fields": fields, "location": loc})
        elif kind == "EnumDecl":
            if identifier:
                fail(node, "only anonymous constant enums are supported")
            constants = []
            for item in node.get("inner", []):
                if item["kind"] != "EnumConstantDecl":
                    fail(item, "enum attributes are unsupported")
                # Evaluated ConstantExpr is an actual compiler value; never eval source text.
                expr = item.get("inner", [])
                if len(expr) != 1 or expr[0]["kind"] != "ConstantExpr" or "value" not in expr[0]:
                    fail(item, "enum values must be explicitly compiler-evaluated")
                value = int(expr[0]["value"])
                if not -(2**31) <= value < 2**31:
                    fail(item, "enum exceeds signed int32")
                constants.append({"name": item["name"], "value": value, "location": location(item)})
            if not constants or len(constants) > 64:
                fail(node, "enum constant count is out of bounds")
            result["enums"].append({"constants": constants, "location": location(node)})
        elif kind == "TypedefDecl":
            if any(c["kind"].endswith("Attr") for c in node.get("inner", [])):
                fail(node, "typedef attributes are unsupported")
            result["aliases"].append(
                {"name": identifier, "type": normalized(node), "location": location(node)}
            )
        elif kind == "FunctionDecl":
            if (
                node.get("variadic")
                or node.get("storageClass")
                or any(
                    n["kind"] not in ("ParmVarDecl", "FullComment") for n in node.get("inner", [])
                )
            ):
                fail(node, "function body/storage/attributes/variadic form is unsupported")
            params = []
            for p in node.get("inner", []):
                if p["kind"] != "ParmVarDecl":
                    continue
                if not p.get("name") or p.get("inner"):
                    fail(p, "named plain parameters are required")
                params.append(
                    {
                        "name": p["name"],
                        "type": normalized(p),
                        "spelling": p["type"]["qualType"],
                        "location": location(p),
                    }
                )
            spelling = node["type"]["qualType"]
            result["functions"].append(
                {
                    "name": identifier,
                    "params": params,
                    "returns": reader.abi_type(spelling.split(" (", 1)[0], aliases),
                    "location": location(node),
                }
            )
        else:
            fail(node, "unsupported public C declaration: " + kind)
    for group in ("records", "aliases", "functions"):
        names = [x["name"] for x in result[group]]
        if len(set(names)) != len(names):
            raise ValueError("duplicate C declaration in " + group)
    observed = {}
    for block in layouts.split("*** Dumping AST Record Layout"):
        match = re.search(r"^Type: struct (.+)$", block, re.MULTILINE)
        if not match or match[1] not in labels:
            continue
        layout = re.search(
            r"Size:(\d+)\s+DataSize:(\d+)\s+Alignment:(\d+)\s+FieldOffsets: \[([^]]*)\]", block
        )
        if not layout:
            raise ValueError("Clang layout is missing required fields")
        record = labels[match[1]]
        if record in observed:
            raise ValueError("duplicate Clang layout")
        observed[record] = {
            "sizeBits": int(layout[1]),
            "alignmentBits": int(layout[3]),
            "fieldOffsetsBits": [int(x.strip()) for x in layout[4].split(",") if x.strip()],
        }
    if set(observed) != complete:
        raise ValueError("Clang layout closure differs from declared records")
    for record in result["records"]:
        layout = observed[record["name"]]
        if len(layout["fieldOffsetsBits"]) != len(record["fields"]):
            raise ValueError("Clang field layout count mismatch")
        record["layout"] = layout
    verify()
    result["tool"] = {k: config["tools"]["clang"][k] for k in ("sha256", "size")}
    for item in dependency_refs:
        p = native / item["path"]
        if (
            p.stat().st_size != item["size"]
            or "sha256:" + hashlib.sha256(p.read_bytes()).hexdigest() != item["sha256"]
        ):
            raise ValueError("SDK header changed during compiler observation")
    result["sdkDependencies"] = dependency_refs
    return result
