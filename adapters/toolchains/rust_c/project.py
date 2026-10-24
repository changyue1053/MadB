"""Prepare real first-route source/profile/lock inputs, without minting an IR."""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import shlex
import shutil
import sys
import tempfile

from .common import profile_digest, project_digest, ref, save, semantic, sha_file, unique


VARIANTS = {"c-to-rust": ("rust", False), "c-to-rust-test": ("rust", True), "rust-to-c": ("c", False)}
ZERO = "sha256:" + "0" * 64


def _copy_sources(repo, workspace, provider):
    names = []
    directories = ("sdk/rust/engine", "sdk/c/first-route", "sdk/rust/c-client", "adapters/generators/rust_c",
                   "adapters/toolchains/rust_c", "adapters/toolchains/common", "adapters/artifacts/rust_c")
    for relative in directories:
        for source in sorted((repo / relative).rglob("*")):
            if source.is_file() and "target" not in source.relative_to(repo).parts and source.suffix in (".py", ".c", ".h", ".rs", ".toml", ".md"):
                name = source.relative_to(repo).as_posix()
                target = workspace / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
                names.append(name)
    for name in ("LICENSE", "NOTICE"):
        if (repo / name).is_file():
            shutil.copyfile(repo / name, workspace / name)
            names.append(name)
    protocol = "sdk/rust/engine/runtime-protocol.json" if provider == "rust" else "sdk/c/first-route/runtime-protocol.json"
    shutil.copyfile(repo / protocol, workspace / protocol)
    names.append(protocol)
    schemas = []
    for source in sorted((repo / "docs/schemas").glob("*.schema.json")):
        name = source.relative_to(repo).as_posix()
        target = workspace / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        schemas.append(name)
    if not names or not schemas:
        raise ValueError("the first-route source/schema inventory is incomplete")
    return sorted(set(names)), schemas


def _exports(provider):
    roles = ["bootstrap", "checksum"] if provider == "c" else ["bootstrap", "create", "checksum", "request_close", "await_closed", "release_handle", "observe"]
    prefix = "madb_c_fixture_" if provider == "c" else "madb_fixture_"
    namespace = "madb.c" if provider == "c" else "madb.engine"
    return [{"name": prefix + role, "role": role, "public_id": namespace + "." + role + ".v1"} for role in roles]


def _config(provider, test_support, observation):
    result = dict(schema="madb.rust-c-backend/1", provider=provider, semantics="madb.first-route/1",
                  source="sdk/rust/engine/src/lib.rs" if provider == "rust" else "sdk/c/first-route/madb_c.h",
                  project="project.json", profile="profile.json", exports=_exports(provider),
                  tools=deepcopy(observation["tools"]), clang_arguments=list(observation["clang_arguments"]),
                  environment=dict(observation["environment"]), rust_target=observation["target"],
                  sdk_inputs=deepcopy(observation["sdk_inputs"]), test_support=test_support)
    if provider == "rust":
        result["cbindgen_config"] = "sdk/rust/engine/cbindgen.toml"
        if test_support:
            result["test_support_header"] = "sdk/rust/engine/test-support.h"
    else:
        result["wrapper"] = dict(manifest="sdk/rust/c-client/Cargo.toml", source="sdk/rust/c-client/src/lib.rs", build_script="sdk/rust/c-client/build.rs")
    return result


def _sdk_dependencies(config, request, workspace, observation, native):
    from support import run_tool
    with tempfile.TemporaryDirectory(prefix="madb-native-discovery-") as temporary:
        header = native._header(request, config, workspace, Path(temporary))
        text = run_tool(config, "clang", [*config["clang_arguments"], "-M", "-x", "c", header]).decode()
        names = shlex.split(text.replace("\\\n", " ").partition(":")[2])
        known = {Path(item["path"]).resolve(): item for item in observation["sdk_inputs"]}
        for name in names:
            path = Path(name).resolve(strict=True)
            if path == header.resolve():
                continue
            if path.is_relative_to(workspace.resolve()):
                raise ValueError("indirect user headers are outside the first-route native reader")
            known[path] = dict(path=str(path), sha256=sha_file(path), size=path.stat().st_size)
        observation["sdk_inputs"] = [known[path] for path in sorted(known)]
        config["sdk_inputs"] = deepcopy(observation["sdk_inputs"])


def _profile(provider, variant, config, observation, workspace):
    target = observation["target"]
    features = ["core.values@1", "core.exact-numbers@1"]
    if provider == "rust":
        features += ["core.resources@1", "core.explicit-close@1", "core.callbacks@1"]
    tools = []
    for name, tool in sorted(observation["tools"].items()):
        tools.append(dict(id=tool["id"], version=tool["version"], sha256=tool["sha256"], host=target, target=target,
                          source="system://observed-native-toolchain/" + name,
                          # Exact executable arguments remain in the locked backend input.
                          arguments=[]))
    return dict(schema="madb.profile/1", id="c05-" + variant + "-" + target, route="RUST-C-ABI",
                provider=dict(language=provider, runtime="native"), consumer=dict(language="c" if provider == "rust" else "rust", runtime="native"),
                direction="consumer-to-provider", target=target, abi="c11",
                artifact_kind="dynamic-library" if provider == "rust" else "static-library", build_system="rustc-clang",
                deployment_target="14.0" if "apple" in target else "native-host",
                tools=tools, sdk=ref(workspace, "toolchain.json", "madb.native-toolchain/1"), macros=[],
                required_features=sorted(features), supported_features=sorted(features), restrictions=[],
                defaults=dict(values=dict(ownership="copy"), origin=dict(authority="profile-default", id="native.copy-values", location=dict(path="profile.json", line=1, column=1))),
                # C02's generic backend-config registry deliberately stays closed.
                # This internal recipe is an exact read/lock/execution source input.
                backend_config=None)


def _public_observation(observation, runtime_protocol=None):
    """Publish image/content identities without the producer's absolute paths."""
    arguments = observation["clang_arguments"]
    roots = []
    for flag, name in (("-isysroot", "sdk"), ("-resource-dir", "clang-resource")):
        if flag in arguments:
            roots.append((Path(arguments[arguments.index(flag) + 1]).resolve(), name))
    sdk_inputs = []
    for item in observation["sdk_inputs"]:
        path = Path(item["path"]).resolve()
        logical = next((name + "/" + path.relative_to(root).as_posix()
                        for root, name in roots if path.is_relative_to(root)), None)
        if logical is None:
            # Rust sysroot objects and system libraries need no host-specific prefix.
            logical = "native/" + path.name + "-" + item["sha256"].removeprefix("sha256:")
        sdk_inputs.append(dict(name=logical, sha256=item["sha256"], size=item["size"]))
    compiler = dict(target=arguments[arguments.index("-target") + 1],
                    language_standard=next(item.removeprefix("-std=") for item in arguments if item.startswith("-std=")),
                    warnings=[item for item in arguments if item in ("-Wall", "-Wextra", "-Werror")])
    deployment = next((item.removeprefix("-mmacosx-version-min=") for item in arguments if item.startswith("-mmacosx-version-min=")), None)
    if deployment is not None:
        compiler["deployment_target"] = deployment
    result = dict(schema=observation["schema"], target=observation["target"], system=observation["system"],
                  tools=sorted(({key: tool[key] for key in ("id", "version", "sha256", "size")}
                                for tool in observation["tools"].values()), key=lambda item: item["id"]),
                  sdk_inputs=sorted(sdk_inputs, key=lambda item: item["name"]), compiler=compiler)
    if runtime_protocol is not None:
        result["runtime_protocol"] = deepcopy(runtime_protocol)
    return result


def _policies(seed, config):
    """Author the declared ABI-v1 policies; the existing Rust resolver validates them."""
    policies = []
    roles = {item["name"]: item["role"] for item in config["exports"]}
    release = next((item["public_id"] for item in config["exports"] if item["role"] == "release_handle"), None)
    def add(symbol, slot, values):
        identity = "native." + roles[symbol["shape"]["link_name"]] + "." + slot["kind"] + ("." + slot["name"] if "name" in slot else "")
        policies.append(dict(selector=dict(kind="public_id", id=symbol["public_id"]), slot=slot, families=[], values=values,
                             origin=dict(authority="project-policy", id=identity, location=dict(path="project.json", line=1, column=1))))
    for symbol in seed["symbols"]:
        shape = symbol["shape"]
        role = roles[shape["link_name"]]
        blocking = "unbounded" if role == "observe" else "non-blocking" if role == "request_close" else "bounded"
        add(symbol, dict(kind="call"), dict(threading="caller", blocking=blocking,
            delivery="registered" if role == "observe" else "work-completed", cancellation="none", cancellation_confirmation="not-supported"))
        if shape["result"] == "handle":
            add(symbol, dict(kind="return"), dict(ownership="transfer", releaser=release, close="request-await-release", release_thread="caller"))
        elif shape["result"] != "unit":
            add(symbol, dict(kind="return"), dict(ownership="copy"))
        for parameter in shape["parameters"]:
            kind = parameter["type_id"]
            slot = dict(kind="parameter", name=parameter["name"])
            if kind == "handle":
                add(symbol, slot, dict(ownership="borrow", borrow_scope="call", keep_alive="call-frame", close="request-await-release", release_thread="caller"))
            elif kind == "bytes":
                add(symbol, slot, dict(ownership="borrow", borrow_scope="call", keep_alive="call-frame"))
            elif kind == "state_callback":
                add(symbol, slot, dict(ownership="borrow", borrow_scope="subscription", keep_alive="source-lease", synchronous_callback=True,
                                      reentrant=True, threading="serial-executor", context_owner="caller", stop="request-and-drain"))
    return policies


def prepare(repo: Path, workspace: Path, observation: dict, variant: str) -> dict:
    repo, workspace = Path(repo).resolve(), Path(workspace).resolve()
    # Each variant owns its dependency closure; another variant's discovered
    # headers must not change this profile merely by being prepared first.
    observation = deepcopy(observation)
    if variant not in VARIANTS:
        raise ValueError("unknown first-route variant: " + variant)
    workspace.mkdir(parents=True, exist_ok=False)
    provider, test_support = VARIANTS[variant]
    names, schema_names = _copy_sources(repo, workspace, provider)
    config = _config(provider, test_support, observation)
    generator_dir = workspace / "adapters/generators/rust_c"
    sys.path.insert(0, str(generator_dir))
    try:
        import driver as native
        selector = dict(route="RUST-C-ABI", profile_id="c05-" + variant + "-" + observation["target"], profile_digest=ZERO,
                        target=observation["target"], direction="consumer-to-provider")
        preliminary = dict(schema="madb.read-request/1", boundary_id=variant, selector=selector,
                           inputs=[ref(workspace, name, "madb.source-input/1") for name in names])
        _sdk_dependencies(config, preliminary, workspace, observation, native)
        protocol_path = "sdk/rust/engine/runtime-protocol.json" if provider == "rust" else "sdk/c/first-route/runtime-protocol.json"
        protocol = json.loads((workspace / protocol_path).read_bytes())
        if protocol.get("schema") != "madb.native-runtime-protocol/1" or protocol.get("provider") != provider:
            raise ValueError("runtime protocol differs from the selected provider")
        save(workspace, "toolchain.json", _public_observation(observation, protocol))
        save(workspace, "backend.json", config)
        profile = _profile(provider, variant, config, observation, workspace)
        save(workspace, "profile.json", profile)
        selector["profile_digest"] = profile_digest(profile)
        project = dict(schema="madb.project/1", project="madb-first-route", sdk_namespace="madb",
                       boundaries=[dict(id=variant, route="RUST-C-ABI", direction="consumer-to-provider",
                           source=dict(kind="rust-c-declaration" if provider == "rust" else "c-header", path=config["source"]),
                           profile=ref(workspace, "profile.json", "madb.profile/1"), target=observation["target"], output="generated")],
                       policies=[], policy_families=[], artifacts=dict(package="madb-first-route-" + variant, manifest="candidate.json"))
        save(workspace, "project.json", project)
        names += ["backend.json", "toolchain.json"]
        inputs = unique(ref(workspace, name, "madb.source-input/1") for name in names)
        preliminary["inputs"] = inputs
        with tempfile.TemporaryDirectory(prefix="madb-native-policy-") as temporary:
            seed = native.metadata_seed(preliminary, config, workspace, Path(temporary))
        project["policies"] = _policies(seed, config)
        save(workspace, "project.json", project)
    finally:
        sys.path.pop(0)
    component_files = [item for item in inputs if item["path"].startswith("adapters/generators/rust_c/")]
    component = dict(id="org.madb.rust-c/reader-generator", version="0.1.0", sha256=semantic("madb.native-component/1", component_files))
    lock = dict(schema="madb.lock/1", project_digest=project_digest(project),
                profiles=[dict(id=profile["id"], semantic_digest=profile_digest(profile), file=ref(workspace, "profile.json", "madb.profile/1"))],
                tools=profile["tools"], components=[component],
                inputs=[{key: item[key] for key in ("path", "sha256", "size")} for item in inputs],
                schemas=[ref(workspace, name) for name in schema_names])
    save(workspace, "madb.lock", lock)
    read = dict(schema="madb.read-request/1", boundary_id=variant, selector=selector, inputs=inputs, component=component,
                lock=ref(workspace, "madb.lock", "madb.lock/1"), profile=ref(workspace, "profile.json", "madb.profile/1"), output_root="staging/read")
    save(workspace, "read.json", read)
    return dict(profile=profile, project=project, read=read, lock=lock, config=config)
