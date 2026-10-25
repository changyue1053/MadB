"""Fixed C07 diagnostic protocols read from their frozen layout authorities."""

from pathlib import Path
import json

C_ROOT = "sdk/arkts/test-support/contracts/"
KOTLIN_ROOT = "sdk/kotlin/arkts-owned/contracts/"
# Keys are protocol identities, not filenames. Selectors are intentionally finite:
# prose, source references and paths belong to Source/build, not the semantic ABI.
C_LAYOUTS = {
    "native_frame": ("sdk/arkts/facade/frame-layout.json", ()),
    "test_frame": (
        C_ROOT + "test-frame-layout.json",
        (
            "native_prefix_length",
            "test_length",
            "total_length",
            "fields",
            "operation_fields",
            "hook_entry_offset",
            "hook_entry_length",
        ),
    ),
    "observer": (
        C_ROOT + "observer-layout.json",
        (
            "format",
            "length",
            "unchanged_prefix",
            "local_offset",
            "local",
            "process_offset",
            "process",
            "range",
        ),
    ),
    "frame_units": (C_ROOT + "diagnostic-units-v1.json", ("version", "fields")),
    "facade": (C_ROOT + "facade-frame-layout.json", ("format", "length", "fields")),
    "delivery": (
        C_ROOT + "ports-layout.json",
        ("delivery.format", "delivery.length", "delivery.fields", "delivery.units"),
    ),
    "bootstrap": (
        C_ROOT + "bootstrap-totals-layout.json",
        ("format", "length", "fields", "units"),
    ),
    "memory": (
        C_ROOT + "memory-layout.json",
        ("format", "length", "fields", "units", "errors"),
    ),
    "subscription": (
        C_ROOT + "subscription-state-layout.json",
        ("format", "length", "fields", "units", "errors"),
    ),
}
KOTLIN_LAYOUTS = {
    "admission": (
        KOTLIN_ROOT + "admission-frame.json",
        ("length", "wire", "numeric_domain", "fields"),
    ),
    "controls": (
        KOTLIN_ROOT + "controls-frame.json",
        ("format", "length", "wire", "frame.fields"),
    ),
    "memory": (
        KOTLIN_ROOT + "memory-frame.json",
        ("format", "length", "wire", "fields", "units"),
    ),
    "napi_observer": (
        KOTLIN_ROOT + "napi-observer-frame.json",
        (
            "format",
            "wire",
            "fixed_fields",
            "unknown",
            "capabilities",
            "categories",
            "roles",
        ),
    ),
    "settlement": (KOTLIN_ROOT + "settlement-frame.json", ("length", "wire", "fields")),
    "subscription": (
        KOTLIN_ROOT + "subscription-frame.json",
        ("format", "length", "wire", "fields"),
    ),
}
FIELD_KEYS = {
    "index",
    "name",
    "unit",
    "domain",
    "range",
    "group",
    "category",
    "return",
    "arguments",
    "function_type",
}


def fields(value):
    """Preserve actual index/order/name/unit domains, excluding field commentary."""
    if isinstance(value, list):
        return [fields(v) for v in value]
    if isinstance(value, dict):
        if "name" in value:
            return {k: v for k, v in value.items() if k in FIELD_KEYS}
        return {k: fields(v) for k, v in value.items()}
    return value


def protocol(document, selectors):
    if not selectors:
        return {key: value for key, value in document.items() if key.isdecimal()}
    result = {}
    for selector in selectors:
        value = document
        for part in selector.split("."):
            value = value[part]
        if selector in ("fields", "frame.fields", "fixed_fields"):
            value = fields(value)
        elif selector == "roles":
            value = [
                {
                    "role": role["role"],
                    "count": role["count"],
                    "per_api_fields": fields(role["per_api_fields"]),
                }
                for role in value
            ]
        result[selector] = value
    return result


def read(root, config, refs, support):
    """Read every declared diagnostic authority from the original checked inputs."""
    if config["variant"] != "test":
        return None
    managed = config["provider"] == "kotlin"
    selected = KOTLIN_LAYOUTS if managed else C_LAYOUTS
    protocols, sources = {}, {}
    for identity, (name, selectors) in selected.items():
        if name not in refs:
            raise ValueError("diagnostic layout omitted from source inputs: " + name)
        document = support.read_json(support.input_path(root, name))
        protocols[identity] = protocol(document, selectors)
        sources[identity] = refs[name]
    if managed:
        name = "adapters/toolchains/arkts/kotlin_frame.py"
        if name not in refs:
            raise ValueError("Kotlin runtime frame generator omitted from checked inputs")
        frame = support.load_module(Path(root) / name)
        document = json.loads(
            frame.render()["generated/contract/kotlin-frame-layout.json"]
        )
        protocols["runtime"] = protocol(
            document, ("format", "length", "fields", "zero_at_closed", "equal_at_closed")
        )
        sources["runtime"] = refs[name]
    return {"protocols": protocols, "sources": sources}


def verify_bounds(diagnostic, policy):
    """Array size comes from the actual protocol, not a second authored count."""
    for name, slot in policy.get("value_slots", {}).items():
        identity = slot.get("diagnostic_protocol")
        if identity is None:
            continue
        value = diagnostic["protocols"][identity]
        if "length_elements" in slot:
            length = value["length"]
            if (
                slot["length_elements"] != length
                or policy["test_types"][slot["type_id"]]["max_bytes"] != length * 8
            ):
                raise ValueError(
                    "test array bound differs from frozen diagnostic protocol: " + name
                )
