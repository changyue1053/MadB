"""Actual napi macros in a cold metadata-only Cargo compilation."""

from __future__ import annotations
import json
from pathlib import Path

import importlib.util

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_rust_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)
rust_frontend = support.load_module(
    _ROOT / "adapters/generators/arkts/rust_frontend.py"
)
rust_dependencies = support.load_module(
    Path(__file__).with_name("rust_dependencies.py")
)
host = support.load_module(Path(__file__).with_name("rust_host.py"))
inventory = support.load_module(Path(__file__).with_name("rust_inventory.py"))

CRATE = "sdk/rust/arkts-bridge"
PROVIDER = "sdk/rust/arkts-provider/src/lib.rs"
AUTHOR_FILES = tuple(
    CRATE + "/" + name
    for name in (
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "frontend-features.json",
        "src/lib.rs",
    )
) + (PROVIDER,)
TARGET = "aarch64-unknown-linux-ohos"
PACKAGE = "madb-arkts-bridge"


def selected_features(config):
    choice = config["rust"]["test_support"]
    if type(choice) is not bool:
        raise ValueError("Rust test-support selection must be explicit boolean")
    return ["--features", "test-support"] if choice else []


def copy_author(inputs, destination):
    destination.mkdir(parents=True, exist_ok=False)
    refs = []
    for name in AUTHOR_FILES:
        source = support.input_path(inputs, name)
        ref = dict(path=name, sha256=support.sha(source), size=source.stat().st_size)
        support.copy_new(source, destination / name)
        support.verify_refs(inputs, [ref])
        support.verify_refs(destination, [ref])
        refs.append(ref)
    return refs


def cargo_environment(config, root, *, metadata=True):
    home, target, raw = root / "cargo-home", root / "target", root / "macro-output"
    for path in (home, target, raw):
        path.mkdir(exist_ok=False)
    vendor = Path(config["rust"]["dependencies"]["root"]) / "vendor"
    (home / "config.toml").write_text(
        '[source.crates-io]\nreplace-with = "madb-vendor"\n'
        "[source.madb-vendor]\ndirectory = " + json.dumps(str(vendor)) + "\n"
    )
    cargo = support.verify_tool(config["tools"]["cargo"])
    rustc = support.verify_tool(config["tools"]["rustc"])
    return {
        **host.environment(config),
        "PATH": host.environment(config)["PATH"],
        "RUSTC": str(rustc),
        "CARGO_HOME": str(home),
        "CARGO_TARGET_DIR": str(target),
        "CARGO_NET_OFFLINE": "true",
        "CARGO_INCREMENTAL": "0",
        "MADB_BUILD_PHASE": "metadata" if metadata else "native",
        "NAPI_TYPE_DEF_TMP_FOLDER": str(raw),
        "NAPI_BUILD_TARGET_NAME": "madb_arkts_bridge",
        "NAPI_DEBUG_GENERATED_CODE": "1",
    }


def feature_graph(metadata, config, inputs):
    packages = {p["id"]: p for p in metadata["packages"]}
    features = {
        packages[n["id"]]["name"]: sorted(n["features"])
        for n in metadata["resolve"]["nodes"]
    }
    expected = {
        "napi-ohos": ["napi1", "napi2", "napi3"],
        "napi-sys-ohos": ["napi" + str(n) for n in range(1, 9)],
    }
    policy = support.read_json(
        support.input_path(inputs, CRATE + "/frontend-features.json")
    )
    for name, allowed in expected.items():
        if (
            features.get(name) != allowed
            or sorted(policy[name]["allowed_resolved_features"]) != allowed
        ):
            raise ValueError(
                "Rust frontend resolved features differ from reviewed ownership policy"
            )
    for name in rust_dependencies.NAPI_PACKAGES:
        if [p["version"] for p in packages.values() if p["name"] == name] != ["1.2.0"]:
            raise ValueError("Rust napi dependency version differs from 1.2.0")
    bridge = features[PACKAGE]
    if set(bridge) != (
        {"default", "test-support"} if config["rust"]["test_support"] else {"default"}
    ):
        raise ValueError("unexpected Rust bridge feature")
    return [
        {
            "name": p["name"],
            "version": p["version"],
            "source": p["source"],
            "features": features[p["name"]],
        }
        for p in sorted(packages.values(), key=lambda p: p["name"])
    ]


def parse_metadata(config, root, gateway_descriptor, capture):
    raw = root / "macro-output" / PACKAGE
    if raw.is_symlink() or not raw.is_file() or raw.stat().st_size > support.MAX_JSON:
        raise ValueError("actual compiler did not emit bounded napi metadata")
    records = [json.loads(line) for line in raw.read_text().splitlines()]
    reader = (
        Path(__file__).resolve().parents[3]
        / "adapters/generators/arkts/read_rust_exports.cjs"
    )
    parsed = support.command(
        config,
        "node",
        [reader, support.verify_tool(config["tools"]["typescript"])],
        cwd=root,
        stdin=json.dumps(records).encode(),
        capture=capture,
    )
    descriptor = json.loads(parsed)
    rust_frontend.compare_exports(descriptor, gateway_descriptor)
    return dict(records=records, raw_sha256=support.sha(raw), descriptor=descriptor)


def read_metadata(inputs, config, work, gateway_descriptor, *, capture=None):
    inputs, work = Path(inputs), Path(work)
    rust_dependencies.verify(inputs, config)
    root = work / "rust-metadata"
    root.mkdir(parents=True, exist_ok=False)
    author = root / "author"
    source_refs = copy_author(inputs, author)
    env = cargo_environment(config, root)
    crate = author / CRATE
    extra = selected_features(config)
    graph_bytes = support.command(
        config,
        "cargo",
        ["metadata", "--offline", "--locked", "--format-version", "1", *extra],
        cwd=crate,
        capture=capture,
        extra_environment=env,
    )
    graph = feature_graph(json.loads(graph_bytes), config, inputs)
    compiler_stdout = support.command(
        config,
        "cargo",
        [
            "check",
            "--lib",
            "--target",
            TARGET,
            "--offline",
            "--locked",
            "--release",
            "--message-format=json",
            "--verbose",
            *extra,
        ],
        cwd=crate,
        capture=capture,
        extra_environment=env,
    )
    macro = parse_metadata(config, root, gateway_descriptor, capture)
    expanded, expansion_checks = inventory.expansion(
        compiler_stdout.decode(), macro["descriptor"]["exports"]
    )
    macro_evidence = dict(
        expanded_rust=expanded.decode(),
        expansion_checks=expansion_checks,
        upstream_sources=inventory.source_inventory(
            Path(config["rust"]["dependencies"]["root"]) / "vendor", sha=support.sha
        ),
    )
    outputs = list(
        (root / "target" / TARGET / "release/build").glob(PACKAGE + "-*/output")
    )
    if len(outputs) != 1:
        raise ValueError("missing unique actual Rust build-script output")
    build_output = outputs[0].read_text()
    if (
        "cargo:rustc-link" in build_output
        or "cargo::rustc-link" in build_output
        or (outputs[0].parent / "out/madb-phase.txt").read_text() != "metadata\n"
    ):
        raise ValueError("metadata phase attempted native linking")
    forbidden = [
        p
        for p in (root / "target" / TARGET).rglob("*")
        if p.is_file() and p.suffix in (".o", ".a", ".so")
    ]
    if forbidden:
        raise ValueError("metadata phase produced native target artifacts")
    support.verify_refs(inputs, source_refs)
    rust_dependencies.verify(inputs, config)
    return dict(
        authority="actual napi-ohos proc macros; raw surface only",
        macro=macro,
        frontend_evidence=macro_evidence,
        resolved_packages=graph,
        inputs=source_refs,
        tools={
            name: {k: config["tools"][name][k] for k in ("sha256", "size")}
            for name in ("cargo", "rustc", "node", "typescript")
        },
        compiler_host=dict(
            xcode=config["rust"]["host"]["xcode"],
            tools={
                name: {k: image[k] for k in ("sha256", "size")}
                for name, image in config["rust"]["host"]["tools"].items()
            },
            trees={
                name: {k: v for k, v in tree.items() if k != "root"}
                for name, tree in config["rust"]["host"]["trees"].items()
            },
        ),
        test_support=config["rust"]["test_support"],
        native_target_artifacts=[],
        typed_api_inferred=False,
    )
