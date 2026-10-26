"""Finite natural-Rust values: independently check the physical ABI, then lower logical values."""
from native_ast import abi_projection
from lowering import source_id
from support import NativeError, semantic

ZERO = "sha256:" + "0" * 64
SEMANTICS = "madb.native-values/1"
BASE_CONSTANTS = {
    "MADB_VALUE_CALL_OK": 0, "MADB_VALUE_CALL_BOOTSTRAP_FAILED": 1,
    "MADB_VALUE_CALL_INTERFACE_MISMATCH": 2, "MADB_VALUE_CALL_PROFILE_MISMATCH": 3,
    "MADB_VALUE_CALL_OPERATION_FAILED": 4, "MADB_VALUE_STATUS_OK": 0,
    "MADB_VALUE_STATUS_INVALID_ARGUMENT": 1, "MADB_VALUE_STATUS_INTERNAL_FAILURE": 8,
    "MADB_VALUE_STATUS_RESOURCE_EXHAUSTED": 9, "MADB_VALUE_STATUS_DOMAIN_ERROR": 10,
    "MADB_VALUE_STATUS_UNSUPPORTED": 11,
}


def enum_constants(plan):
    return {f"{plan['namespace']}_{error['name']}_{variant['name']}": variant["value"]
            for error in plan["errors"] for variant in error["variants"]}


def expected_exports(plan):
    return [plan["namespace"] + "_bootstrap_v1", *[plan["namespace"] + "_raw_" + f["name"] for f in plan["functions"]]]


def validate_surface(surface, plan, config):
    if config.get("semantics") != SEMANTICS or config.get("provider") != "rust":
        raise NativeError("NATIVE001", "unsupported finite value backend semantics")
    prefix = plan["namespace"]
    records = {
        "MadbValueError": [dict(name="domain", type="u32"), dict(name="code", type="i32")],
        "MadbValueUtf8_64": [dict(name="byte_count", type="u32"), dict(name="bytes", type="u8[64]")],
    }
    layouts = {
        "MadbValueError": dict(size=8, alignment=4, fields=[dict(name="domain", offset=0, size=4), dict(name="code", offset=4, size=4)]),
        "MadbValueUtf8_64": dict(size=68, alignment=4, fields=[dict(name="byte_count", offset=0, size=4), dict(name="bytes", offset=4, size=64)]),
    }
    for record in plan["records"]:
        name = prefix + "_" + record["name"]
        records[name] = [dict(name=f["name"], type="MadbValueUtf8_64" if f["kind"] == "String" else f["kind"]) for f in record["fields"]]
        fields, offset = [], 0
        for field in record["fields"]:
            size = 68 if field["kind"] == "String" else 4
            fields.append(dict(name=field["name"], offset=offset, size=size)); offset += size
        layouts[name] = dict(size=offset, alignment=4, fields=fields)
    if surface["records"] != records or surface["layouts"] != layouts:
        raise NativeError("NATIVE005", "actual finite record fields/layout differ from the Rust plan")
    aliases = {"MadbValueStatus": "i32", "MadbValueCallKind": "u32", **{name: name for name in records}}
    if surface["aliases"] != aliases:
        raise NativeError("NATIVE005", "actual finite value typedef set differs")
    constants = {**BASE_CONSTANTS, **enum_constants(plan)}
    if surface["constants"] != constants:
        raise NativeError("NATIVE005", "actual finite value constants or error tags differ")
    functions = {prefix + "_bootstrap_v1": dict(result="i32", parameters=[dict(name="version", type="u32"), dict(name="out_identity", type="u8 *"), dict(name="capacity", type="u64")])}
    for function in plan["functions"]:
        record = prefix + "_" + function["record"]
        parameters = [dict(name="input", type="const " + record + " *")]
        if len(function["parameters"]) == 2:
            parameters.append(dict(name=function["parameters"][1]["name"], type="i32"))
        parameters.extend([dict(name="out_value", type=record + " *"), dict(name="out_error", type="MadbValueError *")])
        functions[prefix + "_raw_" + function["name"]] = dict(result="i32", parameters=parameters)
    actual = {name: {key: fn[key] for key in ("result", "parameters")} for name, fn in surface["functions"].items()}
    if actual != functions:
        raise NativeError("NATIVE005", "actual finite value function set/signatures differ")


def interface_identity(surface, plan, config, profile_digest):
    validate_surface(surface, plan, config)
    return dict(schema="madb.native-identity/1", bootstrap_version=1,
                interface_digest=semantic("madb.native-interface/1", dict(semantics=SEMANTICS, bootstrap_version=1, abi=abi_projection(surface))),
                profile_digest=profile_digest)


def lower(surface, plan, config, request, project):
    validate_surface(surface, plan, config)
    if plan["namespace"] != project["sdk_namespace"]:
        raise NativeError("NATIVE006", "reader namespace differs from project")
    types = [dict(id="madb.recipe.unit", definition=dict(kind="unit"))]
    types += [dict(id="madb.recipe." + name, definition=dict(kind="integer", width=width, signed=signed)) for name, width, signed in (("u8", 8, False), ("u32", 32, False), ("i32", 32, True))]
    types += [dict(id="madb.recipe.String", definition=dict(kind="string", encoding="utf-8", max_bytes=64)),
              dict(id="madb.recipe.identity_bytes", definition=dict(kind="buffer", element="madb.recipe.u8", length_unit="bytes", encoding="none", mutable=False, max_bytes=64))]
    for record in plan["records"]:
        types.append(dict(id=record["name"], definition=dict(kind="record", layout="logical", fields=[dict(name=f["name"], type_id="madb.recipe." + f["kind"]) for f in record["fields"]])))
    for error in plan["errors"]:
        types.append(dict(id=error["name"], definition=dict(kind="variant", tag_type="madb.recipe.i32", unknown="reject",
            cases=[dict(name=v["name"], tag=dict(kind="integer", type="i32", decimal=str(v["value"])), payload=None) for v in error["variants"]])))
    symbols = []
    items = [("$recipe.bootstrap.v1", plan["namespace"] + "_bootstrap_v1", [dict(name="version", type_id="madb.recipe.u32")], "madb.recipe.identity_bytes", None,
              dict(path=config["source"], line=1, column=1))]
    items += [(f["name"], plan["namespace"] + "_raw_" + f["name"], [dict(name=p["name"], type_id=p["kind"] if index == 0 else "madb.recipe." + p["kind"]) for index, p in enumerate(f["parameters"])], f["record"], f["error"], f["location"]) for f in plan["functions"]]
    for name, link, parameters, result, error, location in items:
        key = dict(reader="org.madb.native-values@1", package=project["project"], module=config["source"], kind="function", qualified_name=name, overload="none")
        shape = dict(kind="sync", calling_convention="c", link_name=link, parameters=parameters, result=result, constants=[])
        if error is not None:
            shape["throws"] = error
        symbols.append(dict(source_key=key, source_id=source_id(key), public_id=plan["namespace"] + "." + source_id(key)[10:],
                            public_id_origin="derived", location=location, precision="artifact-derived", shape=shape, shape_digest=ZERO))
    inputs = [{key: item[key] for key in ("path", "sha256", "size")} for item in request["inputs"]]
    features = ["core.values@1", "core.exact-numbers@1"]
    if plan["errors"]:
        features += ["core.errors@1", "core.variants@1"]
    # Logical record representation does not claim C layout. The ABI is separately
    # compiler-observed and fixed by native-interface/1, not injected as logical IR layout.
    return dict(schema="madb.contract-ir/1", sdk_namespace=plan["namespace"], boundary_id=request["boundary_id"], selector=request["selector"],
                source_digest=semantic("madb.source/1", sorted(inputs, key=lambda x: x["path"])), structure_digest=ZERO, policy_digest=ZERO,
                required_features=sorted(features), types=types, symbols=sorted(symbols, key=lambda s: s["public_id"]), policies=[], observations=[])
