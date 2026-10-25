"""Fixed Snapshot public profile: actual AST structure + explicit authored policy.

No opaque NAPI signature or C pointer is used to infer lifetime semantics.
Physical C seam facts remain a separately source-bound generation descriptor;
logical Snapshot is not falsely assigned mp_snapshot's pointer-bearing layout.
"""

from copy import deepcopy
import hashlib
import json
import re

ZERO = "sha256:" + "0" * 64
FEATURES = sorted(
    [
        "core.values@1",
        "core.exact-numbers@1",
        "core.errors@1",
        "core.resources@1",
        "core.backend-resources@1",
        "core.explicit-close@1",
        "core.callbacks@1",
        "core.async@1",
        "core.streams@1",
        "core.external-runtime-verification@1",
    ]
)


def projection(descriptor):
    """Compiler facts excluding location/tool bytes; those remain source provenance."""
    excluded = {
        "location",
        "source",
        "schema",
        "authority",
        "compilerApiVersion",
        "imports",
        "tools",
        "tool",
        "sdkDependencies",
        "dependencies",
    }

    def walk(v):
        if isinstance(v, dict):
            return {k: walk(x) for k, x in v.items() if k not in excluded}
        if isinstance(v, list):
            return [walk(x) for x in v]
        return v

    return walk(descriptor)


def source_id(key):
    h = hashlib.sha256(b"madb.source-id/1\n")
    for name in ("reader", "package", "module", "kind", "qualified_name", "overload"):
        value = key[name].encode()
        h.update(len(value).to_bytes(8, "big"))
        h.update(value)
    return "source:v1:" + h.hexdigest()


def lower(public, gateway, seam, policy, request, config):
    if policy.get("schema") != "madb.private.arkts-runtime-policy.v1":
        raise ValueError("unsupported C07 policy")
    for key, value in [
        ("public_surface", public),
        ("gateway_surface", gateway),
        ("c_seam_surface", seam),
    ]:
        if projection(value) != policy[key]:
            raise ValueError(
                "actual declarations changed outside authored C07 policy: " + key
            )
    return lower_public(public, policy, request, config)


def lower_public(public, policy, request, config):
    """Lower the common public roles after the selected backend checked its own declarations."""
    if policy.get("schema") != "madb.private.arkts-runtime-policy.v1":
        raise ValueError("unsupported C07 policy")
    if projection(public) != policy["public_surface"]:
        raise ValueError(
            "actual declarations changed outside authored C07 policy: public_surface"
        )
    if config["features"] != FEATURES:
        raise ValueError("C07 requires its complete explicit feature set")
    if policy.get("variant", "ordinary") != config.get("variant", "ordinary"):
        raise ValueError("role policy variant differs from actual native composition")
    types = {"unit": {"kind": "unit"}}
    if config.get("variant") == "test":
        types.update(deepcopy(policy["test_types"]))
    owner = "madb.c07." + config["boundary_id"]
    resource_names = {r["name"] for r in public["resources"]}
    for r in public["resources"]:
        types[r["name"]] = {
            "kind": "resource",
            "representation": "backend-handle",
            "owner": owner,
        }
    # Error is a backend-owned erased exception channel (code remains declared
    # in the source descriptor), not a new closable SDK owner or invented enum.
    channel = policy["error_channel"]
    error_name = channel["id"]
    if (
        channel["representation"] != "backend-handle"
        or channel["owner"] != "arkts-runtime"
        or error_name in types
    ):
        raise ValueError("invalid backend-owned erased exception channel")
    types[error_name] = {
        "kind": "resource",
        "representation": channel["representation"],
        "owner": channel["owner"],
    }
    for record in public["records"]:
        fields = []
        for field in record["fields"]:
            key = record["name"] + "." + field["name"]
            mapping = policy["values"].get(key)
            if not mapping or mapping["source_type"] != field["type"]:
                raise ValueError("field lacks exact authored value mapping: " + key)
            tid = mapping["type_id"]
            definition = mapping["definition"]
            if tid in types and types[tid] != definition:
                raise ValueError("conflicting policy type identity")
            types[tid] = deepcopy(definition)
            fields.append({"name": field["name"], "type_id": tid})
        types[record["name"]] = {"kind": "record", "layout": "logical", "fields": fields}

    def type_id(t, context):
        mapping = policy.get("value_slots", {}).get(context)
        if mapping is not None:
            if mapping["source_type"] != t or mapping["type_id"] not in types:
                raise ValueError(
                    "test slot differs from explicit type policy: " + context
                )
            return mapping["type_id"]
        if t["tag"] == "void":
            return "unit"
        if t["tag"] == "reference" and t["name"] in types:
            return t["name"]
        if t["tag"] == "callback":
            name = context + ".callback"
            types[name] = {
                "kind": "callback",
                "parameters": [
                    {
                        "name": p["name"],
                        "type_id": type_id(p["type"], context + "." + p["name"]),
                    }
                    for p in t["params"]
                ],
                "result": type_id(t["returns"], context + ".return"),
                "calling_convention": "system",
                "throws": error_name,
            }
            return name
        raise ValueError("unsupported public value mapping: " + context)

    symbols = []
    bindings = []
    calls = public["functions"] + [m for r in public["resources"] for m in r["methods"]]
    actual_names = {".".join(filter(None, [f["owner"], f["name"]])) for f in calls}
    if actual_names != set(policy["calls"]):
        raise ValueError(
            "C07 call policy closure differs from actual public declarations"
        )
    public_ids = {}
    for name, role in policy["calls"].items():
        explicit_id = role.get("public_id")
        if (
            not isinstance(explicit_id, str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._:/@+\-]{0,511}", explicit_id) is None
        ):
            raise ValueError("call lacks valid authored public_id: " + name)
        if explicit_id in public_ids.values():
            raise ValueError("duplicate authored public_id: " + explicit_id)
        public_ids[name] = explicit_id
    for f in calls:
        name = ".".join(filter(None, [f["owner"], f["name"]]))
        role = policy["calls"][name]
        public_id = public_ids[name]
        parameters = []
        if f["owner"]:
            if f["owner"] not in resource_names:
                raise ValueError("method owner is not a declared resource")
            parameters.append({"name": "self", "type_id": f["owner"]})
        parameters += [
            {"name": p["name"], "type_id": type_id(p["type"], name + "." + p["name"])}
            for p in f["params"]
        ]
        returned = f["returns"]
        is_promise = returned["tag"] == "promise"
        result = type_id(returned["item"] if is_promise else returned, name + ".return")
        kind = role["kind"]
        if (kind == "async") != is_promise or kind not in ("sync", "async", "stream"):
            raise ValueError(
                "authored call kind differs from actual Promise declaration: " + name
            )
        if kind == "stream" and not any(
            types[p["type_id"]]["kind"] == "callback" for p in parameters
        ):
            raise ValueError("stream has no actual callback declaration")
        key = {
            "reader": "org.madb.arkts-compiler@1",
            "package": config["source_package"],
            "module": "Boundary",
            "kind": "method" if f["owner"] else "function",
            "qualified_name": name,
            "overload": "none",
        }
        location = {k: f["location"][k] for k in ("path", "line", "column")}
        shape = {
            "kind": kind,
            "calling_convention": "system",
            "link_name": name,
            "parameters": parameters,
            "result": result,
            "throws": error_name,
            "constants": [],
        }
        symbols.append(
            {
                "source_key": key,
                "source_id": source_id(key),
                "public_id": public_id,
                "public_id_origin": "explicit",
                "location": location,
                "precision": "compiler-derived",
                "shape": shape,
                "shape_digest": ZERO,
            }
        )
        slots = [({"kind": "call"}, role["policy"])]
        for slot, tid in [
            *[(dict(kind="parameter", name=p["name"]), p["type_id"]) for p in parameters],
            ({"kind": "return"}, result),
        ]:
            kind = types[tid]["kind"]
            if kind == "resource":
                values = policy["slots"]["resource"]
            elif kind == "callback":
                values = policy["slots"]["callback"]
            else:
                values = policy["slots"]["value"]
            slot_name = slot["kind"] + ("." + slot["name"] if "name" in slot else "")
            values = role.get("slots", {}).get(slot_name, values)
            slots.append((slot, values))
        for slot, values in slots:
            suffix = slot["kind"] + ("." + slot["name"] if "name" in slot else "")
            bindings.append(
                {
                    "selector": {"kind": "public_id", "id": public_id},
                    "slot": slot,
                    "families": [],
                    "values": deepcopy(values),
                    "origin": {
                        "authority": "project-policy",
                        "id": public_id + "." + suffix,
                        "location": {"path": config["policy"], "line": 1, "column": 1},
                    },
                }
            )
    contract = {
        "schema": "madb.contract-ir/1",
        "sdk_namespace": "madb",
        "boundary_id": config["boundary_id"],
        "selector": request["selector"],
        "source_digest": ZERO,
        "structure_digest": ZERO,
        "policy_digest": ZERO,
        "required_features": FEATURES,
        "types": [{"id": k, "definition": v} for k, v in sorted(types.items())],
        "symbols": sorted(symbols, key=lambda x: x["public_id"]),
        "policies": [],
        "observations": [],
    }
    return contract, bindings


def abi_projection(descriptor, contract, runtime_policy):
    """Only checked type/protocol facts; byte provenance remains in Source/build."""
    logical = {
        "types": contract["types"],
        "symbols": [
            {"public_id": s["public_id"], "shape": s["shape"]}
            for s in contract["symbols"]
        ],
        "policies": [
            {k: p[k] for k in ("symbol_id", "slot", "values")}
            for p in contract["policies"]
        ],
        "required_features": contract["required_features"],
    }
    physical = {
        key: projection(descriptor[key])
        for key in ("public", "gateway", "c_seam")
        if key in descriptor
    }
    if "rust" in descriptor:
        rust = descriptor["rust"]
        physical["rust"] = {
            "exports": [
                {key: export[key] for key in ("name", "arity", "parameters", "rawReturn")}
                for export in rust["macro"]["descriptor"]["exports"]
            ],
            "napi_features": [
                {key: package[key] for key in ("name", "features")}
                for package in rust["resolved_packages"]
                if package["name"]
                in (
                    "napi-ohos",
                    "napi-sys-ohos",
                    "napi-build-ohos",
                    "napi-derive-ohos",
                    "napi-derive-backend-ohos",
                )
            ],
        }
    if "kotlin" in descriptor:
        kotlin = descriptor["kotlin"]
        policy = kotlin["frontend_policy"]
        physical["kotlin"] = {
            "facts": projection(kotlin["facts"]),
            "declaration": projection(kotlin["descriptor"]),
            "record": {
                key: projection(kotlin["record_schema"][key])
                for key in ("qualifiedName", "fields")
            },
            "notification_interfaces": policy["notification_interfaces"],
            "parameters": policy["parameters"],
            "execution_domain": runtime_policy["execution_domain"],
        }
    if "diagnostics" in descriptor:
        physical["diagnostics"] = descriptor["diagnostics"]["protocols"]
    roles = {
        key: runtime_policy[key]
        for key in (
            "values",
            "calls",
            "slots",
            "limits",
            "error_channel",
            "value_slots",
            "test_types",
            "execution_domain",
            "runtime_reopen",
        )
        if key in runtime_policy
    }
    if "value_slots" in roles:
        roles["value_slots"] = {
            name: {key: value for key, value in slot.items() if key != "layout"}
            for name, slot in roles["value_slots"].items()
        }
    return {"logical": logical, "physical": physical, "runtime_roles": roles}
