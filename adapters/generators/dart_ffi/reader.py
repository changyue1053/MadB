"""Finite public C-header reader for the approved Dart Native sync recipe.

The original candidate/contract/identity are immutable inputs. Real Clang
declarations validate the selected C ABI; no private Rust lowering is imported.
The public native metadata finalizer computes the new selector's IR identities
and policy from the separately frozen project/profile inputs.
"""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import shlex

import wire

PACKAGE = "org.madb.dart-ffi"
PROFILE = "dart-c-ffi-sync-aarch64-apple-darwin"
ROUTE = "org.madb.dart.c-ffi"
BOUNDARY = "dart-c-sync"
FEATURES = ["core.exact-numbers@1", "core.explicit-close@1", "core.resources@1", "core.values@1"]
HEADER = "provider/include/madb_rust.h"
CLIENT = "provider/include/madb_rust_client.h"

# This is a finite role-to-C ABI contract, not a generic C lowering promise.
# Public IDs and logical shapes come from the locked public native contract;
# their actual C entrypoints are independently observed below.
ROLES = {
    "bootstrap": ("MadbStatus (uint32_t, uint8_t *, uint64_t)", [("requested_version", "u32")], "identity_bytes"),
    "create": ("MadbStatus (MadbHandle *, MadbError *)", [], "handle"),
    "checksum": ("MadbStatus (MadbHandle, const uint8_t *, uint64_t, uint64_t *, MadbError *)", [("engine", "handle"), ("data", "bytes")], "u64"),
    "request_close": ("MadbStatus (MadbHandle, MadbError *)", [("handle", "handle")], "unit"),
    "await_closed": ("MadbStatus (MadbHandle, uint32_t, MadbError *)", [("handle", "handle"), ("timeout_ms", "u32")], "unit"),
    "release_handle": ("MadbStatus (MadbHandle, MadbError *)", [("handle", "handle")], "unit"),
}
TYPES = [
    {"id": "unit", "definition": {"kind": "unit"}},
    *[{"id": name, "definition": {"kind": "integer", "signed": signed, "width": width}}
      for name, signed, width in (("u8", False, 8), ("u32", False, 32), ("u64", False, 64), ("i32", True, 32))],
    *[{"id": name, "definition": {"kind": "buffer", "element": "u8", "encoding": "none", "length_unit": "bytes", "max_bytes": size, "mutable": False}}
      for name, size in (("bytes", 65536), ("identity_bytes", 64))],
    {"id": "error", "definition": {"kind": "record", "layout": "c", "fields": [{"name": "domain", "type_id": "u32"}, {"name": "code", "type_id": "i32"}]}},
    {"id": "handle", "definition": {"kind": "resource", "owner": "madb-first-route", "representation": "registry-u64"}},
]


def validate_request(request, root):
    profile = wire.read_json(wire.confined(root, request["profile"]["path"]))
    if request["schema"] not in ("madb.read-request/1", "madb.generate-request/1") or request["boundary_id"] != BOUNDARY:
        raise ValueError("unsupported finite Dart boundary")
    if profile["schema"] != "madb.profile/2" or profile["id"] != PROFILE:
        raise ValueError("Dart reader requires its explicit profile/2")
    selection = {"profile_id": PROFILE, "profile_digest": request["selector"]["profile_digest"],
                 "route": ROUTE, "direction": "consumer-to-provider", "target": "aarch64-apple-darwin"}
    if request["selector"] != selection:
        raise ValueError("Dart synchronous selection differs")
    if set(profile["required_features"]) != {*FEATURES, "core.extension-composition@1"}:
        raise ValueError("unsupported requested feature, including callback/async/stream")
    wire.verify(root, request["inputs"] + [request["profile"], request["lock"]])
    return profile


def _header_dependencies(clang, common, root, work, recorded):
    output = wire.command(clang, [*common, "-M", "-MT", "declarations", wire.confined(root, CLIENT)], work).stdout.decode()
    dependencies = shlex.split(output.replace("\\\n", " ").split(":", 1)[1])
    allowed = {}
    for item in recorded:
        path = Path(item["path"])
        if not path.is_absolute() or not path.is_file():
            raise ValueError("compiler header receipt requires actual absolute files")
        if wire.digest(path.read_bytes()) != item["sha256"] or path.stat().st_size != item["size"]:
            raise ValueError("compiler header changed")
        allowed[str(path.resolve())] = item
    for name in dependencies:
        path = Path(name).resolve(strict=True)
        if path.is_relative_to(root):
            relative = path.relative_to(root).as_posix()
            if relative not in (HEADER, CLIENT):
                raise ValueError("unexpected package header dependency")
        elif str(path) not in allowed:
            raise ValueError("compiler read an unrecorded header: " + str(path))
    return sorted(str(Path(p).resolve()) for p in dependencies)


def observe(root, config, work):
    root, work = Path(root).resolve(strict=True), Path(work).resolve(strict=True)
    clang = config["tools"]["clang"]
    common = ["-x", "c", "-std=c11", "-Werror", *clang["arguments"]]
    dependencies = _header_dependencies(clang, common, root, work, config["compiler_headers"])
    result = wire.command(clang, [*common, "-fsyntax-only", "-Xclang", "-ast-dump=json", wire.confined(root, CLIENT)], work)
    ast = json.loads(result.stdout)
    declarations = {}
    for node in ast["inner"]:
        if node.get("kind") == "FunctionDecl" and node.get("name", "").startswith("madb_fixture_"):
            if node["name"] in declarations or node.get("variadic") or node.get("storageClass") == "static":
                raise ValueError("ambiguous or unsupported C declaration")
            declarations[node["name"]] = node
    # The original header supplies typedef spellings; verify their actual ABI,
    # signedness, field offsets and status values with the same locked compiler.
    checks = [
        "sizeof(MadbHandle)==8", "sizeof(MadbStatus)==4", "sizeof(MadbError)==8", "_Alignof(MadbError)==4",
        "__builtin_offsetof(MadbError,domain)==0", "__builtin_offsetof(MadbError,code)==4",
        "__builtin_types_compatible_p(MadbHandle,uint64_t)", "__builtin_types_compatible_p(MadbStatus,int32_t)",
        "__builtin_types_compatible_p(__typeof__(((MadbError*)0)->domain),uint32_t)",
        "__builtin_types_compatible_p(__typeof__(((MadbError*)0)->code),int32_t)",
    ]
    checks += [f"{name}=={index}" for index, name in enumerate([
        "MADB_OK", "MADB_INVALID_ARGUMENT", "MADB_INVALID_HANDLE", "MADB_CLOSING", "MADB_CLOSED", "MADB_BUSY",
        "MADB_TIMEOUT", "MADB_WOULD_DEADLOCK", "MADB_INTERNAL_FAILURE", "MADB_RESOURCE_EXHAUSTED", "MADB_DOMAIN_ERROR", "MADB_UNSUPPORTED"])]
    source = '#include "' + str(wire.confined(root, CLIENT)) + '"\n' + "\n".join(f'_Static_assert({check},"finite Dart ABI");' for check in checks)
    wire.command(clang, [*common, "-fsyntax-only", "-"], work, stdin=source.encode())
    _header_dependencies(clang, common, root, work, config["compiler_headers"])
    return declarations, dependencies


def seed(request, config, root, work):
    validate_request(request, root)
    declarations, _ = observe(root, config, work)
    original = wire.read_json(wire.confined(root, "provider/contract.json"))
    identity = wire.read_json(wire.confined(root, "provider/identity.json"))
    if identity["schema"] != "madb.native-identity/1" or identity["bootstrap_version"] != 1:
        raise ValueError("unsupported original native identity")
    if identity["profile_digest"] != original["selector"]["profile_digest"]:
        raise ValueError("native profile identity differs from the original contract")
    original_types = {t["id"]: t for t in original["types"]}
    if any(original_types.get(t["id"]) != t for t in TYPES):
        raise ValueError("unsupported original synchronous type semantics")
    symbols = []
    header = wire.confined(root, HEADER).read_bytes()
    for role, (signature, parameters, result) in ROLES.items():
        public_id = f"madb.engine.{role}.v1"
        matches = [s for s in original["symbols"] if s["public_id"] == public_id]
        if len(matches) != 1:
            raise ValueError("missing or duplicate synchronous semantic role")
        shape = matches[0]["shape"]
        expected = {"kind": "sync", "calling_convention": "c", "constants": [], "link_name": shape["link_name"],
                    "parameters": [{"name": n, "type_id": t} for n, t in parameters], "result": result}
        if shape != expected:
            raise ValueError("unsupported synchronous role shape: " + role)
        node = declarations.get(shape["link_name"])
        if node is None or node["type"]["qualType"] != signature:
            raise ValueError("public C declaration differs from logical role: " + role)
        location = node["loc"]
        offset = location["offset"]
        if header[offset:offset + len(shape["link_name"])] != shape["link_name"].encode():
            raise ValueError("declaration location does not name the locked public header")
        symbols.append({"public_id": public_id, "public_id_origin": "explicit", "source_id": "source:v1:" + "0" * 64,
                        "source_key": {"reader": PACKAGE + "@0.1.0", "package": PACKAGE, "module": HEADER,
                                       "qualified_name": shape["link_name"], "kind": "release" if role == "release_handle" else "function", "overload": "none"},
                        "precision": "compiler-derived", "location": {"path": HEADER, "line": header[:offset].count(b"\n") + 1, "column": location["col"]},
                        "shape": deepcopy(shape), "shape_digest": "sha256:" + "0" * 64})
    source = wire.source_digest(request["inputs"])
    compiler = {k: config["tools"]["clang"][k] for k in ("id", "version", "sha256")}
    value = {"schema": "madb.contract-ir/1", "boundary_id": BOUNDARY, "sdk_namespace": "madb",
             "selector": request["selector"], "source_digest": source, "required_features": FEATURES,
             "structure_digest": "sha256:" + "0" * 64, "policy_digest": "sha256:" + "0" * 64,
             "types": deepcopy(TYPES), "symbols": sorted(symbols, key=lambda s: s["public_id"]), "policies": [],
             "observations": [{"kind": "layout", "subject": "error", "precision": "compiler-derived", "producer": compiler,
                               "input_digest": source, "selector": request["selector"],
                               "result": {"size": 8, "alignment": 4, "fields": [{"name": "domain", "offset": 0, "size": 4}, {"name": "code", "offset": 4, "size": 4}]}}]}
    wire.verify(root, request["inputs"] + [request["profile"], request["lock"]])
    return value


def read(request, config, root, work):
    value = seed(request, config, root, work)
    completed = wire.command(config["tools"]["finalizer"], ["metadata", root], work,
                             stdin=wire.canonical(value).encode(), maximum=wire.MAX_DOCUMENT)
    return json.loads(completed.stdout)
