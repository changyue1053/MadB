"""Finite Dart source generator consuming the exact admitted reader IR bytes.

Compilation/signing/installation-index pins belong to the subsequent Build
task. This Source stage never fabricates those future bytes or runnable status.
"""
from __future__ import annotations

import json
from pathlib import Path
import re

import reader
import wire

SOURCE_PATHS = sorted([
    "LICENSE", "NOTICE", "README.md", "declarations.json", "lib/madb_dart_ffi.dart",
    "lib/src/generated.dart", "lib/src/madb_runtime.dart", "lib/src/raw_bindings.dart",
    "native/bridge.c", "native/bridge.h", "native/identity.json", "native/include/madb_rust.h",
    "native/include/madb_rust_client.h", "native/native.dylib", "pubspec.lock", "pubspec.yaml",
])


def _policy_values(contract, selected):
    values = [(p["symbol_id"], p["slot"], p["values"]) for p in contract["policies"] if p["symbol_id"] in selected]
    return sorted(values, key=wire.canonical)


def _contract(request, root):
    reader.validate_request(request, root)
    wire.verify(root, [request["contract"]])
    value = wire.read_json(wire.confined(root, request["contract"]["path"]))
    if value["schema"] != "madb.contract-ir/1" or value["selector"] != request["selector"] or value["boundary_id"] != reader.BOUNDARY:
        raise ValueError("generator requires the selected reader IR")
    for field, source in (("input_digest", "source_digest"), ("contract_digest", "structure_digest"), ("policy_digest", "policy_digest")):
        if request[field] != value[source]:
            raise ValueError("IR and GenerateRequest identity differ")
    if value["source_digest"] != wire.source_digest(request["inputs"]) or set(value["required_features"]) != set(reader.FEATURES):
        raise ValueError("IR source or required semantics differ")
    selected = {f"madb.engine.{role}.v1" for role in reader.ROLES}
    if {s["public_id"] for s in value["symbols"]} != selected or len(value["symbols"]) != len(selected):
        raise ValueError("generator accepts only its requested six-role sync view")
    original = wire.read_json(wire.confined(root, "provider/contract.json"))
    if _policy_values(value, selected) != _policy_values(original, selected):
        raise ValueError("unsupported ownership/threading/close policy change")
    if value["types"] != reader.TYPES:
        # Type tables are sets in the public protocol, so order is immaterial.
        if sorted(value["types"], key=lambda t: t["id"]) != sorted(reader.TYPES, key=lambda t: t["id"]):
            raise ValueError("unsupported synchronous type mapping")
    names = {}
    for symbol in value["symbols"]:
        role = symbol["public_id"].split(".")[-2]
        shape = symbol["shape"]
        _, parameters, result = reader.ROLES[role]
        expected = {"kind": "sync", "calling_convention": "c", "link_name": shape["link_name"], "constants": [],
                    "parameters": [{"name": n, "type_id": t} for n, t in parameters], "result": result}
        if shape != expected or not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", shape["link_name"]):
            raise ValueError("unsupported role declaration")
        names[role] = shape["link_name"]
    return value, names


def generate(request, config, root, work):
    contract, names = _contract(request, root)
    package = Path(__file__).resolve().parent
    # The package's source files are separately in the immutable source lock;
    # templates remain owned inputs and are never modified in place.
    def template(name):
        return (package / "templates" / name).read_bytes()
    bridge = template("bridge.c.in").decode()
    for role, name in names.items():
        bridge = bridge.replace("@" + role + "@", name)
    if "@" in bridge:
        raise ValueError("unresolved bridge role")
    wire.write_new(work, "native/bridge.c", bridge.encode())
    wire.write_new(work, "native/bridge.h", template("bridge.h.in"))
    for source, destination in (
        ("provider/include/madb_rust.h", "native/include/madb_rust.h"),
        ("provider/include/madb_rust_client.h", "native/include/madb_rust_client.h"),
        ("provider/lib/libmadb_engine.dylib", "native/native.dylib"),
        ("provider/identity.json", "native/identity.json"),
        ("provider/LICENSE", "LICENSE"), ("provider/NOTICE", "NOTICE"),
        (f"packages/{reader.PACKAGE}/0.1.0/sdk/dart/ffi/madb_runtime.dart", "lib/src/madb_runtime.dart"),
    ):
        wire.write_new(work, destination, wire.confined(root, source).read_bytes())
    wire.write_new(work, "lib/madb_dart_ffi.dart", template("library.dart.in"))
    wire.write_new(work, "lib/src/generated.dart", template("generated.dart.in"))
    wire.write_new(work, "pubspec.yaml", template("pubspec.yaml.in"))
    wire.write_new(work, "pubspec.lock", template("pubspec.lock"))
    wire.write_new(work, "declarations.json", (wire.canonical(contract) + "\n").encode())
    wire.write_new(work, "README.md", b"Generated Dart Native synchronous source. Build the declared bridge and package pins before consuming. No callback/stream/async or unload API. Native identity remains the original candidate identity.\n")
    wire.check_tool(config["tools"]["libclang"])
    output = Path(work) / "lib/src/raw_bindings.dart"
    wire.command(config["tools"]["ffigen"], [Path(work) / "native/bridge.h", output,
        config["sdk_root"], Path(work) / "native/include", config["tools"]["libclang"]["path"]], work)
    wire.check_tool(config["tools"]["libclang"])
    raw = output.read_text()
    if "extends ffi.Opaque" in raw or "madb_fixture_observe" in raw:
        raise ValueError("ffigen omitted required record fields or exposed callbacks")
    for name in ["native_path", "bootstrap", "create_checked", "checksum", "request_close", "await_closed", "release_handle"]:
        if "madb_dart_" + name not in raw:
            raise ValueError("ffigen omitted a required forwarding entrypoint")
    files = []
    source_ids = sorted(s["public_id"] for s in contract["symbols"])
    for name in SOURCE_PATHS:
        item = wire.reference(work, name)
        # Whole-module notices and package metadata are emitted for this same
        # selected declaration set; OutputSet requires explicit provenance.
        item["sources"] = source_ids
        files.append(item)
    value = {"schema": "madb.output-set/1", "input_digest": request["input_digest"], "task_digest": request["task_digest"],
             "profile_digest": request["selector"]["profile_digest"], "manifest_path": "outputs.json", "files": files}
    value["generation_id"] = wire.semantic("madb.generation/1", {k: value[k] for k in ("input_digest", "task_digest", "profile_digest", "files")})
    wire.verify(root, request["inputs"] + [request["profile"], request["lock"], request["contract"]])
    return value
