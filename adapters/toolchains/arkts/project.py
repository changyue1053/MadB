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
            source.stat().st_size != expected["size"] or support.sha(source) != expected["sha256"]
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
            e for e in inventory_data["entries"] if e["path"] == original and e["kind"] == "file"
        ]
        if len(entries) != 1:
            raise ValueError("Native SDK notice missing from verified archive inventory")
        expected = {"sha256": "sha256:" + entries[0]["sha256"], "size": entries[0]["size"]}
        logical = "third-party/harmony/" + name
        copy(cli / original, logical, expected)
        runtime_notices.append({"source": logical, "sdk_path": original})
    if route not in ("c", "cpp", "rust") or variant not in ("ordinary", "test"):
        raise ValueError("unsupported native source profile")
    suffix = "test" if variant == "test" else ""
    package_name = "madbc07sdk" + route + suffix
    boundary = route + "-arkts"
    config = deepcopy(observation["config"])
    config["runtime_notices"] = runtime_notices
    config["delivery_inputs"] = list(delivery_inputs)
    if variant == "test":
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
            "derived/native/Boundary.ets.in" if composition else "sdk/arkts/facade/Boundary.ets.in"
        ),
        public_source="derived/Boundary.declarations.ets",
        runtime_frame="sdk/arkts/facade/RuntimeFrame.ets",
        frame_layout="sdk/arkts/facade/frame-layout.json",
        gateway_source=(
            "derived/native/Index.d.ts" if composition else "sdk/arkts/native-gateway/Index.d.ts"
        ),
        provider_source="sdk/arkts/native-runtime/include/provider_abi.h",
        cli_inventory="tools/cli-inventory.json",
        native_module="lib" + package_name + ".so",
        phase_timeout_seconds=900,
        phase_termination_grace_ms=6000,
        build=deepcopy(build_recipe),
    )
    driver = support.load_module(workspace / "adapters/generators/arkts/driver.py")
    config["features"] = driver.lowering.FEATURES
    projection = driver.render_template(
        (workspace / config["public_template"]).read_bytes(),
        config["native_module"],
        "madb-declaration-projection-not-runtime",
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
        route="ARKTS-" + route.upper() + "-NAPI",
        provider={"language": route, "runtime": "native"},
        consumer={"language": "arkts", "runtime": "harmonyos"},
        direction="consumer-to-provider",
        target="aarch64-linux-ohos",
        abi="harmonyos-napi8-c07-snapshot",
        artifact_kind="har",
        build_system="clang-hvigor",
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
    inputs = [d.ref(workspace, n, "madb.source-input/1") for n in sorted(set(names) - set(schemas))]
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
    return {"config": config, "read": read, "lock": lock, "profile": profile, "project": project}


def prepare_route(
    repo,
    workspace,
    observation,
    *,
    route="c",
    variant="ordinary",
    author_fixture=None,
    delivery_inputs=(),
):
    """Freeze original sources, then replay the exact ordinary/test derivation."""
    repo = Path(repo).resolve(strict=True)
    native = support.load_module(repo / "adapters/toolchains/arkts/native.py")
    paths = native.source_paths(repo, route=route, variant=variant)
    if author_fixture is not None:
        author_fixture = Path(author_fixture).absolute()
        if variant != "ordinary" or author_fixture.is_symlink():
            raise ValueError("author fixture requires an ordinary source directory")
        if not author_fixture.is_dir() or any(p.is_symlink() for p in author_fixture.parents):
            raise ValueError("author fixture path must not traverse symlinks")
        if {p.name for p in author_fixture.iterdir()} != {"Boundary.ets.in", "runtime-policy.json"}:
            raise ValueError("author fixture must contain exactly the source template and policy")
        for name in ("Boundary.ets.in", "runtime-policy.json"):
            support.input_path(author_fixture, name)
    with tempfile.TemporaryDirectory(prefix="madb-c07-source-") as temporary:
        temp = Path(temporary).resolve()
        seed = temp / "inputs"
        seed.mkdir()
        source_refs = []
        for path in paths:
            logical = path.relative_to(repo).as_posix()
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
            "system_libraries": ["libace_napi.z.so", "libuv.so", "libc.so", "libm.so", "libdl.so"],
            "symbol_policy": "constructor-only; all defined dynamic symbols local",
        }
        composition = {
            "route": route,
            "variant": variant,
            "prefix": "derived/native",
            "original_inputs": [ref["path"] for ref in source_refs],
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
