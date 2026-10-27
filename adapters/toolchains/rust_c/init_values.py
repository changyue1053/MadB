"""Installed finite init recipe: prepare existing C04 documents, never launch MadB.

The Rust Host supplies a private final-project root, fixed embedded sources and
an independently checked installation observation. This helper has no Source,
Build or Candidate commit authority. Every returned document is validated and
executed by the existing Host entry point.
"""
from __future__ import annotations

import argparse
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

# The only import root is the Host-materialized runtime next to this script.
RUNTIME_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(RUNTIME_ROOT))
sys.path.insert(0, str(RUNTIME_ROOT / "adapters/generators/rust_c"))
from adapters.toolchains.common.documents import canonical, profile_digest, ref, save, sha_file
from adapters.toolchains.rust_c.project import _public_observation
from adapters.toolchains.rust_c.values_pipeline import Pipeline
from adapters.toolchains.rust_c.values_project import ZERO, finish_context
from support import NativeError, verify_tool
import values_driver
from values_codegen import render_values_cbindgen_config

TARGET = "aarch64-apple-darwin"
VARIANT = "init-values"
NATIVE_TOOLS = frozenset(("ar", "bindgen", "cbindgen", "clang", "finalizer", "libclang", "nm", "python", "rustc", "values_reader"))
# Exact installation inventory, also consumed when the build produces the Rust
# include_bytes list. No glob, neighboring source tree or tool discovery at run.
RUNTIME_FILES = (
    "LICENSE", "NOTICE", "Cargo.lock", "adapters/contracts/Cargo.toml",
    "adapters/contracts/src/native_values.rs",
    "adapters/contracts/src/native_values/names.rs",
    "adapters/contracts/src/native_values/witness.rs",
    "crates/madb-cli/src/bin/madb-native-values-read.rs",
    "adapters/artifacts/rust_c/pack.py",
    "adapters/generators/rust_c/checked_client.py",
    "adapters/generators/rust_c/driver.py",
    "adapters/generators/rust_c/lowering.py",
    "adapters/generators/rust_c/native_ast.py",
    "adapters/generators/rust_c/ownership.py",
    "adapters/generators/rust_c/support.py",
    "adapters/generators/rust_c/values_client.py",
    "adapters/generators/rust_c/values_codegen.py",
    "adapters/generators/rust_c/values_driver.py",
    "adapters/generators/rust_c/values_lowering.py",
    "adapters/generators/rust_c/values_ownership.py",
    "adapters/generators/rust_c/values_runtime.py",
    "adapters/toolchains/common/documents.py",
    "adapters/toolchains/common/pipeline.py",
    "adapters/toolchains/rust_c/common.py",
    "adapters/toolchains/rust_c/project.py",
    "adapters/toolchains/rust_c/values_project.py",
    "adapters/toolchains/rust_c/values_pipeline.py",
    "adapters/toolchains/rust_c/values_worker.py",
    "adapters/toolchains/rust_c/worker.py",
    "adapters/toolchains/rust_c/init_values.py",
)
SCHEMA_FILES = tuple("docs/schemas/" + name + ".schema.json" for name in (
    "build-evidence", "build-task", "cache-entry", "candidate", "case", "common", "composition",
    "contract-ir", "coverage", "dependency-manifest", "dependency-request", "diagnostic-context",
    "execution", "explain-result", "extension-manifest", "extension-wire", "generate-request",
    "init-result", "lock", "output-set", "package-request", "profile-v2", "profile",
    "project-template", "project", "read-request", "report", "verify-request",
))
EXECUTIONS = {
    "prepare": "metadata-execution.json",
    "generate-plan": "generate-execution.json",
    "build-plan": "build-execution.json",
    "package-plan": "package-execution.json",
}
REQUIRED_CASES = ("i01.rust-c-values.bootstrap", "i01.rust-c-values.ordinary")
INSTALLATION_INPUTS = ("installation.json", "execution-resources.json")


class InitPipeline(Pipeline):
    """Planning-only facade; the Rust caller owns all actual C04 execution."""
    def execute(self, *_args, **_kwargs):
        raise RuntimeError("installed initialization cannot launch a nested MadB process")

    def layer(self, layer):
        identity = load(self.root / "domain/current.json")[layer]
        if not isinstance(identity, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", identity):
            raise ValueError("invalid committed batch identity")
        path = self.root / "domain/batches" / identity[7:] / "files"
        if path.resolve(strict=True) != path or not path.is_dir():
            raise ValueError("committed batch domain contains a redirected path")
        return path


def read_regular(path, limit=1024 * 1024):
    path = Path(path)
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(descriptor, "rb") as source:
        info = os.fstat(source.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise ValueError("expected a bounded regular recipe input: " + str(path))
        data = source.read(limit + 1)
        if len(data) > limit:
            raise ValueError("recipe input grew beyond its byte budget")
        return data


def input_file(root, name):
    if not isinstance(name, str) or not name or Path(name).is_absolute() or any(part in ("", ".", "..") for part in name.split("/")):
        raise ValueError("expected a logical installed input path")
    path = root / name
    if path.resolve(strict=True) != path:
        raise ValueError("installed input contains a redirected path")
    return path


def verify_refs(root, references):
    seen = set()
    for item in references:
        name = item["path"]
        if name in seen:
            raise ValueError("duplicate frozen input reference")
        seen.add(name)
        data = read_regular(input_file(root, name), 256 * 1024 * 1024)
        if len(data) != item["size"] or "sha256:" + hashlib.sha256(data).hexdigest() != item["sha256"]:
            raise ValueError("frozen recipe input changed: " + name)


def decode(data):
    def members(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate recipe JSON member: " + key)
            result[key] = value
        return result
    value = json.loads(data, object_pairs_hook=members)
    canonical(value)  # Reuse the exact integer-only public document subset.
    return value


def load(path):
    return decode(read_regular(path))


def write_new(root, name, value):
    data = (canonical(value) + "\n").encode()
    write_bytes_new(root / name, data)


def write_bytes_new(path, data):
    with Path(path).open("xb") as output:
        output.write(data)


def tool_projection(observation):
    return [dict(id=tool["id"], version=tool["version"], sha256=tool["sha256"],
                 host=TARGET, target=TARGET, source="system://observed-native-toolchain/" + name,
                 arguments=[]) for name, tool in sorted(observation["tools"].items())]


def validate_profile(profile, sdk_bytes, observation):
    expected = dict(schema="madb.profile/1", route="RUST-C-ABI", target=TARGET,
        provider=dict(language="rust", runtime="native"), consumer=dict(language="c", runtime="native"),
        direction="consumer-to-provider", abi="c11", artifact_kind="dynamic-library",
        build_system="rustc-clang", deployment_target="14.0", backend_config=None, macros=[], restrictions=[])
    if any(profile.get(key) != value for key, value in expected.items()):
        raise ValueError("profile is outside the installed finite init recipe")
    if (set(observation) != {"schema", "target", "system", "tools", "clang_arguments", "sdk_inputs", "environment"}
            or observation["schema"] != "madb.native-toolchain/1" or observation["target"] != TARGET
            or observation["system"] != "Darwin" or set(observation["tools"]) != NATIVE_TOOLS):
        raise ValueError("installation observation is outside the fixed native recipe")
    if any(set(tool) != {"path", "sha256", "size", "id", "version"} for tool in observation["tools"].values()):
        raise ValueError("installed tools cannot provide hidden invocation arguments")
    if profile["tools"] != tool_projection(observation):
        raise ValueError("original profile tools differ from the installed images")
    sdk_ref = dict(path="toolchain.json", schema="madb.native-toolchain/1",
        sha256="sha256:" + hashlib.sha256(sdk_bytes).hexdigest(), size=len(sdk_bytes))
    if profile["sdk"] != sdk_ref or decode(sdk_bytes) != _public_observation(observation):
        raise ValueError("original SDK bytes differ from the complete installed observation")
    for tool in [*observation["tools"].values(), *observation["sdk_inputs"]]:
        verify_tool(tool)


def backend(observation):
    return dict(schema="madb.rust-c-backend/1", provider="rust", semantics="madb.native-values/1",
        source="author.rs", project="project.json", profile="profile.json",
        cbindgen_config="values-cbindgen.toml", base_cbindgen_config="values-base-cbindgen.toml",
        tools=deepcopy(observation["tools"]), clang_arguments=list(observation["clang_arguments"]),
        environment=dict(observation["environment"]), rust_target=observation["target"],
        sdk_inputs=deepcopy(observation["sdk_inputs"]))


def check_originals(options, root, *, initial):
    sources = {"profile.json": options.profile, "toolchain.json": options.sdk, "author.rs": options.author}
    originals = {name: read_regular(path) for name, path in sources.items()}
    observation = load(options.observation)
    profile = decode(originals["profile.json"])
    validate_profile(profile, originals["toolchain.json"], observation)
    if not initial:
        for name, data in originals.items():
            if read_regular(root / name) != data:
                raise ValueError("original project input changed after preparation: " + name)
        if load(root / "backend.json") != backend(observation):
            raise ValueError("frozen backend differs from the installation observation")
    return originals, profile, observation


def prepare(options, root):
    originals, profile, observation = check_originals(options, root, initial=True)
    for name in (*RUNTIME_FILES, *SCHEMA_FILES):
        read_regular(input_file(root, name))
    # The Host checks the interpreter/framework/stdlib installation itself.
    # Carry its exact private record as a source input so all later Source and
    # Build identities include the executed resource closure, not just launcher.
    for name in INSTALLATION_INPUTS:
        read_regular(input_file(root, name), 16 * 1024 * 1024)
    for name, data in originals.items():
        write_bytes_new(root / name, data)
    config = backend(observation)
    with tempfile.TemporaryDirectory(prefix="init-discovery-", dir=root) as temporary:
        scratch = Path(temporary)
        plan = values_driver.read_plan(config, root, scratch, options.namespace)
        required = ["core.values@1", "core.exact-numbers@1"] + (["core.errors@1", "core.variants@1"] if plan["errors"] else [])
        if (sorted(profile["required_features"]) != sorted(required)
                or not set(required) <= set(profile["supported_features"])):
            raise ValueError("author semantics require a different locked profile")
        for base, key in ((True, "base_cbindgen_config"), (False, "cbindgen_config")):
            write_bytes_new(root / config[key], render_values_cbindgen_config(plan, base_only=base).encode())
        zero = dict(schema="madb.native-identity/1", bootstrap_version=1, interface_digest=ZERO, profile_digest=ZERO)
        surface = values_driver.generated_headers(config, root, scratch, plan, zero)
        # Unlike producer discovery, installed init cannot expand the SDK lock.
        values_driver.header_dependencies(config, scratch, root)
    write_new(root, "backend.json", config)
    names = [*RUNTIME_FILES, *INSTALLATION_INPUTS, "author.rs", "backend.json", "toolchain.json", "values-cbindgen.toml", "values-base-cbindgen.toml"]
    prepared = finish_context(root, profile, config, plan, surface, VARIANT, names, SCHEMA_FILES, write_new)
    check_originals(options, root, initial=False)
    return prepared


def restore(options, root):
    _, profile, _ = check_originals(options, root, initial=False)
    prepared = {key: load(root / name) for key, name in (("read", "read.json"), ("lock", "madb.lock"),
                                                        ("config", "backend.json"), ("project", "project.json"))}
    prepared["profile"] = profile
    expected_inputs = {*RUNTIME_FILES, *INSTALLATION_INPUTS, "author.rs", "backend.json", "toolchain.json", "values-cbindgen.toml", "values-base-cbindgen.toml"}
    if ({item["path"] for item in prepared["read"]["inputs"]} != expected_inputs
            or {item["path"] for item in prepared["lock"]["schemas"]} != set(SCHEMA_FILES)):
        raise ValueError("prepared input inventory differs from the installed recipe")
    verify_refs(root, prepared["read"]["inputs"])
    verify_refs(root, prepared["lock"]["schemas"])
    if (prepared["read"]["profile"] != ref(root, "profile.json", "madb.profile/1")
            or prepared["read"]["lock"] != ref(root, "madb.lock", "madb.lock/1")
            or prepared["read"]["selector"]["profile_digest"] != profile_digest(profile)
            or prepared["project"]["sdk_namespace"] != options.namespace):
        raise ValueError("prepared context no longer identifies the original project")
    return prepared


def checked_metadata(path, expected_digest):
    if not isinstance(expected_digest, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", expected_digest):
        raise ValueError("phase requires a fixed Host metadata receipt")
    data = read_regular(path)
    if "sha256:" + hashlib.sha256(data).hexdigest() != expected_digest:
        raise ValueError("committed metadata differs from the Host receipt")
    return decode(data)


def restore_source(pipeline, expected_digest):
    source = pipeline.layer("source")
    checked_metadata(source.parent / "metadata.json", expected_digest)
    execution = load(pipeline.root / EXECUTIONS["generate-plan"])
    if execution["operation"] != "generate" or execution["context"] != pipeline.context():
        raise ValueError("generation preparation context changed")
    pipeline.accept_generation(execution["recipe"]["inputs"])
    if pipeline.source != source or pipeline._generated_context["metadata_digest"] != expected_digest:
        raise ValueError("source changed while restoring the Host receipt")
    pipeline.checked_build_phases()
    verify_refs(pipeline.root, pipeline.common_inputs())
    verify_refs(pipeline.source, pipeline.outputs["files"])


def restore_build(pipeline, expected_digest):
    pipeline.built = pipeline.layer("build")
    metadata = checked_metadata(pipeline.built.parent / "metadata.json", expected_digest)
    spec = metadata.get("spec", {})
    if (metadata.get("schema") != "madb.storage-batch/1"
            or metadata.get("id") != "sha256:" + pipeline.built.parent.name
            or spec.get("layer") != "build"
            or spec.get("profile_digest") != pipeline.read["selector"]["profile_digest"]
            or spec.get("generation_id") != pipeline.outputs["generation_id"]
            or spec.get("input_digest") != pipeline.contract["source_digest"]
            or spec.get("parent") != "sha256:" + pipeline.source.parent.name):
        raise ValueError("build is not from the current source generation")
    verify_refs(pipeline.built, metadata["files"])


def plan_stage(options):
    root = options.workspace.resolve(strict=True)
    if root != RUNTIME_ROOT:
        raise ValueError("helper must run from the Host-materialized runtime root")
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]{0,63}", options.namespace):
        raise ValueError("invalid checked namespace")
    if not re.fullmatch(r"[0-9a-f]{40}", options.producer_revision):
        raise ValueError("missing exact producer source revision")
    verify_tool(dict(path=str(options.host), sha256=options.host_sha256, size=options.host_size))
    if (root / EXECUTIONS[options.phase]).exists():
        raise ValueError("this preparation stage already has an execution document")
    prepared = prepare(options, root) if options.phase == "prepare" else restore(options, root)
    pipeline = InitPipeline(root, root, prepared, options.host)
    pipeline.python_arguments = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]
    # Override the captured values with the caller's already checked identity;
    # replacing the image before construction must not manufacture a new pin.
    pipeline.host_digest, pipeline.host_size = options.host_sha256, options.host_size
    pipeline.verify_host()
    if options.phase == "prepare":
        execution = pipeline.plan_metadata()
    else:
        pipeline.contract = load(root / "contract.json")
        if options.phase == "generate-plan":
            execution = pipeline.plan_generation()
        else:
            restore_source(pipeline, options.source_metadata_sha256)
            if options.phase == "build-plan":
                execution = pipeline.plan_build()
            else:
                restore_build(pipeline, options.build_metadata_sha256)
                execution = pipeline.plan_package(options.producer_revision, REQUIRED_CASES)
    check_originals(options, root, initial=False)
    pipeline.verify_host()
    if options.phase in ("build-plan", "package-plan"):
        checked_metadata(pipeline.source.parent / "metadata.json", options.source_metadata_sha256)
    if options.phase == "package-plan":
        checked_metadata(pipeline.built.parent / "metadata.json", options.build_metadata_sha256)
    write_new(root, EXECUTIONS[options.phase], execution)
    return EXECUTIONS[options.phase]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=EXECUTIONS)
    for name in ("workspace", "profile", "sdk", "author", "observation", "host"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("namespace", "host-sha256", "producer-revision"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--host-size", type=int, required=True)
    parser.add_argument("--source-metadata-sha256")
    parser.add_argument("--build-metadata-sha256")
    options = parser.parse_args(argv)
    try:
        print(plan_stage(options))
    except (NativeError, OSError, ValueError, KeyError, TypeError) as error:
        print("init native values: " + str(error), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
