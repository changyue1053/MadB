"""Explicit first-route ABI v1 lowering. It never discovers semantics by names."""
from __future__ import annotations

import hashlib

from native_ast import abi_projection
from support import NativeError, semantic


ZERO = "sha256:" + "0" * 64
CALLBACK = "fn(void *,u64,u32)->void"
ROLES = {"create", "checksum", "request_close", "await_closed", "release_handle", "observe", "bootstrap"}
STATUSES = dict(zip(("MADB_OK", "MADB_INVALID_ARGUMENT", "MADB_INVALID_HANDLE", "MADB_CLOSING", "MADB_CLOSED", "MADB_BUSY", "MADB_TIMEOUT", "MADB_WOULD_DEADLOCK", "MADB_INTERNAL_FAILURE", "MADB_RESOURCE_EXHAUSTED", "MADB_DOMAIN_ERROR", "MADB_UNSUPPORTED"), range(12)))
SIGNATURES = {
    "create": ["u64 *", "MadbError *"],
    "request_close": ["u64", "MadbError *"],
    "await_closed": ["u64", "u32", "MadbError *"],
    "release_handle": ["u64", "MadbError *"],
    "observe": ["u64", CALLBACK, "void *", "u64 *", "MadbError *"],
    "bootstrap": ["u32", "u8 *", "u64"],
}


def validate_surface(surface, config):
    if config.get("schema") != "madb.rust-c-backend/1" or config.get("semantics") != "madb.first-route/1":
        raise NativeError("NATIVE001", "unsupported backend configuration/semantic version")
    provider = config.get("provider")
    if provider not in ("rust", "c"):
        raise NativeError("NATIVE001", "provider must be rust or c")
    exports = config.get("exports", [])
    names = [item["name"] for item in exports]
    roles = [item["role"] for item in exports]
    public_ids = [item["public_id"] for item in exports]
    required = ROLES if provider == "rust" else {"checksum", "bootstrap"}
    if len(set(names)) != len(names) or set(roles) != required or len(roles) != len(required) or len(set(public_ids)) != len(public_ids):
        raise NativeError("NATIVE001", "exports must map each supported role and public identity exactly once")
    if set(names) != set(surface["functions"]):
        raise NativeError("NATIVE004", "actual public exports differ: missing=" + str(sorted(set(names) - set(surface["functions"])))
                          + "; extra=" + str(sorted(set(surface["functions"]) - set(names))))
    if surface["records"] != {"MadbError": [{"name": "domain", "type": "u32"}, {"name": "code", "type": "i32"}]}:
        raise NativeError("NATIVE005", "MadbError must have exactly domain:u32 and code:i32")
    expected_layout = {"size": 8, "alignment": 4, "fields": [{"name": "domain", "offset": 0, "size": 4}, {"name": "code", "offset": 4, "size": 4}]}
    if surface["layouts"].get("MadbError") != expected_layout:
        raise NativeError("NATIVE005", "compiler-observed MadbError layout differs from ABI v1")
    constants = surface.get("constants", {})
    if any(constants.get(name) != value for name, value in STATUSES.items()):
        raise NativeError("NATIVE005", "native status constants differ from ABI v1")
    allowed_constants = dict(STATUSES, MADB_MAX_HANDLES=4096, MADB_MAX_SUBSCRIPTIONS_PER_ENGINE=64)
    if any(allowed_constants.get(name) != value for name, value in constants.items()):
        raise NativeError("NATIVE004", "unsupported public constant")
    allowed_aliases = {"MadbStatus": "i32", "MadbError": "MadbError", "MadbHandle": "u64", "MadbStateCallback": CALLBACK}
    if any(allowed_aliases.get(name) != value for name, value in surface["aliases"].items()):
        raise NativeError("NATIVE004", "unsupported public typedef or callback")
    for item in exports:
        fn = surface["functions"][item["name"]]
        expected = SIGNATURES.get(item["role"])
        if item["role"] == "checksum":
            expected = (["u64"] if provider == "rust" else []) + ["const u8 *", "u64", "u64 *", "MadbError *"]
        if fn["result"] != "i32" or [p["type"] for p in fn["parameters"]] != expected:
            raise NativeError("NATIVE005", f"ABI signature mismatch for {item['name']} ({item['role']})")
    return exports


def source_id(key):
    value = hashlib.sha256(b"madb.source-id/1\n")
    for name in ("reader", "package", "module", "kind", "qualified_name", "overload"):
        data = key[name].encode()
        value.update(len(data).to_bytes(8, "big"))
        value.update(data)
    return "source:v1:" + value.hexdigest()


def interface_identity(surface, config, profile_digest):
    validate_surface(surface, config)
    interface = semantic("madb.native-interface/1", {"semantics": config["semantics"], "bootstrap_version": 1, "abi": abi_projection(surface)})
    return {"schema": "madb.native-identity/1", "bootstrap_version": 1,
            "interface_digest": interface, "profile_digest": profile_digest}


def lower(surface, config, request, project):
    exports = validate_surface(surface, config)
    namespace = project["sdk_namespace"]
    types = [{"id": "unit", "definition": {"kind": "unit"}}]
    for name, width, signed in [("u8", 8, False), ("u32", 32, False), ("u64", 64, False), ("i32", 32, True)]:
        types.append({"id": name, "definition": {"kind": "integer", "width": width, "signed": signed}})
    for name, maximum in [("bytes", 65536), ("identity_bytes", 64)]:
        types.append({"id": name, "definition": {"kind": "buffer", "element": "u8", "length_unit": "bytes", "encoding": "none", "mutable": False, "max_bytes": maximum}})
    types.append({"id": "error", "definition": {"kind": "record", "layout": "c", "fields": [{"name": "domain", "type_id": "u32"}, {"name": "code", "type_id": "i32"}]}})
    features = ["core.values@1", "core.exact-numbers@1"]
    if config["provider"] == "rust":
        types.extend([
            {"id": "handle", "definition": {"kind": "resource", "representation": "registry-u64", "owner": project["project"]}},
            {"id": "state_callback", "definition": {"kind": "callback", "parameters": [{"name": "sequence", "type_id": "u64"}, {"name": "state", "type_id": "u32"}], "result": "unit", "calling_convention": "c"}},
        ])
        features += ["core.resources@1", "core.callbacks@1", "core.explicit-close@1"]
    symbols = []
    for export in exports:
        fn, role = surface["functions"][export["name"]], export["role"]
        native = fn["parameters"]
        parameters, result = [], "unit"
        def parameter(index, kind):
            return {"name": native[index]["name"], "type_id": kind}
        if role in ("request_close", "await_closed", "release_handle", "observe"):
            parameters.append(parameter(0, "handle"))
        if role == "create":
            result = "handle"
        elif role == "checksum":
            index = 0
            if config["provider"] == "rust":
                parameters.append(parameter(0, "handle"))
                index = 1
            parameters.append(parameter(index, "bytes"))
            result = "u64"
        elif role == "await_closed":
            parameters.append(parameter(1, "u32"))
        elif role == "observe":
            parameters.append(parameter(1, "state_callback"))
            result = "handle"
        elif role == "bootstrap":
            parameters.append(parameter(0, "u32"))
            result = "identity_bytes"
        key = {"reader": "org.madb.rust-c-abi@1", "package": project["project"], "module": config["source"],
               "kind": "release" if role == "release_handle" else "function", "qualified_name": export["name"], "overload": "none"}
        location = dict(fn["location"])
        location["path"] = fn.get("logical_source_path", config["source"])
        symbols.append({"source_key": key, "source_id": source_id(key), "public_id": export["public_id"],
                        "public_id_origin": "explicit", "location": location,
                        "precision": "artifact-derived" if config["provider"] == "rust" else "compiler-derived",
                        "shape": {"kind": "sync", "calling_convention": "c", "link_name": export["name"],
                                  "parameters": parameters, "result": result, "constants": []}, "shape_digest": ZERO})
    producer = config["tools"]["clang"]
    if not producer.get("id") or not producer.get("version"):
        raise NativeError("NATIVE001", "Clang producer id/version are required for layout provenance")
    inputs = [{key: item[key] for key in ("path", "sha256", "size")} for item in request["inputs"]]
    source_digest = semantic("madb.source/1", sorted(inputs, key=lambda x: x["path"]))
    return {"schema": "madb.contract-ir/1", "sdk_namespace": namespace, "boundary_id": request["boundary_id"],
            "selector": request["selector"], "source_digest": source_digest, "structure_digest": ZERO, "policy_digest": ZERO,
            "required_features": sorted(features), "types": types, "symbols": sorted(symbols, key=lambda s: s["public_id"]), "policies": [],
            "observations": [{"subject": "error", "kind": "layout", "precision": "compiler-derived", "input_digest": source_digest,
                              "selector": request["selector"], "producer": {key: producer[key] for key in ("id", "version", "sha256")},
                              "result": surface["layouts"]["MadbError"]}]}
