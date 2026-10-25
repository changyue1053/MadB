"""Verify the actual Rust-derived C++ translation units with the one observer."""

from __future__ import annotations
import importlib.util
from pathlib import Path
import shutil

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_rust_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)


def verify(config, inputs, work, capture):
    if not config["rust"]["test_support"]:
        return None
    inputs, work = Path(inputs), Path(work)
    worker = support.load_module(Path(__file__).with_name("worker.py"))
    common, includes = worker.native_arguments(config, inputs, work)
    units = config["build"]["units"]
    by_file = {Path(u["source"]).name: u for u in units}
    required = {"api.cpp", "runtime.cpp", "runtime_ffi.cpp", "support.cpp"}
    if not required <= by_file.keys():
        raise ValueError(
            "Rust companion observer is missing an actual gateway translation unit"
        )
    prefix = config["composition"]["prefix"]
    source_root = inputs / prefix
    raw = [
        support.input_path(inputs, by_file[n]["source"])
        for n in ("api.cpp", "runtime_ffi.cpp")
    ]
    scratch = work / "rust-observer"
    scratch.mkdir(exist_ok=False)
    preprocessed = []
    for unit in units:
        source = support.input_path(inputs, unit["source"])
        if unit["language"] != "cpp":
            raise ValueError(
                "Rust companion native archive contains an unexpected non-C++ unit"
            )
        data = support.command(
            config,
            "clangxx",
            [*common, "-std=c++17", *includes, "-E", source],
            cwd=work,
            capture=capture,
            maximum=32 * support.MAX_JSON,
        )
        output = scratch / (unit["name"] + ".ii")
        output.write_bytes(data)
        if source.name in required:
            preprocessed.append(output)
    inspector = support.input_path(
        inputs, "sdk/arkts/test-support/native/observer_inventory.py"
    )
    output = scratch / "inventory.json"
    args = [
        inspector,
        "--source-root",
        source_root,
        "--observer-header",
        source_root / "include/observer.hpp",
        "--napi-headers",
        Path(config["native_sdk"]) / "sysroot/usr/include/napi",
        "--out",
        output,
    ]
    for path in raw:
        args += ["--source-file", path]
    for path in preprocessed:
        args += ["--preprocessed", path]
    support.command(config, "python", args, cwd=work, capture=capture)
    result = support.read_json(output, 8 * support.MAX_JSON)
    if (
        not result.get("preprocessing_checked")
        or result.get("preprocessed_source_call_count") is None
    ):
        raise ValueError(
            "Rust gateway inventory has no actual compiler expansion proof"
        )
    # Raw paths and preprocessor line markers are diagnostic evidence, not
    # deterministic output identity. Keep original facts in the accepted log.
    capture.append(dict(kind="actual-rust-gateway-observer", result=result))
    source_refs = []
    for path in result["preprocessed_source_paths"]:
        path = Path(path)
        if not path.is_relative_to(inputs):
            raise ValueError("observer source escaped frozen C04 inputs")
        source_refs.append(
            dict(
                path=path.relative_to(inputs).as_posix(),
                sha256=support.sha(path),
                size=path.stat().st_size,
            )
        )
    summary = dict(
        scope="actual Rust-derived C++ gateway only; excludes Rust macro prelude",
        compiled_units=[u["source"] for u in units],
        preprocessed_gateway_units=[by_file[n]["source"] for n in sorted(required)],
        observed_sources=sorted(source_refs, key=lambda r: r["path"]),
        source_call_count=result["preprocessed_source_call_count"],
        meaning="static sites in sources seen by actual compiler preprocessing, not dynamic executions",
    )
    shutil.rmtree(scratch)
    return summary
