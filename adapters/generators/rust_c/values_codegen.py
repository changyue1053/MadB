"""Render the finite value shim from an already checked natural-Rust plan.

The reader and real rustc witness establish the author structure. This module
only emits the selected ABI mechanics; no function-name or business whitelist
participates in the conversion. The actual physical headers come from cbindgen.
"""
from __future__ import annotations

import hashlib
import json
import re

from support import NativeError
from values_runtime import BASE_NAMES, BASE_TYPES, CALL_KINDS, RUNTIME, STATUSES


def _identifier(value):
    if not isinstance(value, str) or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value) is None:
        raise NativeError("NATIVE004", "value generator requires a checked ASCII identifier")
    return value


def _identity_bytes(identity):
    if (not isinstance(identity, dict) or identity.get("schema") != "madb.native-identity/1"
            or type(identity.get("bootstrap_version")) is not int or identity["bootstrap_version"] != 1):
        raise NativeError("NATIVE001", "value generator requires native identity v1")
    result = b""
    for key in ("interface_digest", "profile_digest"):
        value = identity.get(key)
        if not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None:
            raise NativeError("NATIVE001", "value generator requires a SHA-256 identity: " + key)
        result += bytes.fromhex(value[7:])
    return result


def _plan_names(plan):
    """Check only the bounded rendering shape; semantic authority is the reader."""
    if not isinstance(plan, dict):
        raise NativeError("NATIVE004", "value generator requires the checked reader plan")
    namespace = _identifier(plan.get("namespace"))
    records, errors, functions = plan.get("records"), plan.get("errors"), plan.get("functions")
    if not all(isinstance(items, list) for items in (records, errors, functions)):
        raise NativeError("NATIVE004", "value plan lacks record/error/function arrays")
    record_names, error_names = set(), set()
    for record in records:
        name = _identifier(record["name"])
        if name in record_names or not 1 <= len(record["fields"]) <= 16:
            raise NativeError("NATIVE004", "value record is duplicate or outside the field budget")
        record_names.add(name)
        field_names = set()
        for field in record["fields"]:
            field_name = _identifier(field["name"])
            if field_name in field_names or field["kind"] not in ("i32", "u32", "String"):
                raise NativeError("NATIVE004", "value field is duplicate or outside the finite kinds")
            field_names.add(field_name)
    for error in errors:
        name = _identifier(error["name"])
        if name in error_names:
            raise NativeError("NATIVE004", "duplicate value error type")
        error_names.add(name)
        variant_names, tags = set(), set()
        for variant in error["variants"]:
            name, value = _identifier(variant["name"]), variant["value"]
            if name in variant_names or type(value) is not int or not 1 <= value <= 2147483647 or value in tags:
                raise NativeError("NATIVE004", "value error tags must be distinct positive i32 values")
            variant_names.add(name)
            tags.add(value)
    function_names = set()
    for function in functions:
        name = _identifier(function["name"])
        parameters = function["parameters"]
        if (name in function_names or function["record"] not in record_names or len(parameters) not in (1, 2)
                or parameters[0]["kind"] != function["record"]
                or (len(parameters) == 2 and parameters[1]["kind"] != "i32")
                or (function["error"] is not None and function["error"] not in error_names)):
            raise NativeError("NATIVE004", "value function differs from its checked finite signature")
        function_names.add(name)
        for parameter in parameters:
            _identifier(parameter["name"])
    return namespace, records, errors, functions


def _rust_record(namespace, record):
    name = namespace + "_" + record["name"]
    fields = [(field["name"], "MadbValueUtf8_64" if field["kind"] == "String" else field["kind"]) for field in record["fields"]]
    lines = ["#[repr(C)]", "#[derive(Clone, Copy)]", "pub struct " + name + " {"]
    lines.extend("    pub " + field + ": " + kind + "," for field, kind in fields)
    lines += ["}", "", "impl " + name + " {", "    fn zero() -> Self {", "        Self {"]
    lines.extend("            " + field + ": " + ("MadbValueUtf8_64 { byte_count: 0, bytes: [0; 64] }" if kind == "MadbValueUtf8_64" else "0") + "," for field, kind in fields)
    lines += ["        }", "    }", "}", ""]
    return "\n".join(lines)


def _raw_function(namespace, function, record):
    physical = namespace + "_" + record["name"]
    parameters = ["input: *const " + physical]
    delta = function["parameters"][1]["name"] if len(function["parameters"]) == 2 else None
    if delta:
        parameters.append(delta + ": i32")
    parameters += ["out_value: *mut " + physical, "out_error: *mut MadbValueError"]
    lines = [
        "/// The caller owns valid, aligned, initialized, nonoverlapping input/output objects.",
        "/// Borrowed input is used only during this synchronous call; no caller pointer is stored.",
        "#[unsafe(no_mangle)]",
        "pub unsafe extern \"C\" fn " + namespace + "_raw_" + function["name"] + "(" + ", ".join(parameters) + ") -> MadbValueStatus {",
        *(["    let madb_value_argument = " + delta + ";"] if delta else []),
        "    let value_ok = madb_value_runtime::usable(out_value);",
        "    let error_ok = madb_value_runtime::usable(out_error);",
        "    // Clear each mechanically valid output independently, before considering input.",
        "    if value_ok { unsafe { out_value.write(" + physical + "::zero()); } }",
        "    if error_ok { unsafe { out_error.write(MadbValueError { domain: 0, code: 0 }); } }",
        "    if !value_ok || !error_ok || !madb_value_runtime::usable(input) { return 1; }",
        "    // The selected build uses unwind. Abort, panic during an active unwind and invalid caller memory",
        "    // remain process failures; catch_unwind is not a fault/allocator-abort handler.",
        "    let call = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<" + physical + ", madb_value_runtime::Failure> {",
        "        let source = unsafe { &*input };",
    ]
    strings = [field for field in record["fields"] if field["kind"] == "String"]
    # Validate all fields first: a later invalid text cannot be masked by an
    # earlier conversion allocation failure.
    for index, field in enumerate(strings):
        lines.append("        let text_" + str(index) + " = madb_value_runtime::text(&source." + field["name"] + ")?;")
    lines.append("        let owned = madb_value_author::" + record["name"] + " {")
    text_index = 0
    for field in record["fields"]:
        value = "source." + field["name"]
        if field["kind"] == "String":
            value = "madb_value_runtime::owned_text(text_" + str(text_index) + ")?"
            text_index += 1
        lines.append("            " + field["name"] + ": " + value + ",")
    lines.append("        };")
    call = "madb_value_author::" + function["name"] + "(owned" + (", madb_value_argument" if delta else "") + ")"
    if function["error"] is not None:
        call += ".map_err(|error| (10, MadbValueError { domain: 1, code: error as i32 }))?"
    lines.append("        let returned = " + call + ";")
    for field in strings:
        lines.append("        if returned." + field["name"] + ".len() > 64 { return Err(madb_value_runtime::failure(9)); }")
    lines.append("        Ok(" + physical + " {")
    for field in record["fields"]:
        value = "returned." + field["name"]
        if field["kind"] == "String":
            value = "madb_value_runtime::encoded_text(&" + value + ")"
        lines.append("            " + field["name"] + ": " + value + ",")
    lines += [
        "        })",
        "    }));",
        "    match call {",
        "        Ok(Ok(value)) => { unsafe { out_value.write(value); } 0 }",
        "        Ok(Err((status, error))) => { unsafe { out_error.write(error); } status }",
        "        Err(payload) => { madb_value_runtime::dispose_panic(payload); 8 }",
        "    }",
        "}", "",
    ]
    return "\n".join(lines)


def render_values_shim(plan, identity):
    """Return standalone Rust source; the original author bytes live in author.rs."""
    namespace, records, errors, functions = _plan_names(plan)
    expected = _identity_bytes(identity)
    lines = ["// Generated madb.native-values/1 ABI shim. Do not edit.",
             "#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]",
             "#[path = \"author.rs\"]", "mod madb_value_author;", "", BASE_TYPES]
    lines.extend("pub const " + name + ": u32 = " + str(value) + ";" for name, value in CALL_KINDS.items())
    lines.extend("pub const " + name + ": i32 = " + str(value) + ";" for name, value in STATUSES.items())
    for error in errors:
        for variant in error["variants"]:
            lines.append("pub const " + namespace + "_" + error["name"] + "_" + variant["name"] + ": i32 = " + str(variant["value"]) + ";")
    lines += ["", RUNTIME]
    lines.extend(_rust_record(namespace, record) for record in records)
    lines += [
        "/// out_identity is valid for 64 writable bytes when capacity is 64.",
        "#[unsafe(no_mangle)]",
        "pub unsafe extern \"C\" fn " + namespace + "_bootstrap_v1(version: u32, out_identity: *mut u8, capacity: u64) -> MadbValueStatus {",
        "    if out_identity.is_null() || capacity != 64 { return 1; }",
        "    unsafe { std::ptr::write_bytes(out_identity, 0, 64); }",
        "    if version != 1 { return 11; }",
        "    let identity: [u8; 64] = [" + ", ".join(str(value) for value in expected) + "];",
        "    unsafe { std::ptr::copy_nonoverlapping(identity.as_ptr(), out_identity, 64); }",
        "    0",
        "}", "",
    ]
    by_name = {record["name"]: record for record in records}
    lines.extend(_raw_function(namespace, function, by_name[function["record"]]) for function in functions)
    return "\n".join(lines)


def render_values_cbindgen_config(plan, *, base_only=False):
    """Select two actual cbindgen products from the same complete Rust source."""
    namespace, records, errors, functions = _plan_names(plan)
    base = BASE_NAMES + list(CALL_KINDS) + list(STATUSES)
    physical = [namespace + "_" + record["name"] for record in records]
    exports = [namespace + "_bootstrap_v1"] + [namespace + "_raw_" + function["name"] for function in functions]
    tags = [namespace + "_" + error["name"] + "_" + variant["name"] for error in errors for variant in error["variants"]]
    authors = [record["name"] for record in records] + [error["name"] for error in errors] + [function["name"] for function in functions]
    guard = "MADB_VALUE_ABI_V1_H" if base_only else "MADB_VALUE_NATIVE_" + hashlib.sha256(namespace.encode()).hexdigest()[:16].upper() + "_H"
    lines = ['language = "C"', 'style = "type"', 'include_guard = ' + json.dumps(guard),
             'autogen_warning = "/* Generated by locked cbindgen from madb.native-values/1. Do not edit. */"',
             'usize_is_size_t = true']
    if not base_only:
        lines.append('includes = ["madb_value_abi_v1.h"]')
    lines += ['[export]', 'include = ' + json.dumps(base if base_only else physical + tags),
              'exclude = ' + json.dumps(sorted(set(authors + (physical + exports + tags if base_only else base))))]
    return "\n".join(lines) + "\n"
