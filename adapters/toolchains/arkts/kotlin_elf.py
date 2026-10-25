"""Kotlin-specific ELF checks; retain compiler exports as an explicit inventory."""

from __future__ import annotations
import re
from pathlib import Path
from .kotlin_dependencies import support

SYSTEM = {
    "libace_napi.z.so",
    "libhilog_ndk.z.so",
    "libuv.so",
    "libffrt.z.so",
    "libtime_service_ndk.so",
    "libhitrace_ndk.z.so",
    "libc.so",
    "libqos.so",
    "libohhidebug.so",
    "libdeviceinfo_ndk.z.so",
    "libhiappevent_ndk.z.so",
    "libz.so",
}


def check(config, inputs, libraries, work, capture):
    bootstrap_source = support.input_path(
        inputs, "sdk/kotlin/arkts-owned/native/bootstrap.cpp.in"
    ).read_text()
    required = set(re.findall(r'dlsym\(library,"([A-Za-z_][A-Za-z0-9_]*)"\)', bootstrap_source))
    required.discard("madb_kn_provider_napi_metric")
    required.discard("madb_kn_owned_memory_metric")
    required.discard("madb_kn_owned_thread_probe")
    if len(required) != 15:
        raise ValueError("Reviewed Kotlin loader inventory changed")
    records = []
    for source in libraries[:2]:
        header = support.command(
            config,
            "cpf_readelf",
            ["--file-header", "--wide", source],
            cwd=work,
            capture=capture,
        ).decode()
        dynamic = support.command(
            config, "cpf_readelf", ["--dynamic", "--wide", source], cwd=work, capture=capture
        ).decode()
        symbols = support.command(
            config, "cpf_readelf", ["--dyn-syms", "--wide", source], cwd=work, capture=capture
        ).decode()
        if (
            not re.search(r"Type:\s+DYN\b", header)
            or not re.search(r"Machine:\s+AArch64\b", header)
            or not re.search(r"Entry point address:\s+0x0\b", header)
        ):
            raise ValueError("Kotlin output is not an entryless aarch64 DSO")
        names = re.findall(r"\(SONAME\).*\[([^]]+)\]", dynamic)
        if names != [source.name]:
            raise ValueError("Kotlin SONAME differs")
        needed = re.findall(r"\(NEEDED\).*\[([^]]+)\]", dynamic)
        if len(needed) != len(set(needed)) or set(needed) - SYSTEM - {"libc++_shared.so"}:
            raise ValueError("Kotlin dependency is outside explicit system/runtime closure")
        if "(RPATH)" in dynamic or "(RUNPATH)" in dynamic:
            raise ValueError("Unexpected Kotlin runtime search path")
        exports = []
        undefined = []
        for line in symbols.splitlines():
            m = re.fullmatch(
                r"\s*\d+:\s+[0-9a-fA-F]+\s+\d+\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)(?:.*)",
                line,
            )
            if not m:
                continue
            name = m[5].split("@")[0]
            if m[4] == "UND":
                undefined.append(name)
            elif m[2] in ("GLOBAL", "WEAK"):
                exports.append(name)
        if "main" in undefined:
            raise ValueError("Shared CPF library retained executable main dependency")
        if source.name == config["native_module"]:
            if (
                set(exports)
                != (
                    {"RegisterOwnedRuntime", "_init", "_fini"}
                    | (
                        {"madb_kn_bootstrap_napi_metric"}
                        if config["kotlin"]["test_support"]
                        else set()
                    )
                )
                or "napi_module_register" not in undefined
            ):
                raise ValueError("Kotlin sole bootstrap registrar/export inventory differs")
        else:
            if config["kotlin"]["test_support"]:
                required.update(("madb_kn_provider_napi_metric", "madb_kn_owned_memory_metric", "madb_kn_owned_thread_probe"))
            elif {"madb_kn_owned_thread_probe", "madb_kn_executor_test_command", "madb_kn_executor_test_cleanup_release", "madb_kn_lifecycle_test_command"} & set(exports):
                raise ValueError("Ordinary Kotlin provider contains thread test controls")
            if required - set(exports) or "RegisterOwnedRuntime" in exports:
                raise ValueError(
                    "Kotlin provider loader ABI is incomplete or duplicates registrar"
                )
        records.append(
            dict(
                path=source.name,
                sha256=support.sha(source),
                soname=names[0],
                needed=needed,
                exports=sorted(exports),
                undefined=sorted(undefined),
                required_loader_symbols=(
                    sorted(required)
                    if source.name != config["native_module"]
                    else ["RegisterOwnedRuntime"]
                ),
            )
        )
    runtime = libraries[2]
    expected = config["kotlin"]["native_runtime"]
    if support.sha(runtime) != expected["sha256"] or runtime.stat().st_size != expected["size"]:
        raise ValueError("Kotlin runtime library differs from selected CPF tool tree")
    return dict(
        scope="exact ELF/loader ABI inventory; target runtime availability is separately observed",
        provider_exports="CPF default Kotlin runtime symbols are preserved and listed, not claimed to be application API",
        libraries=records,
        runtime={k: expected[k] for k in ("soname", "sha256", "size")},
    )
