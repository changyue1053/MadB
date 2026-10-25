"""Evidence for the Rust macro prelude, separate from gateway dynamic counters.

This is a fixed-version source inventory, not a generic Rust reachability proof.
Actual proc-macro output, resolved features and ELF imports remain independent
facts. The runtime gateway owns its separate callsite/target evidence.
"""

from __future__ import annotations
import json
from pathlib import Path
import re

SOURCES = {
    "callback": ("src/bindgen_runtime/callback_info.rs", 27, 80),
    "unknown": ("src/js_values/unknown.rs", 31, 52),
    "raw_result": ("src/bindgen_runtime/js_values.rs", 88, 93),
    "registration": ("src/bindgen_runtime/module_register.rs", 293, 575),
    "error": ("src/error.rs", 392, 529),
}


def expansion(stdout, exports):
    # Cargo JSON events and the backend's debug-expanded Rust share stdout.
    # Keep original generated Rust bytes; names/types still come from actual
    # macro records parsed by the SDK TypeScript AST, not this diagnostic scan.
    lines = []
    for line in stdout.splitlines(keepends=True):
        if line.startswith("{"):
            try:
                event = json.loads(line)
            except ValueError:
                event = None
            if isinstance(event, dict) and event.get("reason", "").startswith(
                ("compiler-", "build-")
            ):
                continue
        lines.append(line)
    generated = "".join(lines)
    flat = re.sub(r"\s+", "", generated)
    count = len(exports)
    checks = {
        "synchronous_callback_wrappers": flat.count("CallbackInfo::<"),
        "borrowed_callback_info": len(
            re.findall(r"CallbackInfo::<\d+usize>::new\(env,cb,None,false\)", flat)
        ),
        "raw_result_conversion": flat.count(
            "<napi_valueasnapi_ohos::bindgen_prelude::ToNapiValue>"
        ),
        "module_exports": flat.count("register_module_export("),
    }
    # Each method has normal constructor and wasm constructor in debug output;
    # the actual OHOS target selects the first. No class/async macro is admitted.
    if (
        checks["synchronous_callback_wrappers"] != count
        or checks["borrowed_callback_info"] != count
        or checks["raw_result_conversion"] != count
        or checks["module_exports"] != 2 * count
        or any(
            token in flat
            for token in ("register_class(", "execute_tokio_future(", "JsDeferred")
        )
    ):
        raise ValueError(
            "actual Rust macro expansion differs from the reviewed synchronous raw frontend"
        )
    return generated.encode(), checks


def source_inventory(vendor, *, sha):
    package = Path(vendor) / "napi-ohos-1.2.0"
    result = []
    for role, (relative, begin, end) in SOURCES.items():
        path = package / relative
        lines = path.read_text().splitlines()
        section = "\n".join(lines[begin - 1 : end])
        result.append(
            dict(
                role=role,
                path="napi-ohos-1.2.0/" + relative,
                sha256=sha(path),
                size=path.stat().st_size,
                lines=[begin, end],
                lexical_napi_calls=sorted(
                    set(re.findall(r"\bnapi_[a-z0-9_]+\s*\(", section))
                ),
            )
        )
    return result


def link_inventory(imported, macro_evidence):
    callbacks = {"napi_get_cb_info"}
    registration = {"napi_create_function", "napi_set_named_property"}
    error = {
        "napi_create_string_utf8",
        "napi_create_error",
        "napi_is_exception_pending",
        "napi_get_and_clear_last_exception",
        "napi_throw",
        "napi_set_named_property",
    }
    known = callbacks | registration | error
    return dict(
        schema="madb.private.rust-napi-inventory/1",
        scope="fixed napi-ohos 1.2.0 napi3 function-only macro frontend; static source/ELF evidence",
        macro=macro_evidence,
        contexts=[
            dict(
                context="platform module registration callback, env valid",
                imports=sorted(set(imported) & registration),
            ),
            dict(
                context="synchronous platform function callback, same borrowed env/frame",
                imports=sorted(set(imported) & callbacks),
            ),
            dict(
                context="synchronous macro/registration failure before callback returns",
                imports=sorted(set(imported) & error),
            ),
        ],
        other_napi_imports=sorted(
            n for n in set(imported) - known if n.startswith("napi_")
        ),
        exclusions=[
            "napi4 CustomGC/Tokio not compiled by resolved feature policy",
            "class infrastructure exists upstream but no class macro registration is emitted",
            "Unknown conversion borrows raw value; raw return is Ok(value), including null",
            "Rust frontend imports are not counted by gateway dynamic observer",
        ],
        coverage="other imports require the separate shared-gateway inventory; ELF alone does not prove execution",
    )
