"""C04 Kotlin backend: real KSP declaration authority and the sole managed owner."""

from __future__ import annotations
from copy import deepcopy
import json
from pathlib import Path
import re
import shutil
from . import kotlin_dependencies as dependencies
from . import kotlin_project as project
from . import kotlin_metadata as metadata
from . import kotlin_elf

support = dependencies.support
_ROOT = Path(__file__).resolve().parents[3]


def source_inputs(repo, variant):
    if variant not in ("ordinary", "test"):
        raise ValueError("Unsupported Kotlin source variant")
    repo = Path(repo)
    names = [
        dependencies.SOURCE_PIN,
        "docs/adr/0011-arkts-native-protocol-owner.md",
        "docs/adr/0013-arkts-runtime-and-completion.md",
        dependencies.PATCH,
        "adapters/generators/arkts/read_kotlin_exports.cjs",
        "adapters/generators/arkts/read_kotlin_public.cjs",
        *[
            "adapters/toolchains/arkts/" + n
            for n in (
                "kotlin.py",
                "kotlin_dependencies.py",
                "kotlin_project.py",
                "kotlin_metadata.py",
                "kotlin_binding.py",
                "kotlin_frame.py",
                "kotlin_test_binding.py",
                "kotlin_elf.py",
            )
        ],
    ]
    for folder in (
        project.SDK + "/src/common",
        project.SDK + "/src/" + variant,
        project.SDK + "/native",
        project.SDK + "/facade",
        project.SDK + "/record-codegen",
        project.RECIPES,
    ):
        names.extend(
            p.relative_to(repo).as_posix()
            for p in sorted((repo / folder).rglob("*"))
            if p.is_file()
        )
    if variant == "test":
        for folder in (
            project.SDK + "/test-observer",
            project.SDK + "/contracts",
            project.SDK + "/test-runtime",
        ):
            names.extend(
                p.relative_to(repo).as_posix()
                for p in sorted((repo / folder).rglob("*"))
                if p.is_file()
            )
    names += [
        "sdk/arkts/facade/Boundary.ets.in",
        project.SDK + "/frontend-policy.json",
        project.SDK + "/README.md",
    ]
    return [
        dict(
            path=n,
            source=str(support.input_path(repo, n)),
            sha256=support.sha(support.input_path(repo, n)),
            size=support.input_path(repo, n).stat().st_size,
        )
        for n in sorted(set(names))
    ]


def prepare_configuration(base_observation, reviewed_config, native_module, variant):
    if (
        variant not in ("ordinary", "test")
        or native_module
        != "libmadbc07sdkkotlin" + ("test" if variant == "test" else "") + ".so"
    ):
        raise ValueError("Kotlin module differs from selected variant")
    if set(reviewed_config) != {"tools", "kotlin"}:
        raise ValueError("Kotlin preparation requires explicit tools and closure")
    observation = deepcopy(base_observation)
    config = observation["config"]
    selected = deepcopy(reviewed_config["kotlin"])
    if (selected["provider"], selected["version"], selected["akinterop"]) != (
        "kotlin-owned",
        "2.2.21-1.1.0",
        "1.1.0",
    ):
        raise ValueError("Unreviewed Kotlin backend")
    for name, tool in reviewed_config["tools"].items():
        support.verify_tool(tool)
        if name in config["tools"] and any(
            config["tools"][name][key] != tool[key] for key in ("sha256", "size")
        ):
            raise ValueError("Base/Kotlin tool conflict: " + name)
        config["tools"].setdefault(name, tool)
    config["native_module"] = native_module
    config["kotlin"] = selected
    selected["test_support"] = variant == "test"
    source = source_inputs(_ROOT, variant)
    pin = support.read_json(support.input_path(_ROOT, dependencies.SOURCE_PIN))
    for e in pin["base_files"]:
        p = support.input_path(selected["upstream_root"], e["path"])
        if support.sha(p) != "sha256:" + e["sha256"] or p.stat().st_size != e["size"]:
            raise ValueError("Public akinterop base changed: " + e["path"])
        source.append(
            dict(
                path=project.BASE_PREFIX + e["path"],
                source=str(p),
                sha256="sha256:" + e["sha256"],
                size=e["size"],
            )
        )
    inventory_path = dependencies.verify_file(observation["inventory"])
    inventory = support.read_json(inventory_path, 64 * support.MAX_JSON)
    cli = Path(config["cli_root"]).resolve(strict=True)
    native_sdk = Path(config["native_sdk"]).resolve(strict=True)
    if not native_sdk.is_relative_to(cli):
        raise ValueError("Kotlin formal headers must belong to the verified CLI SDK")
    original_headers = {e["path"]: e for e in inventory["entries"] if e["kind"] == "file"}
    for name in dependencies.HEADERS:
        p = (native_sdk / "sysroot/usr/include" / name).resolve(strict=True)
        if not p.is_relative_to(cli):
            raise ValueError("Kotlin formal header escapes CLI inventory")
        original = original_headers.get(p.relative_to(cli).as_posix())
        if (
            original is None
            or support.sha(p) != "sha256:" + original["sha256"]
            or p.stat().st_size != original["size"]
        ):
            raise ValueError(
                "Kotlin formal header differs from original CLI inventory: " + name
            )
        source.append(
            dict(
                path="kotlin-formal-headers/" + name,
                source=str(p),
                sha256=support.sha(p),
                size=p.stat().st_size,
            )
        )
    dependency_lock = support.read_json(dependencies.verify_file(selected["dependency_lock"]))
    selected["maven_summary"] = dict(
        sha256=selected["dependency_lock"]["sha256"],
        entries=len(dependency_lock["maven"]),
        size=sum(e["bytes"] for e in dependency_lock["maven"]),
    )
    selected["source_inputs"] = [
        {k: e[k] for k in ("path", "sha256", "size")}
        for e in sorted(source, key=lambda e: e["path"])
    ]
    receipts = [
        dict(
            path="kotlin-receipts/dependency-lock.json",
            source=selected["dependency_lock"]["path"],
            sha256=selected["dependency_lock"]["sha256"],
            size=selected["dependency_lock"]["size"],
        )
    ]
    for key, tree in sorted(selected["trees"].items()):
        dependencies.verify_tree(tree)
        receipts.append(
            dict(
                path="kotlin-receipts/" + key + "-tree.json",
                source=tree["manifest"]["path"],
                sha256=tree["manifest"]["sha256"],
                size=tree["manifest"]["size"],
            )
        )
    runtime = (
        Path(selected["trees"]["llvm"]["root"]) / "lib/aarch64-linux-ohos/libc++_shared.so"
    )
    native_runtime = {"soname": "libc++_shared.so", **dependencies.ref(runtime)}
    selected["native_runtime"] = native_runtime
    observation["native_runtime"] = native_runtime
    public = observation["public"]
    public["native_runtime"] = {k: native_runtime[k] for k in ("soname", "sha256", "size")}

    public["tools"] = [
        {k: t[k] for k in ("id", "version", "sha256", "size", "source")}
        for _, t in sorted(config["tools"].items())
    ]
    public["kotlin"] = dict(
        provider="cpf",
        version="2.2.21-1.1.0",
        akinterop="1.1.0",
        ksp="KSP1/2.2.21-2.0.4",
        gradle="8.14.6",
        jdk="21.0.5+11",
        dependency_lock=selected["dependency_lock"]["sha256"],
        trees={k: v["summary"] for k, v in selected["trees"].items()},
        test_support=variant == "test",
        compiler_platform="CPF sysroot 6.0.2.640-04 and matching platform KLIB",
        native_api_headers="selected formal application SDK; distinct from compiler platform KLIB",
    )
    notices = []
    pairs = [
        (
            "akinterop/LICENSE",
            _ROOT / "adapters/generators/akinterop/patches/LICENSE-Apache-2.0",
        ),
        ("akinterop/NOTICE", _ROOT / "adapters/generators/akinterop/patches/NOTICE-upstream"),
        (
            "cpf-kotlin/LICENSE.txt",
            Path(selected["trees"]["compiler"]["root"]) / "licenses/LICENSE.txt",
        ),
        (
            "cpf-kotlin/NOTICE.txt",
            Path(selected["trees"]["compiler"]["root"]) / "licenses/NOTICE.txt",
        ),
        ("cpf-llvm/NOTICE", Path(selected["trees"]["llvm"]["root"]) / "NOTICE"),
    ]
    runtime_notices = _ROOT / project.SDK / "notices"
    notice_sources = support.read_json(runtime_notices / "sources.json")
    locked = {item["relative_path"]: item for item in dependency_lock["maven"]}
    for library in notice_sources["libraries"]:
        for item in library["artifacts"]:
            actual = locked.get(item["relative_path"])
            if actual is None or any(
                actual[key] != item[key] for key in ("url", "sha256", "bytes")
            ):
                raise ValueError("Runtime license dependency differs from actual Maven lock")
    pairs.extend(
        ("runtime-libraries/" + p.relative_to(runtime_notices).as_posix(), p)
        for p in sorted(runtime_notices.rglob("*"))
        if p.is_file()
    )
    for name, p in pairs:
        notices.append(
            dict(
                path="notices/kotlin/" + name,
                source=str(p),
                sha256=support.sha(p),
                size=p.stat().st_size,
            )
        )
    return dict(
        observation=observation,
        source_inputs=source,
        notice_inputs=notices,
        receipt_inputs=receipts,
    )


def read_metadata(inputs, config, work, public_descriptor, *, capture=None):
    return metadata.read_metadata(inputs, config, work, public_descriptor, capture=capture)


def _identity(inputs):
    identity = support.read_json(support.input_path(inputs, "identity.json"))
    if (
        identity.get("schema") != "madb.arkts-runtime-identity/1"
        or identity.get("bootstrap_version") != 1
    ):
        raise ValueError("Kotlin Build needs the standard C04 identity")
    digests = [identity[k] for k in ("abi_digest", "profile_digest", "build_id")]
    if any(not re.fullmatch("sha256:[0-9a-f]{64}", d) for d in digests):
        raise ValueError("Invalid standard identity digests")
    if identity["expected_identity"] != "madb-arkts/1:" + ":".join(
        d.removeprefix("sha256:") for d in digests
    ):
        raise ValueError("Kotlin expected identity does not match digests")
    return identity


def _declared(inputs):
    return support.read_json(
        support.input_path(inputs, "declarations.json"), 16 * support.MAX_JSON
    )["kotlin"]


def _bootstrap(config, root, capture, *, link):
    sysroot = Path(config["kotlin"]["trees"]["ohos"]["root"])
    obj = root / "native/bootstrap/bootstrap.o"
    if not link:
        args = [
            "--target=aarch64-linux-ohos",
            "--sysroot=" + str(sysroot),
            "-std=c++17",
            "-fPIC",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-D__OHOS_API__=23",
            "-I" + str(root / "inputs/formal-api/include"),
            "-I" + str(root / "native-support"),
            "-I" + str(root / "native-bootstrap"),
            "-I" + str(root / "native-observer"),
            "-MMD",
            "-MF",
            str(obj.with_suffix(".d")),
            "-c",
            root / "native-bootstrap/bootstrap.cpp",
            "-o",
            obj,
        ]
        if config["kotlin"]["test_support"]:
            args.insert(0, "-DMADB_KN_TEST_SUPPORT=1")
        support.command(config, "cpf_clang", args, cwd=root, capture=capture)
        if config["kotlin"]["test_support"]:
            observer_args = list(args)
            observer_args[observer_args.index(root / "native-bootstrap/bootstrap.cpp")] = (
                root / "native-observer/bootstrap_observer.cpp"
            )
            observer_args[observer_args.index(obj)] = root / "native/bootstrap/observer.o"
            observer_args[observer_args.index(str(obj.with_suffix(".d")))] = str(
                root / "native/bootstrap/observer.d"
            )
            support.command(config, "cpf_clang", observer_args, cwd=root, capture=capture)
    else:
        support.command(
            config,
            "cpf_clang",
            [
                "--target=aarch64-linux-ohos",
                "--sysroot=" + str(sysroot),
                "-shared",
                obj,
                *(
                    [
                        root / "native/bootstrap/observer.o",
                        *[
                            "-Wl," + flag
                            for flag in (root / "native-observer/bootstrap-wrap.rsp")
                            .read_text()
                            .splitlines()
                        ],
                    ]
                    if config["kotlin"]["test_support"]
                    else []
                ),
                "-L"
                + str(
                    Path(config["kotlin"]["trees"]["llvm"]["root"]) / "lib/aarch64-linux-ohos"
                ),
                "-lace_napi.z",
                "-ldl",
                "-Wl,--no-undefined",
                "-Wl,-soname," + config["native_module"],
                "-o",
                root / "artifacts" / config["native_module"],
            ],
            cwd=root,
            capture=capture,
        )


def run_compile(config, inputs, work, capture=None):
    inputs, work = Path(inputs), Path(work)
    capture = [] if capture is None else capture
    dependencies.verify(inputs, config)
    identity = _identity(inputs)
    declared = _declared(inputs)
    workspace, key = metadata.workspace_key(inputs, config, "kotlin-compile", identity)
    outputs = []
    with workspace.acquire(
        key, base=Path(config["kotlin"]["compiler_workspace"]["root"])
    ) as stage:
        root = project.materialize(inputs, config, stage / "project", identity=identity)
        project.compile_native(inputs, config, root, capture)
        _bootstrap(config, root, capture, link=False)
        project.gradle(config, root, workspace, [":probe:kspKotlinOhosArm64"], capture)
        actual = metadata.collect(config, root, inputs, capture)
        if actual != declared:
            raise ValueError("Kotlin Compile KSP authority differs from Metadata")
        for source, name in (
            (root / "native/lib/libnapi_bridge.a", "objects/libnapi_bridge.a"),
            (root / "native/lib/libmadb_kn_executor.a", "objects/libmadb_kn_executor.a"),
            (root / "native/bootstrap/bootstrap.o", "objects/kotlin-bootstrap.o"),
        ):
            support.copy_new(source, work / name)
            outputs.append(name)
        if config["kotlin"]["test_support"]:
            name = "objects/kotlin-observer.o"
            support.copy_new(root / "native/bootstrap/observer.o", work / name)
            outputs.append(name)
        project.write(
            work / "kotlin-compile.json",
            json.dumps(
                dict(
                    scope="actual CPF helper/executor/bootstrap objects and KSP/type check; not target execution",
                    kotlin=actual,
                    identity=identity,
                ),
                sort_keys=True,
            )
            + "\n",
        )
        outputs.append("kotlin-compile.json")
        support.verify_refs(inputs, config["kotlin"]["source_inputs"])
        dependencies.verify(inputs, config)
    return outputs


def run_link(config, inputs, work, capture=None):
    inputs, work = Path(inputs), Path(work)
    capture = [] if capture is None else capture
    dependencies.verify(inputs, config)
    identity = _identity(inputs)
    declared = _declared(inputs)
    workspace, key = metadata.workspace_key(inputs, config, "kotlin-link", identity)
    outputs = []
    with workspace.acquire(
        key, base=Path(config["kotlin"]["compiler_workspace"]["root"])
    ) as stage:
        root = project.materialize(inputs, config, stage / "project", identity=identity)
        for name, dest in (
            ("objects/libnapi_bridge.a", "native/lib/libnapi_bridge.a"),
            ("objects/libmadb_kn_executor.a", "native/lib/libmadb_kn_executor.a"),
            ("objects/kotlin-bootstrap.o", "native/bootstrap/bootstrap.o"),
        ):
            support.copy_new(support.input_path(inputs, name), root / dest)
        if config["kotlin"]["test_support"]:
            support.copy_new(
                support.input_path(inputs, "objects/kotlin-observer.o"),
                root / "native/bootstrap/observer.o",
            )
        project.gradle(config, root, workspace, [":probe:linkReleaseSharedOhosArm64"], capture)
        if metadata.collect(config, root, inputs, capture) != declared:
            raise ValueError("Kotlin Link KSP authority differs from Metadata")
        _bootstrap(config, root, capture, link=True)
        libraries = [
            root / "artifacts" / config["native_module"],
            root / "probe/build/bin/ohosArm64/releaseShared/libmadb_kotlin_owned_runtime.so",
            Path(config["kotlin"]["trees"]["llvm"]["root"])
            / "lib/aarch64-linux-ohos/libc++_shared.so",
        ]
        elf = kotlin_elf.check(config, inputs, libraries, root, capture)
        if config["kotlin"]["test_support"]:
            support.command(
                config,
                "python",
                [
                    root / "native-observer/check_coverage.py",
                    "--manifest",
                    root / "native-observer/manifest.json",
                    "--readelf",
                    support.verify_tool(config["tools"]["cpf_readelf"]),
                    "--bootstrap",
                    libraries[0],
                    "--provider",
                    libraries[1],
                    "--output",
                    work / "kotlin-napi-coverage.json",
                ],
                cwd=root,
                capture=capture,
            )
            outputs.append("kotlin-napi-coverage.json")
        for source in libraries:
            name = "native/" + source.name
            support.copy_new(source, work / name)
            outputs.append(name)
        link_records = [
            json.loads(p.read_text())
            for p in sorted((root / "logs").glob("linker-argv-*.json"))
        ]
        if len(link_records) != 1:
            raise ValueError("Missing unique actual CPF shared-link correction receipt")
        project.write(
            work / "kotlin-link.json",
            json.dumps(
                dict(
                    scope="matching native helpers/cinterop/runtime/KSP consumer Release shared libraries; no target execution",
                    identity=identity,
                    outputs=[
                        dict(
                            path=n, sha256=support.sha(work / n), size=(work / n).stat().st_size
                        )
                        for n in outputs
                    ],
                    linker_correction=link_records,
                ),
                sort_keys=True,
            )
            + "\n",
        )
        outputs.append("kotlin-link.json")
        project.write(work / "kotlin-symbols.json", json.dumps(elf, sort_keys=True) + "\n")
        outputs.append("kotlin-symbols.json")
        support.verify_refs(inputs, config["kotlin"]["source_inputs"])
        dependencies.verify(inputs, config)
    return outputs
