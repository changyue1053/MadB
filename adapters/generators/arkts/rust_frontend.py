"""Restricted raw Rust seam generation; the typed ArkTS declaration stays separate."""

from __future__ import annotations
import re

CALLBACK_BEGIN = "// MADB_NATIVE_CALLBACK_BEGIN"
CALLBACK_END = "// MADB_NATIVE_CALLBACK_END"
REGISTRAR_BEGIN = "// MADB_NATIVE_REGISTRAR_BEGIN"
REGISTRAR_END = "// MADB_NATIVE_REGISTRAR_END"


def _replace_block(source, begin, end, replacement):
    if source.count(begin) != 1 or source.count(end) != 1:
        raise ValueError("Rust extraction requires one reviewed native marker pair")
    left, right = source.index(begin), source.index(end)
    if right <= left:
        raise ValueError("reversed native marker pair")
    return source[:left] + replacement + source[right + len(end) :]


def raw_api(source, *, test_support=False):
    """Keep every business/lifecycle statement; replace only the callback frame."""
    entry = "#define MADB_NATIVE_TEST_ENTRY(env) napi_observer::Scope napiEntry(napi_observer::Mode::Entry, env)"
    use = "MADB_NATIVE_TEST_ENTRY(env);"
    if test_support:
        if (
            source.count(entry) != 1
            or source.count(use) != 1
            or source.count("MADB_NATIVE_TEST_ENTRY") != 2
            or source.index(entry) >= source.index(CALLBACK_BEGIN)
            or not source.index(CALLBACK_BEGIN)
            < source.index(use)
            < source.index(CALLBACK_END)
        ):
            raise ValueError(
                "test raw extraction requires the exact companion Entry authority"
            )
    elif "MADB_NATIVE_TEST_ENTRY" in source:
        raise ValueError("ordinary raw entry cannot contain test observer authority")
    callback = r"""// Generated same-frame raw entry. No callback_info is fabricated.
#define BEGIN(name, count) \
    napi_value raw_##name(napi_env env, size_t argc, const napi_value *argv) noexcept { \
        try { \
            if (argc != (count) || ((count) > 0 && !argv)) \
                return error(env, "InvalidArgument");
"""
    if test_support:
        callback = callback.replace(
            "        try {", "        MADB_NATIVE_TEST_ENTRY(env); \\\n        try {"
        )
    source = _replace_block(source, CALLBACK_BEGIN, CALLBACK_END, callback)
    return _replace_block(source, REGISTRAR_BEGIN, REGISTRAR_END, "")


def methods(descriptor):
    result = descriptor["methods"]
    names = [m["name"] for m in result]
    if (
        len(result) not in (16, 32)
        or len(set(names)) != len(names)
        or any(not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", n) for n in names)
        or any(len(m["params"]) > 4 for m in result)
    ):
        raise ValueError("unsupported Rust raw gateway method set")
    return result


def render_bridge(descriptor, *, test_support=False):
    selected = methods(descriptor)
    if len(selected) != (32 if test_support else 16):
        raise ValueError("raw bridge variant differs from actual method authority")
    declarations = "\n".join(
        f'napi_value raw_{m["name"]}(napi_env, size_t, const napi_value*) noexcept;'
        for m in selected
    )
    cases = "\n".join(
        f'case {i}: return gateway::raw_{m["name"]}(env, argc, argv);'
        for i, m in enumerate(selected)
    )
    header = """// Generated private same-frame C seam; not public ABI.
#pragma once
#include <napi/native_api.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#define MADB_NOEXCEPT noexcept
#else
#define MADB_NOEXCEPT
#endif
// Borrowed env/argv expire on return. Retained callbacks use real napi_ref.
// argv may be null only for argc0; raw nullptr preserves pending exceptions.
napi_value madb_c07_native_call(napi_env, uint32_t, size_t, const napi_value*) MADB_NOEXCEPT;
#ifdef __cplusplus
}
#endif
"""
    source = (
        """// Generated from the checked gateway declaration; sole registrar is Rust.
#include "runtime_ffi.h"
#include "codec.hpp"
#include <new>
__asm__(".hidden mp_rust_create\\n.hidden mp_rust_layout");
namespace gateway {
"""
        + declarations
        + """
}
extern "C" napi_value madb_c07_native_call(napi_env env, uint32_t method,
    size_t argc, const napi_value* argv) noexcept {
    if (!env) return nullptr;
    try {
        if (argc > 4 || (argc > 0 && !argv)) return gateway::error(env, "InvalidArgument");
        for (size_t i = 0; i < argc; ++i)
            if (!argv[i]) return gateway::error(env, "InvalidArgument");
        switch (method) {
"""
        + cases
        + """
        default: return gateway::error(env, "InvalidArgument");
        }
    } catch (const std::bad_alloc&) { return gateway::error(env, "ResourceExhausted"); }
      catch (...) { return gateway::error(env, "InternalFailure"); }
}
"""
    )
    if test_support:
        source = source.replace(
            "#include <new>", '#include <new>\n#include "observer.hpp"'
        )
        source = source.replace(
            "namespace gateway {", '__asm__(".hidden mp_rust_test_memory_bind");\nnamespace gateway {', 1
        )
        source = source.replace(
            "    if (!env) return nullptr;",
            "    if (!env) return nullptr;\n    gateway::napi_observer::Scope napiEntry(gateway::napi_observer::Mode::Entry, env);",
        )
    return {"runtime_ffi.h": header.encode(), "runtime_ffi.cpp": source.encode()}


def compare_exports(raw_descriptor, gateway_descriptor):
    """Macro records authorize raw exports, never Swift/ArkTS type inference."""
    expected = {m["name"]: len(m["params"]) for m in methods(gateway_descriptor)}
    exports = raw_descriptor["exports"]
    actual = {e["name"]: e["arity"] for e in exports}
    if (
        len(actual) != len(exports)
        or actual != expected
        or [e["name"] for e in exports] != list(expected)
    ):
        raise ValueError(
            "actual Rust macro exports differ from typed gateway names/arity"
        )
    if any(e["rawReturn"] != "unknown" for e in exports):
        raise ValueError("raw Rust result cannot be promoted to a typed public model")
