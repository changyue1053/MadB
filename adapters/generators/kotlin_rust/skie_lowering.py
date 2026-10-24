"""A deliberately closed C06 projection of compiler-produced SKIE declarations.

The compiler owns types and dispatch identities. This adapter declares which
SampleEngine APIs are promised; it does not infer thread or ownership semantics.
Raw framework support APIs remain in the hashed compiler descriptor.
"""
from __future__ import annotations

import hashlib


class UnsupportedSwiftDeclaration(ValueError):
    pass


SCALARS = {
    "s:SS": "string", "s:Sb": "bool", "s:s5Int32V": "i32",
    "s:s5Int64V": "i64", "s:Si": "i64", "s:Su": "u64",
    "c:objc(cs)MBSLong": "i64", "c:objc(cs)MBSKotlinByteArray": "bytes",
}
RESOURCES = {"Engine", "Operation", "Subscription"}
GLOBALS = {"profileIdentity", "interfaceIdentity", "nativeAbiVersion", "bootstrapVersion", "openEngine", "failureEvent"}
METHODS = {
    "Engine": {"phase", "publish", "requestClose", "roundtrip", "startOperation", "stats", "subscribeEvents", "subscribeState", "awaitClosed"},
    "Operation": {"requestCancel", "run"},
    "Subscription": {"requestStop", "awaitStopped", "events"},
    "Host": {"transform"},
}


class Reader:
    def __init__(self, api):
        self.symbols = api["swift_symbols"]
        self.named = {}
        for symbol in self.symbols:
            if symbol["kind"]["identifier"] in ("swift.class", "swift.enum", "swift.protocol"):
                self.named[".".join(symbol["pathComponents"])] = symbol["identifier"]["precise"]
        self.definitions = {}
        self.objc = {d["name"]: d for d in api["objc_declarations"]}

    def validate_syntax(self, symbol):
        fragments = symbol.get("declarationFragments", [])
        allowed_keywords = {"func", "async", "throws", "static", "class", "var", "get", "init", "convenience", "required", "override", "protocol", "enum", "final"}
        keywords = {f["spelling"] for f in fragments if f["kind"] == "keyword"}
        if keywords - allowed_keywords:
            raise UnsupportedSwiftDeclaration("Swift ownership/isolation modifier needs an explicit adapter")
        attributes = [f["spelling"] for f in fragments if f["kind"] == "attribute"]
        if attributes and not (symbol["pathComponents"] == ["Phase"] and attributes == ["@frozen"]):
            raise UnsupportedSwiftDeclaration("Swift declaration attribute needs an explicit adapter")
        if any("=" in f["spelling"] for f in fragments):
            raise UnsupportedSwiftDeclaration("Swift default argument needs an explicit adapter")

    def select(self, path, kind):
        found = [s for s in self.symbols if s["pathComponents"] == path and s["kind"]["identifier"] == kind]
        if len(found) != 1:
            raise UnsupportedSwiftDeclaration("missing or ambiguous compiler declaration: " + ".".join(path))
        self.validate_syntax(found[0])
        return found[0]

    def type(self, fragments, parameter=False):
        if parameter:
            # A parameter's declared local name precedes its colon. It is not a type.
            colon = next((i for i, f in enumerate(fragments) if f["kind"] == "text" and ":" in f["spelling"]), None)
            if colon is None:
                raise UnsupportedSwiftDeclaration("parameter has no compiler type fragments")
            prefix = fragments[colon]["spelling"].split(":", 1)[1]
            fragments = [{"kind": "text", "spelling": prefix}] + fragments[colon + 1:]
        spelling = "".join(f["spelling"] for f in fragments).strip()
        if spelling in ("", "Void", "()"):
            return None
        if spelling.endswith("?"):
            copied = [dict(f) for f in fragments]
            copied[-1]["spelling"] = copied[-1]["spelling"].rsplit("?", 1)[0]
            return {"kind": "optional", "inner": self.type(copied)}
        if spelling.startswith("any "):
            spelling = spelling[4:]
        if any(f["kind"] == "genericParameter" for f in fragments):
            raise UnsupportedSwiftDeclaration("open generic is outside the C06 public contract")
        identifiers = [f for f in fragments if f["kind"] == "typeIdentifier"]
        if spelling == "SkieSwiftFlow<BoundaryEvent>" and len(identifiers) == 2 and identifiers[0].get("preciseIdentifier") == "s:8MadBSkie13SkieSwiftFlowC" and identifiers[1].get("preciseIdentifier") == self.named["BoundaryEvent"]:
            self.definitions["Flow.BoundaryEvent"] = {"kind": "object", "name": "Flow.BoundaryEvent", "owner": "madb.c06.swift-kotlin.Subscription"}
            return {"kind": "object", "name": "Flow.BoundaryEvent"}
        if len(identifiers) != 1:
            raise UnsupportedSwiftDeclaration("unsupported composite Swift type: " + spelling)
        if spelling != identifiers[0]["spelling"]:
            raise UnsupportedSwiftDeclaration("unsupported Swift type syntax: " + spelling)
        precise = identifiers[0].get("preciseIdentifier")
        if precise in SCALARS:
            return {"kind": SCALARS[precise]}
        name = next((name for name, identifier in self.named.items() if identifier == precise), None)
        if name not in {"Value", "Stats", "Phase", "BoundaryEvent", "Host"} | RESOURCES:
            raise UnsupportedSwiftDeclaration("unsupported Swift reference: " + spelling)
        kind = "record" if name in {"Value", "Stats"} else "enum" if name in {"Phase", "BoundaryEvent"} else "callback" if name == "Host" else "object"
        return {"kind": kind, "name": name}

    def fields(self, symbol):
        return [{"name": p["name"], "type": self.type(p["declarationFragments"], True)} for p in symbol.get("functionSignature", {}).get("parameters", [])]

    def constructor(self, path):
        matches = [s for s in self.symbols if s["pathComponents"][:-1] == path and s["kind"]["identifier"] == "swift.init"]
        if len(matches) != 1:
            raise UnsupportedSwiftDeclaration("value type needs one unambiguous compiler constructor: " + ".".join(path))
        self.validate_syntax(matches[0])
        return matches[0]

    def value_types(self):
        for name in ("Value", "Stats"):
            self.select([name], "swift.class")
            fields = self.fields(self.constructor([name]))
            for field in fields:
                property_symbol = self.select([name, field["name"]], "swift.property")
                fragments = property_symbol["declarationFragments"]
                # Properties have a getter suffix; the constructor and getter must agree.
                text = "".join(f["spelling"] for f in fragments)
                if "{ get }" not in text or "set" in [f["spelling"] for f in fragments if f["kind"] == "keyword"]:
                    raise UnsupportedSwiftDeclaration("mutable DTO field is not a snapshot: " + name + "." + field["name"])
                if self.type(property_symbol["names"]["subHeading"], True) != field["type"]:
                    raise UnsupportedSwiftDeclaration("DTO constructor and getter types disagree")
            self.definitions[name] = {"kind": "record", "name": name, "fields": fields}
        self.select(["Phase"], "swift.enum")
        cases = [s["pathComponents"][-1] for s in self.symbols if s["pathComponents"][:-1] == ["Phase"] and s["kind"]["identifier"] == "swift.enum.case"]
        if not cases:
            raise UnsupportedSwiftDeclaration("Phase has no actual Swift enum cases")
        self.definitions["Phase"] = {"kind": "enum", "name": "Phase", "variants": [{"name": n, "fields": []} for n in sorted(cases)]}
        self.select(["BoundaryEvent"], "swift.class")
        enhanced = [s for s in self.symbols if s["pathComponents"] == ["onEnum(of:)"] and any(c.get("rhsPrecise") == self.named["BoundaryEvent"] for c in s.get("swiftGenerics", {}).get("constraints", []))]
        if not enhanced:
            raise UnsupportedSwiftDeclaration("sealed BoundaryEvent enhancement is missing")
        cases = [s["pathComponents"] for s in self.symbols if s["kind"]["identifier"] == "swift.class" and len(s["pathComponents"]) == 2 and s["pathComponents"][0] == "BoundaryEvent"]
        if not cases:
            raise UnsupportedSwiftDeclaration("sealed payload constructors are missing")
        for path in cases:
            identifier = self.named[".".join(path)]
            objc_name = identifier.removeprefix("c:objc(cs)")
            if self.objc.get(objc_name, {}).get("super") != self.named["BoundaryEvent"].removeprefix("c:objc(cs)"):
                raise UnsupportedSwiftDeclaration("nested event payload is not a compiler-confirmed sealed subclass")
        self.definitions["BoundaryEvent"] = {"kind": "enum", "name": "BoundaryEvent", "variants": [{"name": path[-1], "fields": self.fields(self.constructor(path))} for path in sorted(cases)]}
        for name in RESOURCES:
            self.select([name], "swift.class")
            self.definitions[name] = {"kind": "object", "name": name}
        self.select(["Host"], "swift.protocol")
        self.definitions["Host"] = {"kind": "callback", "name": "Host"}

    def function(self, symbol, owner):
        self.validate_syntax(symbol)
        signature = symbol.get("functionSignature", {})
        keywords = {f["spelling"] for f in symbol["declarationFragments"] if f["kind"] == "keyword"}
        if keywords & {"inout", "borrowing", "consuming", "isolated", "sending", "rethrows"}:
            raise UnsupportedSwiftDeclaration("Swift ownership/isolation modifier needs an explicit adapter")
        if symbol.get("swiftGenerics"):
            raise UnsupportedSwiftDeclaration("generic callable needs an explicit adapter")
        precise = symbol["identifier"]["precise"]
        result = self.type(signature.get("returns", []))
        name = symbol["pathComponents"][-1].split("(", 1)[0]
        if symbol["kind"]["identifier"] == "swift.property":
            fragments = symbol["names"]["subHeading"]
            result = self.type(fragments, True)
        throws = None
        if "throws" in keywords:
            # Swift's erased Error is backend-owned. No inferred Kotlin error enum or DTO.
            self.definitions["Swift.Error"] = {"kind": "object", "name": "Swift.Error", "owner": "swift.error"}
            throws = {"kind": "object", "name": "Swift.Error"}
        return dict(kind="callback_method" if owner == "Host" else "method" if owner else "function",
                    owner=owner, name=name, parameters=self.fields(symbol), result=result,
                    throws=throws, async_="async" in keywords, source_overload=precise,
                    ffi_name="swift.dispatch." + hashlib.sha256(precise.encode()).hexdigest())


def descriptor(api, source):
    if api.get("schema_version") != 1 or api.get("source_kind") != "compiled-kotlin-native-objc-skie" or api.get("module") != "MadBSkie" or api.get("target") != "arm64-apple-macosx14.0":
        raise UnsupportedSwiftDeclaration("unsupported compiled Swift profile")
    reader = Reader(api)
    reader.value_types()
    functions = []
    seen = set()
    for symbol in reader.symbols:
        path, kind = symbol["pathComponents"], symbol["kind"]["identifier"]
        name = path[-1].split("(", 1)[0]
        owner = path[0] if len(path) == 2 and path[0] in METHODS else None
        selected = owner is not None and kind in ("swift.method", "swift.property") or len(path) == 1 and name in GLOBALS and kind == "swift.func"
        if not selected:
            continue
        if owner and name not in METHODS[owner]:
            raise UnsupportedSwiftDeclaration("new public method requires a C06 semantic policy: " + ".".join(path))
        identity = (owner, name)
        if identity in seen:
            raise UnsupportedSwiftDeclaration("overloaded public API requires an explicit stable identity")
        seen.add(identity)
        item = reader.function(symbol, owner)
        item["async"] = item.pop("async_")
        item["location"] = {"path": source, "line": 1, "column": 1}
        functions.append(item)
    required = {(owner, method) for owner, methods in METHODS.items() for method in methods} | {(None, name) for name in GLOBALS}
    if seen != required:
        raise UnsupportedSwiftDeclaration("public C06 surface is incomplete: " + repr(sorted(required - seen, key=str)))
    return dict(schema_version="1.0.0", crate="MadBSkie", namespace="madb_skie", calling_convention="system",
                source_kind=api["source_kind"], types=sorted(reader.definitions.values(), key=lambda t: t["name"]),
                functions=sorted(functions, key=lambda f: (f["owner"] or "", f["name"])),
                evidence=dict(header_digest=api["header_digest"], swift_interfaces=api["swift_interfaces"]),
                scope="C06 operational surface; DTO conveniences, raw ObjC aliases, SKIE helpers and conformance-only GC hook remain captured in the compiler API snapshot")
