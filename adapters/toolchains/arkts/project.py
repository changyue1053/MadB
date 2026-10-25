"""Freeze restricted native route authors, tools, policy and C04 documents."""

from __future__ import annotations
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile

_spec = importlib.util.spec_from_file_location(
    "_c07_port", Path(__file__).resolve().parents[3] / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)
LOCAL = Path(__file__).resolve().parent
HELPERS = (
    "adapters/generators/arkts/driver.py",
    "adapters/generators/arkts/support.py",
    "adapters/generators/arkts/read_arkts.cjs",
    "adapters/generators/arkts/read_facade.cjs",
    "adapters/generators/arkts/read_har_exports.cjs",
    "adapters/generators/arkts/read_c_seam.py",
    "adapters/generators/arkts/lowering.py",
    "adapters/toolchains/arkts/toolchain.py",
    "adapters/toolchains/arkts/worker.py",
    "adapters/artifacts/arkts/pack.py",
    "adapters/artifacts/arkts/har_inputs.py",
    "adapters/artifacts/arkts/har_public.py",
)
SHARED = (
    "scripts/build_arkts_routes.py",
    "adapters/toolchains/arkts/project.py",
    "adapters/toolchains/arkts/pipeline.py",
    "adapters/toolchains/arkts/requirements.py",
    "adapters/toolchains/common/documents.py",
    "adapters/toolchains/common/pipeline.py",
    "adapters/toolchains/common/compiler_workspace.py",
    "adapters/generators/rust_c/support.py",
    "adapters/generators/rust_c/native_ast.py",
    "adapters/toolchains/arkts/tools.py",
    "adapters/toolchains/arkts/preflight.py",
    "adapters/artifacts/arkts/har.py",
    "adapters/toolchains/arkts/inventory-format.json",
    "adapters/toolchains/kotlin_rust/tools.py",
    "adapters/toolchains/kotlin_rust/install_tools.py",
    "adapters/toolchains/kotlin_rust/python_runtime.py",
    "LICENSE",
    "NOTICE",
)


def prepare(
    repo,
    workspace,
    author_files,
    observation,
    build_recipe,
    *,
    route="c",
    variant="ordinary",
    composition=None,
    policy_source=None,
    delivery_inputs=(),
    additional_notices=(),
):
    """author_files: exact {path,source,sha256,size} refs, never directory globbing.

    The caller supplies stable source review pins. Source paths are copied only
    after matching those ORIGINAL pins. Existing/prebuilt .so is never accepted.
    """
    repo = Path(repo).resolve(strict=True)
    workspace = Path(workspace)
    if workspace.exists() or workspace.is_symlink():
        raise ValueError("C04 project root must be new")
    workspace.mkdir(parents=True)
    d = support.load_module(repo / "adapters/toolchains/common/documents.py")
    names = []

    def copy(source, name, expected=None):
        if name in names:
            raise ValueError("duplicate author logical destination")
        if (
            not name
            or Path(name).is_absolute()
            or any(p in ("", ".", "..") for p in name.split("/"))
        ):
            raise ValueError("invalid author logical path")
        source = Path(source)
        if source.is_symlink() or not source.is_file():
            raise ValueError("author must be an ordinary source file")
        if source.suffix in (".so", ".o", ".a", ".har", ".hap", ".hsp", ".klib"):
            raise ValueError("prebuilt artifact is not an author declaration/implementation")
        if expected and (
            source.stat().st_size != expected["size"]
            or support.sha(source) != expected["sha256"]
        ):
            raise ValueError("original author input changed: " + name)
        support.copy_new(source, workspace / name)
        if expected:
            copied = workspace / name
            if any(
                p.stat().st_size != expected["size"] or support.sha(p) != expected["sha256"]
                for p in (source, copied)
            ):
                raise ValueError("author changed while being frozen: " + name)
        names.append(name)

    for entry in author_files:
        copy(entry["source"], entry["path"], entry)
    for name in SHARED:
        if name not in names:
            copy(repo / name, name)
    for name in HELPERS:
        if name not in names:
            copy(repo / name, name)
    schemas = sorted(
        p.relative_to(repo).as_posix() for p in (repo / "docs/schemas").glob("*.schema.json")
    )
    for name in schemas:
        copy(repo / name, name)
    copy(
        policy_source or repo / "adapters/generators/arkts/profiles/snapshot-v1.json",
        "runtime-policy.json",
    )
    inventory = observation["inventory"]
    copy(inventory["path"], "tools/cli-inventory.json", inventory)
    inventory_data = support.read_json(inventory["path"], 64 * support.MAX_JSON)
    cli = Path(observation["config"]["cli_root"])
    runtime_notices = []
    for original, name in [
        ("sdk/default/openharmony/native/NOTICE.txt", "native-NOTICE.txt"),
        ("sdk/default/openharmony/native/llvm/NOTICE", "llvm-NOTICE.txt"),
    ]:
        entries = [
            e
            for e in inventory_data["entries"]
            if e["path"] == original and e["kind"] == "file"
        ]
        if len(entries) != 1:
            raise ValueError("Native SDK notice missing from verified archive inventory")
        expected = {"sha256": "sha256:" + entries[0]["sha256"], "size": entries[0]["size"]}
        logical = "third-party/harmony/" + name
        copy(cli / original, logical, expected)
        runtime_notices.append({"source": logical, "sdk_path": original})
    if route not in ("c", "cpp", "rust", "kotlin") or variant not in ("ordinary", "test"):
        raise ValueError("unsupported native source profile")
    suffix = "test" if variant == "test" else ""
    package_name = "madbc07sdk" + route + suffix
    boundary = route + "-arkts"
    config = deepcopy(observation["config"])
    config["runtime_notices"] = runtime_notices
    config["additional_notices"] = list(additional_notices)
    config["delivery_inputs"] = list(delivery_inputs)
    if variant == "test" and route != "kotlin":
        config["test_contracts"] = sorted(
            n for n in names if n.startswith("sdk/arkts/test-support/contracts/")
        )
        config["test_facade_inputs"] = [
            "sdk/arkts/test-support/facade/" + n for n in ("TestFrame.ets", "FacadeFrame.ets")
        ]
    config.update(
        provider=route,
        variant=variant,
        package_name=package_name,
        source_package=package_name,
        provider_factory="mp_" + route + "_create",
        composition=composition,
    )
    config.update(
        schema="madb.private.c07-native-backend/1",
        boundary_id=boundary,
        project="project.json",
        profile="profile.json",
        policy="runtime-policy.json",
        public_template=(
            "derived/native/Boundary.ets.in"
            if composition
            else "sdk/arkts/facade/Boundary.ets.in"
        ),
        public_source="derived/Boundary.declarations.ets",
        runtime_frame="sdk/arkts/facade/RuntimeFrame.ets",
        frame_layout="sdk/arkts/facade/frame-layout.json",
        gateway_source=(
            "derived/native/Index.d.ts"
            if composition
            else "sdk/arkts/native-gateway/Index.d.ts"
        ),
        provider_source="sdk/arkts/native-runtime/include/provider_abi.h",
        cli_inventory="tools/cli-inventory.json",
        native_module="lib" + package_name + ".so",
        phase_timeout_seconds=900,
        phase_termination_grace_ms=6000,
        build=deepcopy(build_recipe),
    )
    if route == "kotlin":
        config.update(
            provider_factory=None,
            gateway_source=None,
            provider_source=None,
            runtime_frame=None,
            frame_layout=None,
        )
    driver = support.load_module(workspace / "adapters/generators/arkts/driver.py")
    config["features"] = driver.lowering.FEATURES
    projection = driver.public_projection(
        workspace, config, "madb-declaration-projection-not-runtime"
    )
    (workspace / "derived").mkdir(exist_ok=True)
    (workspace / config["public_source"]).write_bytes(projection)
    names.append(config["public_source"])
    toolchain = deepcopy(observation["public"])
    toolchain["schema"] = "madb.native-toolchain/1"
    d.save(workspace, "toolchain.json", toolchain)
    d.save(workspace, "backend.json", config)
    names += ["toolchain.json", "backend.json"]
    public_tools = [
        {
            **{k: t[k] for k in ("id", "version", "sha256")},
            "host": "aarch64-apple-darwin",
            "target": "aarch64-linux-ohos",
            "source": t["source"],
            "arguments": t.get("invocation_arguments", []),
        }
        for _, t in sorted(config["tools"].items())
    ]
    profile = dict(
        schema="madb.profile/1",
        id="c07-" + route + "-arkts-aarch64-ohos23" + ("-test" if variant == "test" else ""),
        route="KN-ARKTS-NAPI" if route == "kotlin" else "ARKTS-" + route.upper() + "-NAPI",
        provider={
            "language": route,
            "runtime": "kotlin-native" if route == "kotlin" else "native",
        },
        consumer={"language": "arkts", "runtime": "harmonyos"},
        direction="consumer-to-provider",
        target="aarch64-linux-ohos",
        abi="harmonyos-napi8-c07-snapshot",
        artifact_kind="har",
        build_system={"rust": "cargo-ohrs-hvigor", "kotlin": "cpf-gradle-hvigor"}.get(
            route, "clang-hvigor"
        ),
        deployment_target="API23",
        tools=public_tools,
        sdk=d.ref(workspace, "toolchain.json", "madb.native-toolchain/1"),
        macros=[],
        required_features=config["features"],
        supported_features=config["features"],
        restrictions=[],
        defaults={
            "values": {"ownership": "copy"},
            "origin": {
                "authority": "profile-default",
                "id": "c07.copy-values",
                "location": {"path": "profile.json", "line": 1, "column": 1},
            },
        },
        backend_config=None,
    )
    d.save(workspace, "profile.json", profile)
    selector = {k: profile[k] for k in ("route", "target", "direction")}
    selector.update(profile_id=profile["id"], profile_digest=d.profile_digest(profile))
    inputs = [
        d.ref(workspace, n, "madb.source-input/1") for n in sorted(set(names) - set(schemas))
    ]
    preliminary = {
        "schema": "madb.read-request/1",
        "boundary_id": boundary,
        "selector": selector,
        "inputs": inputs,
    }
    scratch = workspace / "prepare-metadata"
    scratch.mkdir()
    _, seed, policies = driver._read(preliminary, config, workspace, scratch)
    project = dict(
        schema="madb.project/1",
        project="madb-c07-" + boundary,
        sdk_namespace="madb",
        boundaries=[
            dict(
                id=boundary,
                route=profile["route"],
                direction=profile["direction"],
                source={"kind": "arkts-compiler-declarations", "path": config["public_source"]},
                profile=d.ref(workspace, "profile.json", "madb.profile/1"),
                target=profile["target"],
                output="generated",
            )
        ],
        policies=policies,
        policy_families=[],
        artifacts={"package": package_name, "manifest": "candidate.json"},
    )
    d.save(workspace, "project.json", project)
    component = dict(
        id="org.madb.arkts/reader-generator",
        version="0.1.0",
        sha256=d.semantic(
            "madb.arkts-component/1",
            [r for r in inputs if r["path"].startswith("adapters/generators/")],
        ),
    )
    lock = dict(
        schema="madb.lock/1",
        project_digest=d.project_digest(project),
        profiles=[
            dict(
                id=profile["id"],
                semantic_digest=d.profile_digest(profile),
                file=d.ref(workspace, "profile.json", "madb.profile/1"),
            )
        ],
        tools=profile["tools"],
        components=[component],
        inputs=[{k: r[k] for k in ("path", "sha256", "size")} for r in inputs],
        schemas=[d.ref(workspace, n) for n in schemas],
    )
    d.save(workspace, "madb.lock", lock)
    read = dict(
        preliminary,
        component=component,
        lock=d.ref(workspace, "madb.lock", "madb.lock/1"),
        profile=d.ref(workspace, "profile.json", "madb.profile/1"),
        output_root="staging/read",
    )
    d.save(workspace, "read.json", read)
    return {
        "config": config,
        "read": read,
        "lock": lock,
        "profile": profile,
        "project": project,
    }


def prepare_route(
    repo,
    workspace,
    observation,
    *,
    route="c",
    variant="ordinary",
    author_fixture=None,
    delivery_inputs=(),
    rust_configuration=None,
    kotlin_configuration=None,
    observation_path=None,
):
    """Freeze original sources, then replay the exact ordinary/test derivation."""
    repo = Path(repo).resolve(strict=True)
    if route == "kotlin":
        if rust_configuration is not None:
            raise ValueError("Rust configuration supplied to Kotlin")
        return prepare_kotlin(
            repo,
            workspace,
            observation,
            variant,
            kotlin_configuration,
            author_fixture,
            delivery_inputs,
            observation_path,
        )
    if kotlin_configuration is not None:
        raise ValueError("Kotlin configuration supplied to another route")
    native = support.load_module(repo / "adapters/toolchains/arkts/native.py")
    paths = native.source_paths(repo, route=route, variant=variant)
    backend_inputs = {"source_inputs": [], "notice_inputs": [], "receipt_inputs": []}
    if route == "rust":
        if rust_configuration is None:
            raise ValueError("Rust route requires caller-pinned configuration")
        source = Path(rust_configuration["source"])
        if (
            support.sha(source) != rust_configuration["sha256"]
            or source.stat().st_size != rust_configuration["size"]
        ):
            raise ValueError("Rust configuration changed before preparation")
        backend = support.load_module(repo / "adapters/toolchains/arkts/rust.py")
        native_module = "libmadbc07sdkrust" + ("test" if variant == "test" else "") + ".so"
        backend_inputs = backend.prepare_configuration(
            observation, support.read_json(source), native_module, variant
        )
        observation = backend_inputs["observation"]
    elif rust_configuration is not None:
        raise ValueError("Rust configuration supplied to another route")
    if observation_path is not None:
        with Path(observation_path).open("x") as output:
            json.dump(observation, output, indent=2)
            output.write("\n")
    if author_fixture is not None:
        author_fixture = Path(author_fixture).absolute()
        if variant != "ordinary" or author_fixture.is_symlink():
            raise ValueError("author fixture requires an ordinary source directory")
        if not author_fixture.is_dir() or any(p.is_symlink() for p in author_fixture.parents):
            raise ValueError("author fixture path must not traverse symlinks")
        if {p.name for p in author_fixture.iterdir()} != {
            "Boundary.ets.in",
            "runtime-policy.json",
        }:
            raise ValueError(
                "author fixture must contain exactly the source template and policy"
            )
        for name in ("Boundary.ets.in", "runtime-policy.json"):
            support.input_path(author_fixture, name)
    with tempfile.TemporaryDirectory(prefix="madb-c07-source-") as temporary:
        temp = Path(temporary).resolve()
        seed = temp / "inputs"
        seed.mkdir()
        source_refs = []
        originals = {path.relative_to(repo).as_posix(): path for path in paths}
        frozen_sources = dict(originals)
        if route == "rust":
            for ref in backend_inputs["source_inputs"]:
                path = support.input_path(repo, ref["path"])
                if (
                    path != Path(ref["source"])
                    or support.sha(path) != ref["sha256"]
                    or path.stat().st_size != ref["size"]
                ):
                    raise ValueError("Rust source input differs from its repository pin")
                if ref["path"] in frozen_sources and frozen_sources[ref["path"]] != path:
                    raise ValueError("conflicting Rust/native source input")
                frozen_sources[ref["path"]] = path
            for name in (*SHARED, *HELPERS):
                frozen_sources.setdefault(name, support.input_path(repo, name))
        for logical, path in frozen_sources.items():
            source = path
            if author_fixture is not None and logical == "sdk/arkts/facade/Boundary.ets.in":
                source = Path(author_fixture) / "Boundary.ets.in"
            if source.is_symlink() or not source.is_file():
                raise ValueError("author source is not an ordinary file")
            expected = {
                "path": logical,
                "sha256": support.sha(source),
                "size": source.stat().st_size,
            }
            support.copy_new(source, seed / logical)
            support.verify_refs(seed, [expected])
            if support.sha(source) != expected["sha256"]:
                raise ValueError("author changed during initial freeze")
            source_refs.append(expected)
        frozen_native = support.load_module(seed / "adapters/toolchains/arkts/native.py")
        composed = temp / "composed"
        value = frozen_native.compose_native(seed, composed, route=route, variant=variant)
        if route == "rust":
            driver = support.load_module(seed / "adapters/generators/arkts/driver.py")
            declaration = value.native_declaration
            gateway = driver.read_gateway(
                observation["config"],
                composed,
                {
                    "path": declaration.relative_to(composed).as_posix(),
                    "sha256": support.sha(declaration),
                    "size": declaration.stat().st_size,
                },
                temp,
            )
            frozen_backend = support.load_module(seed / "adapters/toolchains/arkts/rust.py")
            value = frozen_backend.derive_native(value, composed, variant, gateway)

        def logical(path):
            if path.is_relative_to(seed):
                return path.relative_to(seed).as_posix()
            if path.is_relative_to(composed):
                return "derived/native/" + path.relative_to(composed).as_posix()
            raise ValueError("native composition returned a file outside its frozen roots")

        authors = [
            {
                "path": ref["path"],
                "source": str(seed / ref["path"]),
                "sha256": ref["sha256"],
                "size": ref["size"],
            }
            for ref in source_refs
        ]
        authors += [
            {
                "path": logical(f.path),
                "source": str(f.path),
                "sha256": "sha256:" + f.sha256,
                "size": f.size,
            }
            for f in value.derived_files
        ]
        policy_path = (
            Path(author_fixture) / "runtime-policy.json"
            if author_fixture is not None
            else repo
            / (
                "adapters/generators/arkts/profiles/snapshot-test-v1.json"
                if variant == "test"
                else "adapters/generators/arkts/profiles/snapshot-v1.json"
            )
        )
        authority = support.read_json(policy_path)["authority"]
        authors.append(
            {
                "path": authority["path"],
                "source": str(repo / authority["path"]),
                "sha256": authority["sha256"],
                "size": authority["size"],
            }
        )
        authors.extend(delivery_inputs)
        authors.extend(backend_inputs["notice_inputs"])
        authors.extend(backend_inputs["receipt_inputs"])
        if rust_configuration is not None:
            authors.append(rust_configuration)
        units = [
            dict(
                name=u.path.stem.replace("_", "-"),
                source=logical(u.path),
                language="cpp" if u.language == "c++" else u.language,
            )
            for u in value.units
            if u.language != "rust"
        ]
        recipe = {
            "units": units,
            "includes": [logical(d) for d in value.include_dirs] + ["generated/native"],
            "strip": False,
            "runtime_libraries": [observation["native_runtime"]],
            "system_libraries": [
                "libace_napi.z.so",
                "libuv.so",
                "libc.so",
                "libm.so",
                "libdl.so",
            ],
            "symbol_policy": (
                "napi-ohos-macro-only; napi_register_module_v1 is the sole dynamic export"
                if route == "rust"
                else "constructor-only; all defined dynamic symbols local"
            ),
        }
        composition = {
            "route": route,
            "variant": variant,
            "prefix": "derived/native",
            "original_inputs": list(originals),
            "derived_inputs": [logical(f.path) for f in value.derived_files],
        }
        prepared = prepare(
            repo,
            workspace,
            authors,
            observation,
            recipe,
            route=route,
            variant=variant,
            composition=composition,
            policy_source=policy_path,
            delivery_inputs=[f["path"] for f in delivery_inputs],
            additional_notices=[f["path"] for f in backend_inputs["notice_inputs"]],
        )
        producer_paths = {ref["path"]: ref["path"] for ref in source_refs}
        producer_paths.update({name: name for name in (*SHARED, *HELPERS)})
        producer_paths.update({r["path"]: r["path"] for r in prepared["lock"]["schemas"]})
        producer_paths[authority["path"]] = authority["path"]
        if author_fixture is None:
            producer_paths["runtime-policy.json"] = policy_path.relative_to(repo).as_posix()
        else:
            producer_paths.pop("sdk/arkts/facade/Boundary.ets.in")
        prepared["producer_inputs"] = producer_paths
        return prepared


def prepare_kotlin(
    repo,
    workspace,
    observation,
    variant,
    configuration,
    author_fixture,
    delivery_inputs,
    observation_path,
):
    """KSP owns Kotlin declarations; there is no synthetic C native composition."""
    if variant != "ordinary":
        raise ValueError(
            "Kotlin companion requires its independently completed test-control policy"
        )
    if configuration is None:
        raise ValueError("Kotlin route requires caller-pinned configuration")
    source = Path(configuration["source"])
    if (
        support.sha(source) != configuration["sha256"]
        or source.stat().st_size != configuration["size"]
    ):
        raise ValueError("Kotlin configuration changed before preparation")
    backend = support.load_module(repo / "adapters/toolchains/arkts/kotlin.py")
    selected = backend.prepare_configuration(
        observation, support.read_json(source), "libmadbc07sdkkotlin.so", variant
    )
    selected["observation"]["public"]["kotlin"]["execution_domain"] = "main-env-only"
    authors = [
        *selected["source_inputs"],
        *selected["notice_inputs"],
        *selected["receipt_inputs"],
        configuration,
        *delivery_inputs,
    ]
    policy_path = repo / "adapters/generators/arkts/profiles/snapshot-kotlin-v1.json"
    authority = support.read_json(policy_path)["authority"]
    authority_ref = dict(authority, source=str(repo / authority["path"]))
    existing = [ref for ref in authors if ref["path"] == authority["path"]]
    if existing:
        if any(existing[0][key] != authority[key] for key in ("sha256", "size")):
            raise ValueError("conflicting Kotlin/public policy authority")
    else:
        authors.append(authority_ref)
    elf = support.load_module(repo / "adapters/toolchains/arkts/kotlin_elf.py")
    recipe = {
        "strip": False,
        "runtime_libraries": [selected["observation"]["native_runtime"]],
        "implementation_libraries": [
            "libmadbc07sdkkotlin.so",
            "libmadb_kotlin_owned_runtime.so",
        ],
        "system_libraries": sorted(elf.SYSTEM),
        "symbol_policy": "CPF managed provider and independently checked static bootstrap",
    }
    overridden = set()
    if author_fixture is not None:
        fixture = Path(author_fixture).absolute()
        if fixture.is_symlink() or any(parent.is_symlink() for parent in fixture.parents):
            raise ValueError("Kotlin author fixture path must not traverse symlinks")
        if not fixture.is_dir() or {p.name for p in fixture.iterdir()} != {
            "BoundaryTypes.kt",
            "OwnedExports.kt",
            "Boundary.ets.in",
            "runtime-policy.json",
        }:
            raise ValueError(
                "Kotlin author fixture requires exactly Kotlin types/exports, public source and role policy"
            )
        mapping = {
            "sdk/kotlin/arkts-owned/src/common/BoundaryTypes.kt": "BoundaryTypes.kt",
            "sdk/kotlin/arkts-owned/src/common/OwnedExports.kt": "OwnedExports.kt",
            "sdk/arkts/facade/Boundary.ets.in": "Boundary.ets.in",
        }
        for ref in authors:
            if ref["path"] in mapping:
                source = support.input_path(fixture, mapping[ref["path"]])
                ref.update(
                    source=str(source), sha256=support.sha(source), size=source.stat().st_size
                )
                overridden.add(ref["path"])
        if overridden != set(mapping):
            raise ValueError("Kotlin author fixture did not replace its exact declared sources")
        for ref in selected["observation"]["config"]["kotlin"]["source_inputs"]:
            match = next((a for a in authors if a["path"] == ref["path"]), None)
            if match is not None:
                ref.update(sha256=match["sha256"], size=match["size"])
        policy_path = support.input_path(fixture, "runtime-policy.json")
    if observation_path is not None:
        with Path(observation_path).open("x") as output:
            json.dump(selected["observation"], output, indent=2)
            output.write("\n")
    prepared = prepare(
        repo,
        workspace,
        authors,
        selected["observation"],
        recipe,
        route="kotlin",
        variant=variant,
        policy_source=policy_path,
        delivery_inputs=[f["path"] for f in delivery_inputs],
        additional_notices=[f["path"] for f in selected["notice_inputs"]],
    )
    producer_paths = {name: name for name in (*SHARED, *HELPERS)}
    for entry in selected["source_inputs"]:
        source_path = Path(entry["source"])
        if source_path.is_relative_to(repo):
            producer_paths[entry["path"]] = source_path.relative_to(repo).as_posix()
    producer_paths.update({ref["path"]: ref["path"] for ref in prepared["lock"]["schemas"]})
    producer_paths[authority["path"]] = authority["path"]
    if author_fixture is None:
        producer_paths["runtime-policy.json"] = policy_path.relative_to(repo).as_posix()
    for name in overridden:
        producer_paths.pop(name, None)
    prepared["producer_inputs"] = producer_paths
    return prepared
