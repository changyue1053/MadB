"""Lower version-matched compiler/IDL descriptors; no source text establishes types."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re

ZERO = "sha256:" + "0" * 64


class UnsupportedDeclaration(ValueError):
    pass


def source_id(key):
    digest = hashlib.sha256(b"madb.source-id/1\n")
    for name in ("reader", "package", "module", "kind", "qualified_name", "overload"):
        value = key[name].encode()
        digest.update(len(value).to_bytes(8, "big"))
        digest.update(value)
    return "source:v1:" + digest.hexdigest()


class Types:
    def __init__(self, descriptor, owner):
        self.descriptor = descriptor
        self.owner = owner
        self.definitions = {"unit": {"kind": "unit"}}
        self.named = {item["name"]: item for item in descriptor["types"]}
        if len(self.named) != len(descriptor["types"]):
            raise UnsupportedDeclaration("duplicate named declaration")

    def add(self, name, value):
        previous = self.definitions.get(name)
        if previous is not None and previous != value:
            raise UnsupportedDeclaration("conflicting logical type " + name)
        self.definitions[name] = value
        return name

    def reference(self, value):
        if value is None:
            return "unit"
        kind = value["kind"]
        if re.fullmatch(r"[ui](8|16|32|64)", kind):
            return self.add(kind, dict(kind="integer", width=int(kind[1:]), signed=kind[0] == "i"))
        if kind in ("f32", "f64"):
            return self.add(kind, dict(kind="float", width=int(kind[1:])))
        if kind == "bool":
            return self.add("bool", dict(kind="boolean"))
        if kind == "string":
            return self.add("string", dict(kind="string", encoding="utf-8", max_bytes=1024))
        if kind == "optional":
            inner = self.reference(value["inner"])
            return self.add("optional." + inner, dict(kind="nullable", inner=inner))
        if kind == "bytes" or kind == "sequence" and value["inner"] == {"kind": "u8"}:
            self.reference({"kind": "u8"})
            return self.add("bytes", dict(kind="buffer", element="u8", length_unit="bytes", encoding="none", mutable=False, max_bytes=1024))
        if kind not in ("record", "enum", "object", "callback"):
            raise UnsupportedDeclaration("unsupported C06 declaration type: " + kind)
        name = value["name"]
        declaration = self.named.get(name)
        if declaration is None or declaration["kind"] != kind:
            raise UnsupportedDeclaration("missing or inconsistent declaration: " + name)
        if name in self.definitions:
            return name
        # Reserve recursive references; the shared rule engine rejects inline cycles.
        self.definitions[name] = {"kind": "unit"}
        if kind == "object":
            definition = dict(kind="resource", representation="backend-handle", owner=declaration.get("owner", self.owner))
        elif kind == "record":
            definition = dict(kind="record", layout="logical", fields=[dict(name=f["name"], type_id=self.reference(f["type"])) for f in declaration["fields"]])
        elif kind == "enum":
            self.reference({"kind": "i32"})
            cases = []
            for index, variant in enumerate(declaration["variants"]):
                payload = None
                if variant["fields"]:
                    payload = name + "." + variant["name"] + ".payload"
                    self.definitions[payload] = dict(kind="record", layout="logical", fields=[dict(name=f["name"], type_id=self.reference(f["type"])) for f in variant["fields"]])
                # Logical discriminants are distinct from backend wire encoding.
                cases.append(dict(name=variant["name"], tag=dict(kind="integer", type="i32", decimal=str(index)), payload=payload))
            definition = dict(kind="variant", tag_type="i32", cases=cases, unknown="reject")
        else:
            methods = [f for f in self.descriptor["functions"] if f["kind"] == "callback_method" and f["owner"] == name]
            if len(methods) != 1 or methods[0]["async"]:
                raise UnsupportedDeclaration("production callback requires one synchronous method: " + name)
            method = methods[0]
            definition = dict(kind="callback", parameters=[dict(name=p["name"], type_id=self.reference(p["type"])) for p in method["parameters"]], result=self.reference(method["result"]), calling_convention=self.descriptor.get("calling_convention", "c"))
            if method.get("throws"):
                definition["throws"] = self.reference(method["throws"])
        self.definitions[name] = definition
        return name


def lower(descriptor, request, config):
    """Keep backend error/handle mechanics explicit in the source-bound descriptor."""
    if descriptor.get("schema_version") != "1.0.0":
        raise UnsupportedDeclaration("unsupported compiler descriptor version")
    expected_parser = "uniffi-0.29.4" if config["variant"] == "kotlin-native-rust" else "uniffi-0.32.2"
    if config["variant"] != "swift-kotlin" and descriptor.get("parser") != expected_parser:
        raise UnsupportedDeclaration("declaration parser version does not match the selected backend")
    for item in descriptor["types"]:
        if config["variant"] != "swift-kotlin" and item["kind"] == "object" and item.get("implementation") != "struct":
            raise UnsupportedDeclaration("C06 supports only ordinary UniFFI struct objects")
        fields = item.get("fields", []) + [field for variant in item.get("variants", []) for field in variant["fields"]]
        if config["variant"] != "swift-kotlin" and any(field.get("has_default") is not False for field in fields):
            raise UnsupportedDeclaration("C06 has no implicit/default value semantics")
    for function in descriptor["functions"]:
        if config["variant"] != "swift-kotlin" and any(parameter.get("has_default") is not False for parameter in function["parameters"]):
            raise UnsupportedDeclaration("C06 has no implicit/default argument semantics")
        if any(parameter.get("by_ref", False) for parameter in function["parameters"]):
            raise UnsupportedDeclaration("C06 does not support borrowed UDL arguments")
    owner = "madb.c06." + config["variant"]
    types = Types(descriptor, owner)
    for item in descriptor["types"]:
        types.reference({"kind": item["kind"], "name": item["name"]})
    symbols = []
    for function in descriptor["functions"]:
        if function["kind"] == "callback_method":
            continue
        if function["kind"] not in ("function", "method", "constructor"):
            raise UnsupportedDeclaration("unsupported callable kind")
        qualified = ".".join(filter(None, [descriptor["namespace"], function["owner"], function["name"]]))
        key = dict(reader="org.madb.managed-declaration@1", package=descriptor["crate"], module=descriptor["namespace"], kind=function["kind"], qualified_name=qualified, overload=function.get("source_overload", "none"))
        sid = source_id(key)
        # Stable IDs are the authored C06 surface naming convention; source IDs
        # still bind the actual reader identity and parsed declaration.
        public_id = "madb.c06." + qualified + ".v1"
        parameters = []
        if function["kind"] == "method":
            parameters.append(dict(name="self", type_id=types.reference(dict(kind="object", name=function["owner"]))))
        parameters.extend(dict(name=p["name"], type_id=types.reference(p["type"])) for p in function["parameters"])
        shape = dict(kind="async" if function["async"] else "sync", calling_convention=descriptor.get("calling_convention", "c"), link_name=function["ffi_name"], parameters=parameters, result=types.reference(function["result"]), constants=[])
        if function.get("throws"):
            shape["throws"] = types.reference(function["throws"])
        location = function.get("location") or dict(path=config["source"], line=1, column=1)
        symbols.append(dict(source_key=key, source_id=sid, public_id=public_id, public_id_origin="explicit", location=location, precision=config.get("precision", "schema-derived"), shape=shape, shape_digest=ZERO))
    if not symbols:
        raise UnsupportedDeclaration("empty public interface")
    return dict(schema="madb.contract-ir/1", sdk_namespace="madb", boundary_id=config["variant"], selector=request["selector"], source_digest=request.get("input_digest", ZERO), structure_digest=ZERO, policy_digest=ZERO,
                required_features=sorted(config["features"]), types=[dict(id=k, definition=v) for k, v in sorted(types.definitions.items())], symbols=sorted(symbols, key=lambda x: x["public_id"]), policies=[], observations=[])


def semantic_surface(seed):
    """Exclude physical linkage only; preserve all logical signature/type details."""
    value = json.loads(json.dumps({"types": seed["types"], "symbols": [
        {"qualified_name": symbol["source_key"]["qualified_name"],
         "kind": symbol["source_key"]["kind"],
         "shape": {key: value for key, value in symbol["shape"].items() if key != "link_name"}}
        for symbol in seed["symbols"]]}))
    for item in value["types"]:
        definition = item["definition"]
        if definition.get("owner") == "madb.c06." + seed["boundary_id"]:
            definition["owner"] = "$profile"
    return value


def policies(seed):
    """Apply authored semantics only to the exact reviewed C06 logical surface."""
    family = "skie" if seed["boundary_id"] == "swift-kotlin" else "uniffi"
    manifest = json.loads(Path(__file__).with_name("semantic-surface.json").read_text())
    approved = manifest["profiles"][family]
    if semantic_surface(seed) != approved["surface"]:
        raise UnsupportedDeclaration("declaration changed outside the reviewed C06 semantic surface; update its protocol and acceptance before assigning policies")
    result = []
    definitions = {t["id"]: t["definition"] for t in seed["types"]}
    for symbol in seed["symbols"]:
        shape = symbol["shape"]
        call = approved["calls"][symbol["source_key"]["qualified_name"]]
        entries = [(dict(kind="call"), call)]
        slots = [(dict(kind="parameter", name=p["name"]), p["type_id"]) for p in shape["parameters"]]
        slots.append((dict(kind="return"), shape["result"]))
        for slot, type_id in slots:
            definition = definitions[type_id]
            while definition["kind"] == "nullable":
                definition = definitions[definition["inner"]]
            kind = definition["kind"]
            if kind == "resource":
                values = dict(ownership="shared", close="request-await-release", release_thread="runtime-environment")
            elif kind == "callback":
                values = dict(ownership="shared", synchronous_callback=True, reentrant=True, threading="dedicated-thread", context_owner="source-lease", stop="delivery-gate-with-source-lease", keep_alive="source-lease")
            else:
                values = dict(ownership="copy")
            entries.append((slot, values))
        for slot, values in entries:
            identity = symbol["public_id"] + "." + slot["kind"] + ("." + slot["name"] if "name" in slot else "")
            result.append(dict(selector=dict(kind="public_id", id=symbol["public_id"]), slot=slot, families=[], values=values, origin=dict(authority="project-policy", id=identity, location=dict(path="project.json", line=1, column=1))))
    return result
