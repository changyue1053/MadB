"""C04 Rust recipe: real native compile, then ohrs code generation and linking.

These are task responsibilities, not claims that the link phase is linker-only.
No target run, imported SDK binary, compiler cache reuse or binary rewriting.
"""

from __future__ import annotations
import hashlib
import importlib.util
import json
import re
import tomllib
from pathlib import Path
import shutil

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_rust_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)
metadata = support.load_module(Path(__file__).with_name("rust_metadata.py"))
dependencies = support.load_module(Path(__file__).with_name("rust_dependencies.py"))
tools = support.load_module(Path(__file__).with_name("rust_tools.py"))
inventory = support.load_module(Path(__file__).with_name("rust_inventory.py"))


def source_inputs(repo, variant):
    if variant not in ("ordinary", "test"):
        raise ValueError("Rust source variant must be ordinary or test")
    names = [
        *metadata.AUTHOR_FILES,
        "adapters/generators/arkts/rust_frontend.py",
        "adapters/generators/arkts/read_rust_exports.cjs",
        "adapters/toolchains/kotlin_rust/toolchain.json",
        *[
            "adapters/toolchains/arkts/" + n
            for n in (
                "rust.py",
                "rust_metadata.py",
                "rust_dependencies.py",
                "rust_tools.py",
                "rust_inventory.py",
                "rust_observer.py",
                "rust_host.py",
            )
        ],
        "adapters/toolchains/arkts/licenses/napi-ohos-MIT-LICENSE",
        "adapters/toolchains/arkts/licenses/napi-ohos-license-source.json",
    ]
    return [
        dict(
            path=n,
            source=str(support.input_path(repo, n)),
            sha256=support.sha(support.input_path(repo, n)),
            size=support.input_path(repo, n).stat().st_size,
        )
        for n in names
    ]


def _declarations(inputs):
    data = support.read_json(support.input_path(inputs, "declarations.json"))
    if "rust" not in data or "gateway" not in data:
        raise ValueError(
            "Rust Build requires actual checked macro and gateway authorities"
        )
    return data


def run_compile(config, inputs, work, capture=None):
    inputs, work = Path(inputs), Path(work)
    capture = [] if capture is None else capture
    dependencies.verify(inputs, config)
    worker = support.load_module(Path(__file__).with_name("worker.py"))
    outputs = worker.compile_native(config, inputs, work, capture)
    observer = support.load_module(Path(__file__).with_name("rust_observer.py"))
    observed = observer.verify(config, inputs, work, capture)
    archive = work / "objects/libmadb_arkts_gateway.a"
    support.command(
        config,
        "ar",
        ["crsD", archive, *[work / n for n in outputs]],
        cwd=work,
        capture=capture,
    )
    scratch = work / "rust-check"
    scratch.mkdir()
    declared = _declarations(inputs)
    actual = metadata.read_metadata(
        inputs, config, scratch, declared["gateway"], capture=capture
    )
    if actual != declared["rust"]:
        raise ValueError(
            "Rust Compile macro authority differs from the checked Source parent"
        )
    receipt = dict(
        scope="actual C++ objects/archive plus Rust macro/type checking; no runtime execution",
        rust=actual,
        gateway_observer=observed,
        archive_sha256=support.sha(archive),
    )
    (work / "rust-compile.json").write_text(
        json.dumps(receipt, sort_keys=True, indent=2) + "\n"
    )
    shutil.rmtree(scratch)
    dependencies.verify(inputs, config)
    return [*outputs, "objects/libmadb_arkts_gateway.a", "rust-compile.json"]


def _run_ohrs(config, workspace, project, env, capture):
    installed = tools.verify(config["rust"]["ohrs_install"], config)
    tool = support.verify_tool(config["tools"]["ohrs"])
    if installed["binary"]["sha256"] != config["tools"]["ohrs"]["sha256"]:
        raise ValueError(
            "ohrs configured image is not the verified source installation"
        )
    argv = [
        str(tool),
        "build",
        "--arch",
        "arm64",
        "--release",
        "--dist",
        str(project / "dist"),
        "--target-dir",
        env["CARGO_TARGET_DIR"],
        "--soname",
        config["native_module"],
        "--",
        "--offline",
        "--locked",
        "--verbose",
        *metadata.selected_features(config),
    ]
    log = project / "ohrs-build.log"
    # Reuse the existing process-group owner, including quarantine on incomplete
    # cleanup. A killed compiler cannot silently survive a workspace cleanup.
    python = support.verify_tool(config["tools"]["python"])
    launcher = [
        str(python),
        *config["tools"]["python"].get("invocation_arguments", []),
        "-c",
        "import os,sys;os.chdir(sys.argv[1]);os.execv(sys.argv[2],sys.argv[2:])",
        str(project / "author" / metadata.CRATE),
        *argv,
    ]
    code = None
    try:
        # exec keeps the owned process-group leader; only cwd changes. The
        # original compiler bytes/args/status are unmodified and fully recorded.
        code = workspace.execute(launcher, project, env, log)
    finally:
        data = log.read_bytes() if log.exists() else b""
        capture.append(
            dict(
                tool="ohrs",
                tool_sha256=config["tools"]["ohrs"]["sha256"],
                launcher_sha256=config["tools"]["python"]["sha256"],
                arguments=launcher,
                explicit_environment=env,
                exit_code=code,
                combined_stdout_stderr=data.decode(errors="replace"),
            )
        )
    support.verify_tool(config["tools"]["ohrs"])
    tools.verify(config["rust"]["ohrs_install"], config)
    if code:
        raise RuntimeError(
            "ohrs native compile/link failed: " + data.decode(errors="replace")[-8000:]
        )


def compiled_library(project):
    """ohrs preserves Cargo's artifact filename; --soname changes only ELF metadata."""
    manifest = tomllib.loads(
        support.input_path(
            project / "author", metadata.CRATE + "/Cargo.toml"
        ).read_text()
    )
    library = manifest.get("lib", {})
    name = library.get("name")
    if (
        not isinstance(name, str)
        or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) is None
        or library.get("crate-type") != ["cdylib"]
    ):
        raise ValueError("Rust bridge requires one explicitly named Cargo cdylib")
    return support.input_path(project, "dist/arm64-v8a/lib" + name + ".so")


def run_link(config, inputs, work, capture=None):
    inputs, work = Path(inputs), Path(work)
    capture = [] if capture is None else capture
    dependencies.verify(inputs, config)
    archive = support.input_path(inputs, "objects/libmadb_arkts_gateway.a")
    declared = _declarations(inputs)
    selected = config["rust"]["compiler_workspace"]
    if selected.get("strategy") != "input-addressed-exclusive-cold-build":
        raise ValueError(
            "Rust link requires the existing cold compiler workspace strategy"
        )
    workspace = support.load_module(
        _ROOT / "adapters/toolchains/common/compiler_workspace.py"
    )
    names = [
        *metadata.AUTHOR_FILES,
        "objects/libmadb_arkts_gateway.a",
        "declarations.json",
        "identity.json",
        *[item["path"] for item in source_inputs(inputs, "ordinary")],
    ]
    refs = [
        dict(
            path=n,
            size=support.input_path(inputs, n).stat().st_size,
            sha256=support.sha(support.input_path(inputs, n)),
        )
        for n in sorted(set(names))
    ]
    identity = support.read_json(support.input_path(inputs, "identity.json"))
    observation = dict(
        tools=config["tools"],
        trees={
            "rust": config["rust"]["rust_sysroot"],
            "vendor": config["rust"]["dependencies"]["tree"],
            **config["rust"]["host"]["trees"],
        },
        xcode=config["rust"]["host"]["xcode"],
    )
    key = workspace.identity(
        refs, observation, identity["profile_digest"], identity["abi_digest"]
    )
    with workspace.acquire(key, base=Path(selected["root"])) as stage:
        project = stage / "project"
        project.mkdir()
        metadata.copy_author(inputs, project / "author")
        env = metadata.cargo_environment(config, project, metadata=False)
        frozen_archive = project / "libmadb_arkts_gateway.a"
        support.copy_new(archive, frozen_archive)
        env.update(
            OHOS_NDK_HOME=str(Path(config["native_sdk"]).parent),
            MADB_NATIVE_ARCHIVE=str(frozen_archive),
            LC_ALL="C",
            CARGO_RUSTFLAGS="\x1f".join(
                [
                    "-Clink-arg=--target=aarch64-linux-ohos23.0.0",
                    "-Clink-arg=-Wl,--build-id=sha1",
                    "--remap-path-prefix=" + str(project) + "=/madb/rust",
                ]
            ),
        )
        _run_ohrs(config, workspace, project, env, capture)
        emitted = list((project / "target/ohos-rs").rglob(metadata.PACKAGE))
        if len(emitted) != 1:
            raise ValueError("ohrs did not emit unique actual napi macro metadata")
        support.copy_new(emitted[0], project / "macro-output" / metadata.PACKAGE)
        macro = metadata.parse_metadata(config, project, declared["gateway"], capture)
        if macro != declared["rust"]["macro"]:
            raise ValueError("ohrs actual macros differ from checked metadata")
        output = compiled_library(project)
        dynamic = support.command(
            config, "readelf", ["-d", output], cwd=work, capture=capture
        ).decode()
        sonames = re.findall(r"\(SONAME\).*?Library soname: \[([^]]+)\]", dynamic)
        if sonames != [config["native_module"]]:
            raise ValueError("Rust ELF SONAME differs from the declared route module")
        # The delivery filename changes, never the freshly linked ELF bytes.
        support.copy_new(output, work / "native" / config["native_module"])
        (work / "rust-link.json").write_text(
            json.dumps(
                dict(
                    scope="actual ohrs Rust code generation and DSO link, not pure link",
                    compiler_workspace_key=key,
                    actual_macro=macro,
                    source_archive_sha256=support.sha(frozen_archive),
                    backend_artifact=output.relative_to(project).as_posix(),
                    elf_soname=sonames[0],
                ),
                sort_keys=True,
                indent=2,
            )
            + "\n"
        )
    target = work / "native" / config["native_module"]
    symbols = support.command(
        config, "readelf", ["--dyn-syms", "--wide", target], cwd=work, capture=capture
    ).decode()
    defined, imported = [], []
    for line in symbols.splitlines():
        parts = line.split()
        if (
            len(parts) >= 8
            and parts[0].endswith(":")
            and parts[4] in ("GLOBAL", "WEAK")
        ):
            (imported if parts[6] == "UND" else defined).append(parts[7])
    if defined != ["napi_register_module_v1"]:
        raise ValueError("Rust DSO exported symbols beyond its actual napi registrar")
    (work / "rust-symbols.json").write_text(
        json.dumps(
            dict(
                dynamic_defined=defined,
                undefined=sorted(set(imported)),
                frontend=inventory.link_inventory(
                    imported, declared["rust"]["frontend_evidence"]
                ),
            ),
            sort_keys=True,
            indent=2,
        )
        + "\n"
    )
    support.verify_refs(inputs, refs)
    dependencies.verify(inputs, config)
    return ["native/" + config["native_module"], "rust-link.json", "rust-symbols.json"]


def compose_native(repo, output, variant, gateway_descriptor):
    """Derive the same reviewed gateway; never copy a second runtime/provider."""
    native = support.load_module(Path(__file__).with_name("native.py"))
    base = native.compose_native(
        Path(repo), Path(output), route="rust", variant=variant
    )
    return derive_native(base, output, variant, gateway_descriptor)


def derive_native(base, output, variant, gateway_descriptor):
    """Use the one already-composed source/actual declaration as the raw input."""
    if variant not in ("ordinary", "test"):
        raise ValueError("Rust raw derivation requires an explicit variant")
    native = support.load_module(Path(__file__).with_name("native.py"))
    frontend = support.load_module(_ROOT / "adapters/generators/arkts/rust_frontend.py")
    if base.provider_factory != "mp_rust_create":
        raise ValueError("Rust composition must bind the real Rust provider")
    api = next(u.path for u in base.units if u.path.name == "api.cpp")
    raw_root = Path(output) / "rust-raw"
    raw_root.mkdir(exist_ok=False)
    rendered = {
        "api.cpp": frontend.raw_api(
            api.read_text(), test_support=variant == "test"
        ).encode(),
        **frontend.render_bridge(gateway_descriptor, test_support=variant == "test"),
    }
    derived = []
    for name, data in rendered.items():
        path = raw_root / name
        path.write_bytes(data)
        derived.append(
            native.FileIdentity(path, hashlib.sha256(data).hexdigest(), len(data))
        )
    units = tuple(
        native.NativeUnit(raw_root / "api.cpp", u.language) if u.path == api else u
        for u in base.units
        if u.language != "rust"
    )
    units += (native.NativeUnit(raw_root / "runtime_ffi.cpp", "c++"),)
    return native.NativeComposition(
        units,
        (*base.include_dirs, raw_root),
        base.native_declaration,
        base.facade_template,
        base.source_inputs,
        (*base.derived_files, *derived),
        base.provider_factory,
        "napi-ohos-macro-only",
    )


def prepare_configuration(base_observation, reviewed_config, native_module, variant):
    """Add one verified Rust recipe to the existing observed C04 toolchain.

    Returns observation plus explicit source/delivery refs for project freezing.
    Does not create a plan, run a compiler, or silently acquire dependencies.
    """
    from copy import deepcopy

    if variant not in ("ordinary", "test"):
        raise ValueError("unsupported Rust route variant")
    expected_module = (
        "libmadbc07sdkrust" + ("test" if variant == "test" else "") + ".so"
    )
    if native_module != expected_module:
        raise ValueError("Rust module identity differs from the selected route")
    if set(reviewed_config) != {"tools", "rust"}:
        raise ValueError(
            "Rust preparation accepts only reviewed tools and Rust closure"
        )
    observation = deepcopy(base_observation)
    config = observation["config"]
    requested = reviewed_config["tools"]
    if set(requested) != {"cargo", "rustc", "node", "typescript", "ohrs"}:
        raise ValueError("unexpected or missing Rust tool configuration")
    for name, tool in requested.items():
        support.verify_tool(tool)
        if name in config["tools"]:
            if any(config["tools"][name][k] != tool[k] for k in ("sha256", "size")):
                raise ValueError("Rust and base tool observations disagree: " + name)
            # Preserve the base archive provenance and original installed path.
        else:
            config["tools"][name] = deepcopy(tool)
    config["rust"] = deepcopy(reviewed_config["rust"])
    config["rust"]["test_support"] = variant == "test"
    if (
        config["rust"].get("crate"),
        config["rust"].get("provider"),
        config["rust"].get("feature_policy"),
    ) != (
        metadata.CRATE,
        metadata.PROVIDER,
        metadata.CRATE + "/frontend-features.json",
    ):
        raise ValueError("Rust route must use the unique bridge and actual provider")
    if any(config["tools"][n]["version"] != "1.99.1" for n in ("cargo", "rustc")):
        raise ValueError("Rust route requires the reviewed 1.99.1 toolchain")
    installed = tools.verify(config["rust"]["ohrs_install"], config)
    if installed["binary"]["sha256"] != config["tools"]["ohrs"]["sha256"]:
        raise ValueError("Rust ohrs image is not the original-source build")
    dependencies.verify(_ROOT, config)
    inventory_ref = observation["inventory"]
    inventory_path = Path(inventory_ref["path"])
    if (
        support.sha(inventory_path) != inventory_ref["sha256"]
        or inventory_path.stat().st_size != inventory_ref["size"]
    ):
        raise ValueError("base CLI inventory changed before Rust preparation")
    cli = Path(config["cli_root"]).resolve(strict=True)
    ar = (Path(config["native_sdk"]) / "llvm/bin/llvm-ar").resolve(strict=True)
    if not ar.is_relative_to(cli):
        raise ValueError("Rust archive tool resolved outside the verified Native SDK")
    entry = [
        e
        for e in support.read_json(inventory_path, 64 * support.MAX_JSON)["entries"]
        if e["path"] == ar.relative_to(cli).as_posix() and e["kind"] == "file"
    ]
    if (
        len(entry) != 1
        or support.sha(ar) != "sha256:" + entry[0]["sha256"]
        or ar.stat().st_size != entry[0]["size"]
    ):
        raise ValueError("Rust archive tool differs from original CLI inventory")
    config["tools"]["ar"] = dict(
        path=str(ar),
        sha256=support.sha(ar),
        size=ar.stat().st_size,
        id="c07-ar",
        version=config["tools"]["clang"]["version"],
        source=config["tools"]["clang"]["source"],
    )
    for name, image in config["rust"]["host"]["tools"].items():
        config["tools"]["rust_host_" + name] = {**image, "id": "c07-rust-host-" + name}
    public = observation["public"]
    public["tools"] = [
        {k: t[k] for k in ("id", "version", "sha256", "size", "source")}
        for _, t in sorted(config["tools"].items())
    ]
    public["rust"] = dict(
        version="1.99.1",
        napi_frontend="1.2.0/napi3",
        napi_sys="1.2.0/napi8",
        ohrs="1.5.0",
        dependency_receipt=config["rust"]["dependencies"]["receipt"]["sha256"],
        ohrs_receipt=config["rust"]["ohrs_install"]["receipt"]["sha256"],
        sysroot={
            k: v for k, v in config["rust"]["rust_sysroot"].items() if k != "root"
        },
        test_support=variant == "test",
        host_xcode=config["rust"]["host"]["xcode"],
        host_trees={
            name: {k: v for k, v in tree.items() if k != "root"}
            for name, tree in config["rust"]["host"]["trees"].items()
        },
    )
    receipts = []
    for name, selected in (
        ("cargo-vendor", config["rust"]["dependencies"]),
        ("ohrs-source", config["rust"]["ohrs_install"]),
    ):
        ref = selected["receipt"]
        receipts.append(
            dict(
                path="third-party/rust/" + name + "-receipt.json",
                source=ref["path"],
                sha256=ref["sha256"],
                size=ref["size"],
            )
        )
    return dict(
        observation=observation,
        source_inputs=source_inputs(_ROOT, variant),
        notice_inputs=dependencies.notice_inputs(_ROOT, config),
        receipt_inputs=receipts,
    )
