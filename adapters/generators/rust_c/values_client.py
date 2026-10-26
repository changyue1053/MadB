"""Generate ordinary C11 value calls from the checked natural-Rust plan.

The native shim owns input validation/conversion.  This header owns identity
ordering and rejects malformed native output before handing a value to C.
"""
from __future__ import annotations

import hashlib
import re

from support import NativeError


def _identifier(value):
    if not isinstance(value, str) or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value) is None:
        raise NativeError("NATIVE001", "value client received an invalid checked identifier")
    return value


def _identity_bytes(identity):
    if (not isinstance(identity, dict) or identity.get("schema") != "madb.native-identity/1"
            or type(identity.get("bootstrap_version")) is not int or identity["bootstrap_version"] != 1):
        raise NativeError("NATIVE001", "value client requires native identity v1")
    expected = bytearray()
    for key in ("interface_digest", "profile_digest"):
        value = identity.get(key)
        if not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None:
            raise NativeError("NATIVE001", "value client requires a SHA-256 identity: " + key)
        expected.extend(bytes.fromhex(value[7:]))
    return ", ".join(str(value) for value in expected)


def _checked_shapes(plan):
    """Reject malformed internal plans; the source reader remains authority."""
    try:
        prefix = _identifier(plan["namespace"])
        records = {}
        for record in plan["records"]:
            name = _identifier(record["name"])
            if name in records or not 1 <= len(record["fields"]) <= 16:
                raise ValueError("record shape")
            fields = set()
            for field in record["fields"]:
                field_name = _identifier(field["name"])
                if field_name in fields or field["kind"] not in ("i32", "u32", "String"):
                    raise ValueError("field shape")
                fields.add(field_name)
            records[name] = record
        errors = {}
        for error in plan["errors"]:
            name = _identifier(error["name"])
            if name in errors or not error["variants"]:
                raise ValueError("error shape")
            tags, names = set(), set()
            for variant in error["variants"]:
                tag = variant["value"]
                variant_name = _identifier(variant["name"])
                if (type(tag) is not int or not 0 < tag <= 2147483647
                        or tag in tags or variant_name in names):
                    raise ValueError("error tag")
                tags.add(tag)
                names.add(variant_name)
            errors[name] = error
        functions = set()
        for function in plan["functions"]:
            name = _identifier(function["name"])
            parameters = function["parameters"]
            if (name in functions or function["record"] not in records or len(parameters) not in (1, 2)
                    or parameters[0]["kind"] != function["record"]
                    or function["error"] is not None and function["error"] not in errors):
                raise ValueError("function shape")
            functions.add(name)
            for parameter in parameters:
                _identifier(parameter["name"])
            if len(parameters) == 2 and (parameters[1]["kind"] != "i32"
                    or parameters[1]["name"] in ("input", "out_value", "out_error")):
                raise ValueError("second parameter shape")
    except (KeyError, TypeError, ValueError) as error:
        raise NativeError("NATIVE001", "value client received an invalid checked plan") from error
    return prefix, records, errors


def _utf8_helper(prefix):
    return f"""/* Validate bytes only; do not transcode or interpret NUL as a terminator. */
static inline int {prefix}_value_utf8_valid(const MadbValueUtf8_64 *text) {{
    const uint32_t length = text->byte_count;
    uint32_t index = 0;
    if (length > 64u) return 0;
    while (index < length) {{
        const uint32_t first = text->bytes[index++];
        uint32_t remaining, code, minimum;
        if (first < 0x80u) continue;
        if (first >= 0xc2u && first <= 0xdfu) {{
            remaining = 1u; code = first & 0x1fu; minimum = 0x80u;
        }} else if (first >= 0xe0u && first <= 0xefu) {{
            remaining = 2u; code = first & 0x0fu; minimum = 0x800u;
        }} else if (first >= 0xf0u && first <= 0xf4u) {{
            remaining = 3u; code = first & 0x07u; minimum = 0x10000u;
        }} else {{
            return 0;
        }}
        if (length - index < remaining) return 0;
        while (remaining-- != 0u) {{
            const uint32_t next = text->bytes[index++];
            if ((next & 0xc0u) != 0x80u) return 0;
            code = (code << 6u) | (next & 0x3fu);
        }}
        if (code < minimum || code > 0x10ffffu || (code >= 0xd800u && code <= 0xdfffu)) return 0;
    }}
    return 1;
}}
"""


def _record_helpers(prefix, record):
    name = record["name"]
    lines = [f"static inline int {prefix}_value_{name}_is_zero(const {prefix}_{name} *value) {{"]
    for field in record["fields"]:
        member = "value->" + field["name"]
        if field["kind"] == "String":
            lines += [f"    if ({member}.byte_count != 0u) return 0;",
                      f"    for (uint32_t index = 0; index < 64u; ++index) if ({member}.bytes[index] != 0u) return 0;"]
        else:
            lines.append(f"    if ({member} != 0) return 0;")
    lines += ["    return 1;", "}", "", f"static inline int {prefix}_value_{name}_valid_output(const {prefix}_{name} *value) {{"]
    if not any(field["kind"] == "String" for field in record["fields"]):
        lines.append("    (void)value;")
    for field in record["fields"]:
        if field["kind"] == "String":
            member = "value->" + field["name"]
            lines += [f"    if (!{prefix}_value_utf8_valid(&{member})) return 0;",
                      f"    for (uint32_t index = {member}.byte_count; index < 64u; ++index) if ({member}.bytes[index] != 0u) return 0;"]
    lines += ["    return 1;", "}"]
    return "\n".join(lines) + "\n"


def _call(prefix, function, errors, expected):
    record = function["record"]
    parameters = function["parameters"]
    extra = parameters[1]["name"] if len(parameters) == 2 else None
    # Parameter names are source-authoritative. Pick internal names around them,
    # rather than silently renaming an author parameter or widening restrictions.
    taken = {"input", extra}
    local = {}
    for role in ("expected", "actual", "result", "status", "index", "output", "error"):
        name = "madb_value_" + role
        while name in taken:
            name += "_"
        taken.add(name)
        local[role] = name
    e, a, r, s, i, out, err = (local[role] for role in ("expected", "actual", "result", "status", "index", "output", "error"))
    signature = f"{prefix}_{record} input" + (f", int32_t {extra}" if extra else "")
    arguments = "&input" + (f", {extra}" if extra else "") + f", &{out}, &{err}"
    lines = [f"static inline {prefix}_{record}Result {prefix}_{function['name']}({signature}) {{",
             f"    static const uint8_t {e}[64] = {{{expected}}};",
             f"    uint8_t {a}[64] = {{0}};",
             f"    {prefix}_{record}Result {r} = {{0}};",
             f"    {r}.kind = MADB_VALUE_CALL_BOOTSTRAP_FAILED;",
             f"    {r}.status = MADB_VALUE_STATUS_INTERNAL_FAILURE;",
             f"    MadbValueStatus {s} = {prefix}_bootstrap_v1(1u, {a}, 64u);",
             f"    if ({s} != MADB_VALUE_STATUS_OK) {{",
             f"        if ({s} == MADB_VALUE_STATUS_INVALID_ARGUMENT || {s} == MADB_VALUE_STATUS_INTERNAL_FAILURE",
             f"                || {s} == MADB_VALUE_STATUS_RESOURCE_EXHAUSTED || {s} == MADB_VALUE_STATUS_UNSUPPORTED) {r}.status = {s};",
             f"        return {r};", "    }",
             f"    for (uint32_t {i} = 0; {i} < 32u; ++{i}) {{",
             f"        if ({a}[{i}] != {e}[{i}]) {{",
             f"            {r}.kind = MADB_VALUE_CALL_INTERFACE_MISMATCH;",
             f"            {r}.status = MADB_VALUE_STATUS_UNSUPPORTED;",
             f"            return {r};", "        }", "    }",
             f"    for (uint32_t {i} = 32u; {i} < 64u; ++{i}) {{",
             f"        if ({a}[{i}] != {e}[{i}]) {{",
             f"            {r}.kind = MADB_VALUE_CALL_PROFILE_MISMATCH;",
             f"            {r}.status = MADB_VALUE_STATUS_UNSUPPORTED;",
             f"            return {r};", "        }", "    }",
             f"    {prefix}_{record} {out} = {{0}};",
             f"    MadbValueError {err} = {{0}};",
             f"    {s} = {prefix}_raw_{function['name']}({arguments});",
             f"    {r}.kind = MADB_VALUE_CALL_OPERATION_FAILED;",
             f"    {r}.status = MADB_VALUE_STATUS_INTERNAL_FAILURE;",
             f"    if ({s} == MADB_VALUE_STATUS_OK) {{",
             f"        if ({err}.domain != 0u || {err}.code != 0 || !{prefix}_value_{record}_valid_output(&{out})) return {r};",
             f"        {r}.kind = MADB_VALUE_CALL_OK;",
             f"        {r}.status = MADB_VALUE_STATUS_OK;",
             f"        {r}.value = {out};",
             f"        return {r};", "    }",
             f"    if (!{prefix}_value_{record}_is_zero(&{out})) return {r};",
             f"    if ({s} == MADB_VALUE_STATUS_INVALID_ARGUMENT || {s} == MADB_VALUE_STATUS_INTERNAL_FAILURE",
             f"            || {s} == MADB_VALUE_STATUS_RESOURCE_EXHAUSTED) {{",
             f"        if ({err}.domain != 0u || {err}.code != 0) return {r};",
             f"        {r}.status = {s};",
             f"        return {r};", "    }"]
    if function["error"] is not None:
        error = errors[function["error"]]
        tags = " || ".join(f"{err}.code == {prefix}_{error['name']}_{variant['name']}" for variant in error["variants"])
        lines += [f"    if ({s} == MADB_VALUE_STATUS_DOMAIN_ERROR && {err}.domain == 1u && ({tags})) {{",
                  f"        {r}.status = {s};", f"        {r}.error = {err};", f"        return {r};", "    }"]
    lines += [f"    return {r};", "}"]
    return "\n".join(lines) + "\n"


def render_values_client(plan, identity):
    """Return P.h; inputs are a checked private plan and this SDK's identity."""
    prefix, records, errors = _checked_shapes(plan)
    expected = _identity_bytes(identity)
    guard = "MADB_VALUE_CLIENT_" + hashlib.sha256(prefix.encode()).hexdigest()[:16].upper() + "_H"
    parts = ["/* Generated from the checked author declarations and native identity. Do not edit. */",
             f"#ifndef {guard}", f"#define {guard}", '#include "madb_value_abi_v1.h"',
             f'#include "{prefix}_native.h"', "",
             "/* Failed results contain a zero value. No value needs free/close. */"]
    for name in records:
        parts += [f"typedef struct {prefix}_{name}Result {{", "    MadbValueCallKind kind;",
                  "    MadbValueStatus status;", "    MadbValueError error;", f"    {prefix}_{name} value;",
                  f"}} {prefix}_{name}Result;", ""]
    parts.append(_utf8_helper(prefix))
    parts.extend(_record_helpers(prefix, record) for record in records.values())
    parts.extend(_call(prefix, function, errors, expected) for function in plan["functions"])
    parts += ["#endif", ""]
    return "\n".join(parts)
