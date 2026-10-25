"""Materialize and build the single reviewed CPF project in an owned cold workspace."""

from __future__ import annotations
import hashlib
import json
from pathlib import Path
import re
import shutil
from .kotlin_dependencies import support, NAMES, SOURCE_PIN, PATCH, RUNTIME

SDK = "sdk/kotlin/arkts-owned"
RECIPES = "adapters/toolchains/arkts/kotlin-recipes"
BASE_PREFIX = "upstream/akinterop/"


def exact_patch(base, patch):
    """Replay only pinned text hunks, with exact offsets and newline markers."""
    result = dict(base)
    lines = patch.splitlines(keepends=True)
    i = 0
    seen = set()
    while i < len(lines):
        match = re.fullmatch(r"diff --git a/(\S+) b/(\S+)\n", lines[i])
        if not match or match[1] != match[2] or match[1] in seen:
            raise ValueError("Invalid managed patch header")
        name = match[1]
        seen.add(name)
        i += 1
        while i < len(lines) and not lines[i].startswith("--- "):
            if not re.fullmatch(
                r"(?:index [0-9a-f]+\.\.[0-9a-f]+(?: 100644)?|new file mode 100644)\n", lines[i]
            ):
                raise ValueError("Unsupported managed patch metadata")
            i += 1
        new = name not in base
        if (
            lines[i] != ("--- /dev/null\n" if new else "--- a/" + name + "\n")
            or lines[i + 1] != "+++ b/" + name + "\n"
        ):
            raise ValueError("Managed patch path mismatch")
        i += 2
        original = base.get(name, "").splitlines(keepends=True)
        out = []
        cursor = 0
        hunks = 0
        while i < len(lines) and not lines[i].startswith("diff --git "):
            h = re.fullmatch(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n", lines[i])
            if not h:
                raise ValueError("Invalid managed patch hunk")
            old, count, start, added = (int(h[1]), int(h[2] or 1), int(h[3]), int(h[4] or 1))
            pos = old - 1 if count else old
            if pos < cursor or pos > len(original):
                raise ValueError("Managed patch hunk offset")
            out.extend(original[cursor:pos])
            cursor = pos
            if len(out) != (start - 1 if added else start):
                raise ValueError("Managed new line offset")
            i += 1
            minus = plus = 0
            while i < len(lines) and not lines[i].startswith(("@@ ", "diff --git ")):
                line = lines[i]
                if line[0] not in " +-":
                    raise ValueError("Invalid managed patch line")
                marker, value = line[0], line[1:]
                i += 1
                if i < len(lines) and lines[i] == "\\ No newline at end of file\n":
                    if not value.endswith("\n"):
                        raise ValueError("Invalid newline marker")
                    value = value[:-1]
                    i += 1
                if marker in " -":
                    if cursor >= len(original) or original[cursor] != value:
                        raise ValueError("Managed patch context differs")
                    cursor += 1
                    minus += 1
                if marker in " +":
                    out.append(value)
                    plus += 1
            if (minus, plus) != (count, added):
                raise ValueError("Managed patch count differs")
            hunks += 1
        if not hunks:
            raise ValueError("Empty managed file patch")
        out.extend(original[cursor:])
        result[name] = "".join(out)
    return result


def runtime_sources(inputs):
    pin = support.read_json(support.input_path(inputs, SOURCE_PIN))
    base = {
        e["path"]: support.input_path(inputs, BASE_PREFIX + e["path"]).read_text()
        for e in pin["base_files"]
    }
    result = exact_patch(base, support.input_path(inputs, PATCH).read_text())
    expected = {e["path"]: e["sha256"] for e in pin["base_files"]}
    for e in pin["changed_files"]:
        if e["base_sha256"] != (
            hashlib.sha256(base[e["path"]].encode()).hexdigest() if e["path"] in base else None
        ):
            raise ValueError("Managed patch base identity differs")
        expected[e["path"]] = e["result_sha256"]
    if {k: hashlib.sha256(v.encode()).hexdigest() for k, v in result.items()} != expected:
        raise ValueError("Managed patch result identity differs")
    return {k.removeprefix(RUNTIME): v for k, v in result.items()}


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x") as out:
        out.write(text)


def render(source, substitutions):
    for key, value in substitutions.items():
        source = source.replace("@" + key + "@", str(value))
    if re.search(r"@[A-Z_]+@", source):
        raise ValueError("Unbound Kotlin recipe marker")
    return source


def materialize(inputs, config, project, *, identity=None):
    selected = config["kotlin"]
    project.mkdir()
    for n in (
        "gradle-home",
        "konan/dependencies",
        "cache",
        "tmp",
        "home",
        "native/obj",
        "native/lib",
        "native/support",
        "native/bootstrap",
        "logs",
        "artifacts",
    ):
        (project / n).mkdir(parents=True, exist_ok=True)
    trees = selected["trees"]
    compiler = project / "konan" / NAMES["compiler"]
    # No former Gradle/Konan caches; the only compiler change is exact offline configuration.
    shutil.copytree(trees["compiler"]["root"], compiler, symlinks=True)
    properties = compiler / "konan/konan.properties"
    before = properties.read_bytes()
    if before.count(b"airplaneMode = false") != 1:
        raise ValueError("CPF offline configuration does not match pinned source")
    properties.write_bytes(before.replace(b"airplaneMode = false", b"airplaneMode = true"))
    for key in ("llvm", "ohos", "hms", "ffi"):
        (project / "konan/dependencies" / NAMES[key]).symlink_to(
            trees[key]["root"], target_is_directory=True
        )
    (project / "konan/dependencies/.extracted").write_text(
        "\n".join(NAMES[k] for k in ("llvm", "ohos", "hms", "ffi")) + "\n"
    )
    replacements = dict(
        MAVEN=selected["maven_root"],
        JAVA_HOME=Path(trees["jdk"]["root"]) / "Contents/Home",
        PROJECT=project,
        COMPILER_HOME=compiler,
    )
    for name in (
        "build.gradle.kts",
        "settings.gradle.kts",
        "gradle.properties",
        "runtime/build.gradle.kts",
        "probe/build.gradle.kts",
        "record-codegen/build.gradle.kts",
    ):
        write(
            project / name,
            render(
                support.input_path(inputs, RECIPES + "/" + name + ".in").read_text(),
                replacements,
            ),
        )
    with (project / "gradle.properties").open("a") as f:
        f.write("\nmadbProfile=" + ("test" if selected["test_support"] else "ordinary") + "\n")
    runtime = runtime_sources(inputs)
    if selected["test_support"]:
        memory_root = SDK + "/test-runtime/"
        pin = support.read_json(support.input_path(inputs, memory_root + "source-pin.json"))
        patch = support.input_path(inputs, memory_root + "codec-memory.patch").read_text()
        added = support.input_path(
            inputs, memory_root + "ManagedRecordMemoryProbe.kt"
        ).read_text()
        if (
            hashlib.sha256(runtime[pin["path"]].encode()).hexdigest() != pin["base_sha256"]
            or hashlib.sha256(patch.encode()).hexdigest() != pin["patch_sha256"]
            or hashlib.sha256(added.encode()).hexdigest() != pin["added_sha256"]
        ):
            raise ValueError("Test codec memory input identity differs")
        runtime = exact_patch(runtime, patch)
        if (
            hashlib.sha256(runtime[pin["path"]].encode()).hexdigest() != pin["result_sha256"]
            or pin["added_path"] in runtime
        ):
            raise ValueError("Test codec memory result identity differs")
        runtime[pin["added_path"]] = added
    for name, text in runtime.items():
        write(project / "inputs/runtime/src" / name, text)
    for e in selected["source_inputs"]:
        name = e["path"]
        if name.startswith(SDK + "/src/common/"):
            dest = "probe/src/ohosArm64Main/kotlin/com/madb/c07/" + Path(name).name
        elif name.startswith(
            SDK + "/src/" + ("test" if selected["test_support"] else "ordinary") + "/"
        ):
            dest = (
                "probe/src/"
                + ("testSupportMain" if selected["test_support"] else "ordinaryMain")
                + "/kotlin/com/madb/c07/"
                + Path(name).name
            )
        elif name.startswith(SDK + "/record-codegen/"):
            dest = "record-codegen/" + name.removeprefix(SDK + "/record-codegen/")
        elif name.startswith(SDK + "/native/") and not name.endswith(".json"):
            dest = (
                "native-bootstrap/bootstrap.cpp"
                if name.endswith("bootstrap.cpp.in")
                else "native-support/" + Path(name).name
            )
        elif name.startswith(SDK + "/test-observer/"):
            dest = "native-observer/" + name.removeprefix(SDK + "/test-observer/")
        elif name.startswith("kotlin-formal-headers/"):
            dest = "inputs/formal-api/include/" + name.removeprefix("kotlin-formal-headers/")
        else:
            continue
        data = support.input_path(inputs, name).read_text()
        if name.endswith("bootstrap.cpp.in"):
            data = render(
                data,
                {
                    "MADB_KOTLIN_MODULE_NAME": config["native_module"]
                    .removeprefix("lib")
                    .removesuffix(".so")
                },
            )
        write(project / dest, data)
    if selected["test_support"]:
        for name in (
            "observer.h",
            "observer_provider.h",
            "observer_core.inc",
            "provider_observer.cpp",
        ):
            shutil.copyfile(
                project / "native-observer" / name, project / "native-support" / name
            )
        observer = project / "native-observer"
        receipt = json.loads((observer / "source-receipt.json").read_text())
        for item in receipt["files"]:
            p = observer / item["path"]
            if (
                hashlib.sha256(p.read_bytes()).hexdigest() != item["sha256"]
                or p.stat().st_size != item["size"]
            ):
                raise ValueError("Reviewed generated NAPI observer source drift")

        def observer_ref(name):
            p = observer / name
            return dict(
                path=str(p),
                sha256=hashlib.sha256(p.read_bytes()).hexdigest(),
                size=p.stat().st_size,
            )

        manifest = dict(
            support_files=[
                observer_ref(n)
                for n in (
                    "observer.h",
                    "observer_entry.h",
                    "observer_provider.h",
                    "observer_core.inc",
                )
            ],
            schema=observer_ref("observer-schema.json"),
            roles=[
                dict(
                    role=e["role"],
                    symbols=e["symbols"],
                    source=observer_ref(e["role"] + "_observer.cpp"),
                    linker_response=observer_ref(e["role"] + "-wrap.rsp"),
                )
                for e in receipt["roles"]
            ],
        )
        write(observer / "manifest.json", json.dumps(manifest, indent=2) + "\n")
        definition = project / "native-support/executor.def"
        definition.write_text(
            definition.read_text().replace(
                "headers = executor.h lifecycle.h main_gate.h",
                "headers = executor.h lifecycle.h main_gate.h observer_provider.h",
            )
        )
    value = (
        identity["expected_identity"]
        if identity
        else "madb-kotlin-metadata-only:no-runtime-admission"
    )
    header = "#pragma once\n#define MADB_KN_OWNED_IDENTITY " + json.dumps(value) + "\n"
    write(project / "native-bootstrap/identity.h", header)
    write(
        project / "inputs/runtime/src/nativeInterop/cinterop/cpp/include/owned_identity.h",
        header,
    )
    linker = support.verify_tool(config["tools"]["cpf_ld"])
    python = support.verify_tool(config["tools"]["python"])
    startup = str(
        project / "konan/dependencies" / NAMES["ohos"] / "usr/lib/aarch64-linux-ohos/Scrt1.o"
    )
    wrapper = render(
        support.input_path(inputs, RECIPES + "/link-shared.py.in").read_text(),
        dict(
            PYTHON=python,
            LINKER=repr(str(linker)),
            STARTUP=repr(startup),
            LOGS=repr(str(project / "logs")),
        ),
    )
    write(project / "link-shared.py", wrapper)
    (project / "link-shared.py").chmod(0o700)
    return project


def environment(config, project):
    return support.clean_environment(
        dict(
            JAVA_HOME=str(Path(config["kotlin"]["trees"]["jdk"]["root"]) / "Contents/Home"),
            GRADLE_USER_HOME=str(project / "gradle-home"),
            KONAN_DATA_DIR=str(project / "konan"),
            XDG_CACHE_HOME=str(project / "cache"),
            TMPDIR=str(project / "tmp"),
            HOME=str(project / "home"),
            OHOS_SDK_HOME=str(Path(config["native_sdk"]).parent),
            PATH=str(Path(config["kotlin"]["trees"]["jdk"]["root"]) / "Contents/Home/bin")
            + ":/usr/bin:/bin",
        )
    )


def compile_native(inputs, config, project, capture):
    sysroot = Path(config["kotlin"]["trees"]["ohos"]["root"])
    groups = [
        (
            "helper",
            project / "inputs/runtime/src/nativeInterop/cinterop/cpp/src",
            project / "inputs/runtime/src/nativeInterop/cinterop/cpp/include",
            "libnapi_bridge.a",
        ),
        (
            "support",
            project / "native-support",
            project / "native-support",
            "libmadb_kn_executor.a",
        ),
    ]
    for label, source, include, archive in groups:
        objects = []
        for src in sorted(source.glob("*.cpp")):
            obj = project / "native/obj" / (label + "-" + src.stem + ".o")
            objects.append(obj)
            support.command(
                config,
                "cpf_clang",
                [
                    "--target=aarch64-linux-ohos",
                    "--sysroot=" + str(sysroot),
                    "-std=c++17",
                    "-fPIC",
                    "-O2",
                    "-DOHOS_PLATFORM",
                    "-D__OHOS_API__=23",
                    "-I" + str(project / "inputs/formal-api/include"),
                    "-I" + str(include),
                    "-MMD",
                    "-MF",
                    str(obj.with_suffix(".d")),
                    "-c",
                    src,
                    "-o",
                    obj,
                ],
                cwd=project,
                capture=capture,
            )
        support.command(
            config,
            "cpf_ar",
            ["rcs", project / "native/lib" / archive, *objects],
            cwd=project,
            capture=capture,
        )


def gradle(config, project, workspace, tasks, capture):
    java = support.verify_tool(config["tools"]["cpf_java"])
    cli = support.verify_tool(config["tools"]["gradle_cli"])
    gradle = Path(config["kotlin"]["trees"]["gradle"]["root"])
    args = [
        str(java),
        "-Xmx64m",
        "-Xms64m",
        "-Dorg.gradle.appname=gradle",
        "-javaagent:" + str(gradle / "lib/agents/gradle-instrumentation-agent-8.14.6.jar"),
        "-jar",
        str(cli),
        "-p",
        str(project),
        "--no-daemon",
        "--console=plain",
        "--offline",
        *tasks,
    ]
    log = project / "logs/gradle.log"
    code = None
    try:
        code = workspace.execute(args, project, environment(config, project), log)
    finally:
        capture.append(
            dict(
                tool="gradle",
                arguments=args,
                java_sha256=support.sha(java),
                cli_sha256=support.sha(cli),
                exit_code=code,
                combined_stdout_stderr=log.read_text() if log.exists() else "",
            )
        )
    if code:
        output = log.read_text()
        diagnostics = [
            line for line in output.splitlines() if line.startswith(("e:", "error:"))
        ]
        raise RuntimeError(
            "CPF Gradle failed:\n"
            + "\n".join(diagnostics[:40])[:12000]
            + "\n"
            + output[-12000:]
        )
