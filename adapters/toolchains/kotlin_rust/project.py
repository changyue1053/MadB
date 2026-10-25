"""Prepare the four C06 profiles from locked tools and actual declaration readers."""
from __future__ import annotations

from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import shutil

from ..common.documents import profile_digest, project_digest, ref, save, semantic, sha_file, unique
from . import tools

ZERO = "sha256:" + "0" * 64
FEATURES = sorted([
    "core.values@1", "core.exact-numbers@1", "core.variants@1", "core.errors@1",
    "core.resources@1", "core.backend-resources@1", "core.explicit-close@1",
    "core.callbacks@1", "core.async@1", "core.streams@1",
])
VARIANTS = {
    "kotlin-jvm-rust": dict(route="KJVM-RUST-UNIFFI", provider="rust", consumer="kotlin", runtime="jvm", abi="uniffi-0.32.2", artifact="jvm-jar-and-native-library", build="cargo-kotlinc", protocol="sdk/rust/uniffi/runtime-protocol.json"),
    "kotlin-native-rust": dict(route="KN-RUST-GOBLEY", provider="rust", consumer="kotlin", runtime="native", abi="uniffi-0.29.4", artifact="klib-and-native-library", build="cargo-konanc", protocol="sdk/rust/gobley/runtime-protocol.json"),
    "swift-rust": dict(route="SWIFT-RUST-UNIFFI", provider="rust", consumer="swift", runtime="native", abi="uniffi-0.32.2", artifact="swift-source-and-native-library", build="cargo-swiftc", protocol="sdk/rust/uniffi/runtime-protocol.json"),
    "swift-kotlin": dict(route="KN-SWIFT-OBJC", provider="kotlin", consumer="swift", runtime="native", abi="objective-c", artifact="framework", build="gradle-swiftc", protocol="sdk/kotlin/skie/runtime-protocol.json"),
}
SOURCE_ROOTS = (
    "sdk/rust/boundary-core", "sdk/rust/uniffi", "sdk/rust/gobley", "sdk/kotlin/skie",
    "sdk/swift/skie", "sdk/swift/uniffi", "sdk/kotlin/uniffi-jvm", "sdk/kotlin/gobley",
    "adapters/generators/kotlin_rust", "adapters/generators/uniffi", "adapters/generators/gobley", "adapters/generators/skie",
    "adapters/toolchains/kotlin_rust", "adapters/toolchains/common",
)
EXCLUDED_DIRECTORIES = {".git", ".gradle", ".kotlin", "target", "build", "__pycache__", ".DS_Store"}
EXTRA_INPUTS = ("adapters/toolchains/rust_c/common.py", "adapters/artifacts/rust_c/pack.py", "rust-toolchain.toml", "LICENSE", "NOTICE")


def _copy_file(repo, workspace, name):
    source = repo / name
    if source.is_symlink() or not source.is_file() or not source.resolve().is_relative_to(repo):
        raise ValueError("author input is missing or escapes the repository: " + name)
    target = workspace / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)


def _copy_sources(repo, workspace):
    names = set(EXTRA_INPUTS)
    for relative in SOURCE_ROOTS:
        directory = repo / relative
        if not directory.exists():
            continue
        for source in directory.rglob("*"):
            parts = source.relative_to(directory).parts
            if any(part in EXCLUDED_DIRECTORIES for part in parts) or source.suffix in (".pyc", ".pyo"):
                continue
            if source.is_symlink():
                raise ValueError("symlink is not a frozen author file: " + str(source))
            if source.is_file():
                names.add(source.relative_to(repo).as_posix())
    schemas = sorted(path.relative_to(repo).as_posix() for path in (repo / "docs/schemas").glob("*.schema.json"))
    if not schemas:
        raise ValueError("public schemas are absent")
    for name in sorted(names) + schemas:
        _copy_file(repo, workspace, name)
    return sorted(names), schemas


def _required_trees(observation, variant):
    required = {"runtime-dependencies", "python-runtime"}
    if variant == "swift-kotlin":
        required.add("skie-maven")
    else:
        required.add("cargo-vendor")
    missing = required - observation["trees"].keys()
    if missing:
        raise ValueError("selected profile lacks locked dependency trees: " + ", ".join(sorted(missing)))


def _verify_resolution_source(workspace, variant):
    if variant != "swift-kotlin":
        return
    path = workspace / "adapters/toolchains/kotlin_rust/skie-dependencies.json"
    source = json.loads(path.read_text())["resolved_from"]
    source_path = workspace / source["path"]
    if not source_path.is_file() or sha_file(source_path) != source["sha256"]:
        raise ValueError("SKIE offline closure was resolved from different Gradle verification metadata")


def _config(variant, observation):
    selected = VARIANTS[variant]
    source = "sdk/kotlin/skie/src/macosArm64Main/kotlin/madb/skie/Boundary.kt" if variant == "swift-kotlin" else "sdk/rust/uniffi/src/boundary.udl"
    result = dict(schema="madb.kotlin-rust-backend/1", variant=variant, source=source, project="project.json", profile="profile.json",
                  features=FEATURES, tools=deepcopy(observation["tools"]), toolchain=deepcopy(observation),
                  rust_target=observation["target"], deployment_target="14.0", phase_timeout_seconds=900, phase_termination_grace_ms=6000,
                  precision="artifact-derived" if variant == "swift-kotlin" else "schema-derived", test_support=variant == "swift-kotlin")
    if variant == "swift-kotlin":
        result["compiler_workspace"] = dict(root="/private/tmp/madb-compiler/c06-skie", strategy="input-addressed-exclusive-cold-build")
    if variant != "kotlin-jvm-rust":
        result["smoke_workspace"] = dict(root="/private/tmp/madb-compiler/c06-smoke", strategy="input-addressed-exclusive-cold-build")
    if variant != "swift-kotlin":
        result["rust_compiler_workspace"] = dict(root="/private/tmp/madb-compiler/c06-rust", strategy="input-addressed-exclusive-cold-build")
    if selected["abi"] == "uniffi-0.32.2":
        if "uniffi" not in observation:
            raise ValueError("verified UniFFI installation is required")
        result["uniffi"] = deepcopy(observation["uniffi"])
    elif variant == "kotlin-native-rust":
        if observation.get("gobley") is None:
            raise ValueError("verified Gobley installation is required")
        result["gobley"] = deepcopy(observation["gobley"])
    return result


def _backend_identity(workspace, config):
    variant = config["variant"]
    if variant == "swift-kotlin":
        return dict(id="org.madb.kotlin-skie", kotlin="2.4.20", skie="0.10.15", distribution="produceDistributableFramework",
                    compiler_workspace=dict(strategy="input-addressed-exclusive-cold-build", identity="author-inputs/profile/interface/portable-tool-hashes"),
                    gradle_verification=ref(workspace, "sdk/kotlin/skie/gradle/verification-metadata.xml"),
                    gradle_lock=ref(workspace, "sdk/kotlin/skie/gradle.lockfile"))
    if variant == "kotlin-native-rust":
        name = "adapters/generators/gobley/toolchain.json"
        return dict(id="org.madb.gobley", source_recipe=json.loads((workspace / name).read_text()),
                    recipe_digest=sha_file(workspace / name), receipt_sha256=config["gobley"]["receipt_sha256"])
    installation = config["uniffi"]
    receipt = json.loads((Path(installation["root"]) / "receipt.json").read_text())
    sources = {name: {key: item[key] for key in ("source_url", "source_sha256", "patches", "inventory_sha256")}
               for name, item in receipt["sources"].items()}
    return dict(id="org.madb.uniffi", version="0.32.2", receipt_sha256="sha256:" + installation["receipt_sha256"].removeprefix("sha256:"), sources=sources)


def _verify_python_runtime(observation):
    runtime = observation.get("python_runtime")
    python = observation.get("tools", {}).get("python", {})
    if not runtime or not {"version", "system_build", "system_libraries", "root", "search_paths", "libraries"}.issubset(runtime):
        raise ValueError("managed observation lacks the complete Python runtime closure")
    if python.get("invocation_arguments") != tools.python_runtime.ISOLATION or python.get("version") != runtime["version"]:
        raise ValueError("Python invocation or runtime version differs from the observed isolated tool")


def _verify_helper_sources(workspace, observation, variant):
    if variant == "swift-kotlin":
        return
    name = "metadata_v029" if variant == "kotlin-native-rust" else "metadata_v032"
    helper = observation.get("declaration_helpers", {}).get(name)
    if not helper or not helper.get("inputs"):
        raise ValueError("declaration helper has no author source proof: " + name)
    seen = set()
    for item in helper["inputs"]:
        logical = item["path"]
        if Path(logical).is_absolute() or any(part in ("", ".", "..") for part in logical.split("/")) or logical in seen:
            raise ValueError("declaration helper has an invalid author input")
        seen.add(logical)
        if ref(workspace, logical) != item:
            raise ValueError("declaration helper is stale for frozen author input: " + logical)
    expected = ["gobley-029"] if variant == "kotlin-native-rust" else []
    if helper.get("features") != expected:
        raise ValueError("declaration helper feature set differs from the selected backend")


def _load_runtime_protocol(workspace, variant):
    protocol_name = VARIANTS[variant]["protocol"]
    protocol_path = workspace / protocol_name
    if not protocol_path.is_file():
        raise ValueError("selected SDK has no authored runtime protocol: " + protocol_name)
    protocol = json.loads(protocol_path.read_text())
    if not isinstance(protocol, dict) or protocol.get("schema") != "madb.native-runtime-protocol/1":
        raise ValueError("unsupported runtime protocol schema: " + protocol_name)
    if protocol.get("schema_version") != "1.0.0":
        raise ValueError("unsupported C06 runtime protocol schema_version: " + protocol_name)
    # Check the authored format before compilers run. Concrete budget values are
    # verified against actual runtime behavior by the independent consumer Lab.
    required_budgets = {
        "max_payload_bytes", "max_retained_payload_bytes", "max_pending_events",
        "state_pending_capacity_per_subscription", "max_subscription_handles",
        "max_operation_handles", "max_registered_waiters", "owned_worker_threads_per_engine",
    }
    budgets = protocol.get("budgets")
    if not isinstance(budgets, dict) or not required_budgets.issubset(budgets):
        raise ValueError("C06 runtime protocol is missing required budgets: " + protocol_name)
    for name, value in budgets.items():
        if type(value) is not int or value <= 0:
            raise ValueError("C06 runtime protocol budget must be a positive integer: " + protocol_name + "#budgets/" + name)
    return protocol


def _public_observation(workspace, observation, config):
    protocol_name = VARIANTS[config["variant"]]["protocol"]
    protocol = _load_runtime_protocol(workspace, config["variant"])
    managed = observation["managed"]
    backend = _backend_identity(workspace, config)
    if "smoke_workspace" in config:
        backend["smoke_workspace"] = dict(strategy="input-addressed-exclusive-cold-build", identity="sdk-bytes/profile/interface/compiler-flags/smoke-source")
    if "rust_compiler_workspace" in config:
        backend["rust_compiler_workspace"] = dict(strategy="input-addressed-exclusive-cold-build", identity="sdk-rust-source/profile/test-support/tools/trees/recipe-source")
    recipe = workspace / "adapters/toolchains/kotlin_rust/toolchain.json"
    return dict(schema="madb.native-toolchain/1", target=observation["target"], system=observation["system"],
                managed_installation=dict(receipt_sha256=managed["receipt_sha256"], recipe_sha256=sha_file(recipe), components=sorted(managed["paths"])),
                tools=sorted(({key: item[key] for key in ("id", "version", "sha256", "size")} for item in observation["tools"].values()), key=lambda item: item["id"]),
                trees=[dict(name=name, **{key: tree[key] for key in ("sha256", "entries", "size")}) for name, tree in sorted(observation["trees"].items())],
                python_runtime={key: deepcopy(observation["python_runtime"][key]) for key in ("version", "system_build", "system_libraries")} if "python_runtime" in observation else None,
                xcode=deepcopy(observation["xcode"]), compiler=dict(target="arm64-apple-macosx14.0", deployment_target="14.0", configuration="Release"),
                backend=backend, execution_policy={key: config[key] for key in ("phase_timeout_seconds", "phase_termination_grace_ms")},
                declaration_helpers=deepcopy(observation.get("declaration_helpers", {})),
                runtime_protocol=protocol, test_support=config["test_support"],
                provider_role="conformance-provider",
                runtime_protocol_source=ref(workspace, protocol_name))


def _profile(workspace, observation, config):
    variant, target = config["variant"], observation["target"]
    selected = VARIANTS[variant]
    public_tools = [dict(id=tool["id"], version=tool["version"], sha256=tool["sha256"], host=target, target=target,
                         source=tool["source"], arguments=deepcopy(tool.get("invocation_arguments", []))) for _, tool in sorted(observation["tools"].items())]
    return dict(schema="madb.profile/1", id="c06-" + variant + "-" + target, route=selected["route"],
                provider=dict(language=selected["provider"], runtime="native"), consumer=dict(language=selected["consumer"], runtime=selected["runtime"]),
                direction="consumer-to-provider", target=target, abi=selected["abi"], artifact_kind=selected["artifact"], build_system=selected["build"], deployment_target="14.0",
                tools=public_tools, sdk=ref(workspace, "toolchain.json", "madb.native-toolchain/1"), macros=[],
                required_features=FEATURES, supported_features=FEATURES, restrictions=[],
                defaults=dict(values=dict(ownership="copy"), origin=dict(authority="profile-default", id="managed.copy-values", location=dict(path="profile.json", line=1, column=1))), backend_config=None)


def _load_reader(workspace):
    path = workspace / "adapters/generators/kotlin_rust/driver.py"
    if not path.is_file():
        raise ValueError("C06 declaration reader is not installed in the frozen source closure")
    spec = importlib.util.spec_from_file_location("madb_c06_declaration_reader", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def prepare(repo, workspace, observation, variant):
    """Freeze source bytes, observe real declarations, then resolve v1 input documents."""
    repo, workspace = Path(repo).resolve(strict=True), Path(workspace).resolve()
    if variant not in VARIANTS:
        raise ValueError("unknown C06 variant: " + variant)
    observation = deepcopy(observation)
    _required_trees(observation, variant)
    _verify_python_runtime(observation)
    tools.verify(observation)
    workspace.mkdir(parents=True, exist_ok=False)
    names, schema_names = _copy_sources(repo, workspace)
    author_inputs = [ref(workspace, name, "madb.source-input/1") for name in names]
    _verify_resolution_source(workspace, variant)
    _verify_helper_sources(workspace, observation, variant)
    config = _config(variant, observation)
    save(workspace, "toolchain.json", _public_observation(workspace, observation, config))
    profile = _profile(workspace, observation, config)
    save(workspace, "profile.json", profile)
    selector = dict(route=profile["route"], profile_id=profile["id"], profile_digest=profile_digest(profile), target=profile["target"], direction=profile["direction"])
    project = dict(schema="madb.project/1", project="madb-managed-routes", sdk_namespace="madb",
                   boundaries=[dict(id=variant, route=profile["route"], direction=profile["direction"],
                                    source=dict(kind="kotlin-compiler-export" if variant == "swift-kotlin" else "uniffi-udl", path=config["source"]),
                                    profile=ref(workspace, "profile.json", "madb.profile/1"), target=profile["target"], output="generated")],
                   policies=[], policy_families=[], artifacts=dict(package="madb-c06-" + variant, manifest="candidate.json"))
    save(workspace, "project.json", project)
    reader = _load_reader(workspace)
    # Compiler-produced declarations have a separate explicit derivation receipt.
    # The profile does not contain those binaries, avoiding a profile/build cycle.
    derived = reader.prepare_artifacts(config, workspace, selector, author_inputs) if variant == "swift-kotlin" else []
    names_set = set(names) | set(schema_names) | {"backend.json", "toolchain.json", "profile.json", "project.json"}
    for item in derived:
        if item["path"] in names_set or not item["path"].startswith("derived/"):
            raise ValueError("derived declaration collides with author input")
        if ref(workspace, item["path"]) != {key: item[key] for key in ("path", "sha256", "size")}:
            raise ValueError("derived declaration changed before freezing")
        names_set.add(item["path"])
    if variant == "swift-kotlin":
        project["boundaries"][0]["source"] = dict(kind="compiled-kotlin-export", path=config["source"])
        save(workspace, "project.json", project)
    save(workspace, "backend.json", config)
    inputs = unique(author_inputs + derived + [ref(workspace, name, "madb.source-input/1") for name in ("backend.json", "toolchain.json")])
    preliminary = dict(schema="madb.read-request/1", boundary_id=variant, selector=selector, inputs=inputs)
    scratch = workspace / "prepare-metadata"
    scratch.mkdir()
    seed = reader.metadata_seed(preliminary, config, workspace, scratch)
    project["policies"] = reader.policies(seed)
    save(workspace, "project.json", project)
    component_files = [item for item in inputs if item["path"].startswith("adapters/generators/")]
    component = dict(id="org.madb.managed/reader-generator", version="0.1.0", sha256=semantic("madb.managed-component/1", component_files))
    lock = dict(schema="madb.lock/1", project_digest=project_digest(project),
                profiles=[dict(id=profile["id"], semantic_digest=profile_digest(profile), file=ref(workspace, "profile.json", "madb.profile/1"))],
                tools=profile["tools"], components=[component], inputs=[{key: item[key] for key in ("path", "sha256", "size")} for item in inputs],
                schemas=[ref(workspace, name) for name in schema_names])
    save(workspace, "madb.lock", lock)
    read = dict(schema="madb.read-request/1", boundary_id=variant, selector=selector, inputs=inputs, component=component,
                lock=ref(workspace, "madb.lock", "madb.lock/1"), profile=ref(workspace, "profile.json", "madb.profile/1"), output_root="staging/read")
    save(workspace, "read.json", read)
    tools.verify(observation)
    return dict(profile=profile, project=project, read=read, lock=lock, config=config,
                author_inputs=author_inputs, derived_inputs=derived)
