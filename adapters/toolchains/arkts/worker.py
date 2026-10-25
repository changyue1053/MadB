"""C04 compile/link worker for the fixed real C17 + C++17 C07 composition."""

from __future__ import annotations
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import sys

_spec = importlib.util.spec_from_file_location(
    "_c07_port", Path(__file__).resolve().parents[3] / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)


def log_name(phase):
    return "logs/" + phase + "-commands.json"


def checked_dependencies(depfile, inputs, native, original):
    """Clang -MD output is evidence of consumed headers, not ownership policy."""
    data = Path(depfile).read_bytes()
    if len(data) > support.MAX_JSON:
        raise ValueError("compiler dependency output exceeds budget")
    text = data.decode("utf-8").replace("\\\n", " ")
    if not text.startswith("madb-object:"):
        raise ValueError("unexpected compiler dependency target")
    paths = shlex.split(text.split(":", 1)[1])
    result = []
    for raw in sorted(set(paths)):
        p = Path(raw)
        if not p.is_absolute():
            raise ValueError("compiler dependency is not an absolute frozen path")
        if not (p.is_relative_to(inputs) or p.is_relative_to(native)):
            raise ValueError("compiler dependency path is outside frozen roots: " + str(p))
        resolved = p.resolve(strict=True)
        if resolved.is_relative_to(inputs):
            name = resolved.relative_to(inputs).as_posix()
            if name not in original:
                raise ValueError("compiler read an undeclared input: " + name)
            support.verify_refs(inputs, [original[name]])
            result.append(dict(original[name], origin="frozen-input"))
        elif resolved.is_relative_to(native):
            result.append(
                {
                    "path": resolved.relative_to(native).as_posix(),
                    "sha256": support.sha(resolved),
                    "size": resolved.stat().st_size,
                    "origin": "checked-native-sdk-tree",
                }
            )
        else:
            raise ValueError("compiler read outside frozen inputs/Native SDK: " + str(p))
    if not result:
        raise ValueError("compiler dependency inventory is empty")
    return result


def native_arguments(config, inputs, work):
    native = Path(config["native_sdk"])
    recipe = config["build"]
    semantic = [
        "--target=aarch64-linux-ohos23.0.0",
        "--sysroot=" + str(native / "sysroot"),
        "-std=c17",
        "-O2",
        "-fPIC",
        "-fvisibility=hidden",
    ]
    if config["seam_arguments"] != semantic:
        raise ValueError("C reader and compiler semantic arguments differ")
    common = [
        *[a for a in semantic if not a.startswith("-std=")],
        "-g",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-deprecated-declarations",
        "-Werror=date-time",
        "-ffile-prefix-map=" + str(inputs) + "=/madb/inputs",
        "-ffile-prefix-map=" + str(work) + "=/madb/work",
        "-fdebug-compilation-dir=/madb/work",
    ]
    includes = []
    for relative in recipe["includes"]:
        if Path(relative).is_absolute() or any(
            p in ("", ".", "..") for p in relative.split("/")
        ):
            raise ValueError("invalid frozen include directory")
        directory = inputs / relative
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError("missing ordinary include directory")
        includes += ["-I", str(directory)]
    return common, includes


def compile_native(config, inputs, work, capture=None):
    """The one actual Clang compile loop, also used by the Rust phase owner."""
    inputs = Path(inputs).resolve(strict=True)
    work = Path(work).resolve(strict=True)
    d = support.load_module(inputs / "adapters/toolchains/common/documents.py")
    native = Path(config["native_sdk"]).resolve(strict=True)
    common, includes = native_arguments(config, inputs, work)
    units = config["build"]["units"]
    logs = capture if capture is not None else []
    outputs = []
    log_path = log_name("compile")
    if not units or len(units) > 32 or len({u["name"] for u in units}) != len(units):
        raise ValueError("invalid compile unit inventory")
    for unit in units:
        if not re.fullmatch("[a-z][a-z0-9-]*", unit["name"]) or unit["language"] not in (
            "c",
            "cpp",
        ):
            raise ValueError("invalid C07 compile unit")

    def run(tool, args):
        try:
            return support.command(config, tool, args, cwd=work, capture=logs)
        finally:
            d.save(work, log_path, logs)

    original = {
        p.relative_to(inputs).as_posix(): d.ref(inputs, p.relative_to(inputs).as_posix())
        for p in inputs.rglob("*")
        if p.is_file()
    }
    # Reject source-tree links before the compiler can consume them.
    if any(p.is_symlink() for p in inputs.rglob("*")):
        raise ValueError("frozen compiler input tree contains a link")
    for u in units:
        source = support.input_path(inputs, u["source"])
        target = work / "objects" / (u["name"] + ".o")
        target.parent.mkdir(exist_ok=True)
        tool = "clang" if u["language"] == "c" else "clangxx"
        standard = "c17" if u["language"] == "c" else "c++17"
        depfile = work / "logs" / (u["name"] + ".d")
        depfile.parent.mkdir(exist_ok=True)
        run(
            tool,
            [
                *common,
                "-std=" + standard,
                *(["-fvisibility-inlines-hidden"] if u["language"] == "cpp" else []),
                *includes,
                "-MD",
                "-MT",
                "madb-object",
                "-MF",
                depfile,
                "-c",
                source,
                "-o",
                target,
            ],
        )
        dependencies = checked_dependencies(
            depfile, inputs, native.resolve(strict=True), original
        )
        logs.append(
            {
                "kind": "actual-compiler-dependencies",
                "unit": u["name"],
                "dependencies": dependencies,
            }
        )
        d.save(work, log_path, logs)
        depfile.unlink()
        outputs.append(target.relative_to(work).as_posix())
    return sorted(outputs)


def run_phase(config, phase, input_root, work):
    inputs = Path(input_root).resolve(strict=True)
    work = Path(work).resolve(strict=True)
    if (
        inputs == work
        or inputs.is_relative_to(work)
        or work.is_relative_to(inputs)
        or any(work.iterdir())
    ):
        raise ValueError("phase requires disjoint inputs and an empty work root")
    if config.get("schema") != "madb.private.c07-native-backend/1" or phase not in (
        "compile",
        "link",
    ):
        raise ValueError("C07 only accepts explicit compile/link")
    d = support.load_module(inputs / "adapters/toolchains/common/documents.py")
    verifier = support.load_module(inputs / "adapters/toolchains/arkts/tools.py")
    inventory = support.read_json(
        support.input_path(inputs, config["cli_inventory"]), 64 * support.MAX_JSON
    )
    verifier.verify_inventory(inventory, Path(config["cli_root"]))
    for tool in config["tools"].values():
        support.verify_tool(tool)
    support.load_module(Path(__file__).with_name("toolchain.py")).verify_runtime(config, inputs)
    recipe = config["build"]
    native = Path(config["native_sdk"])
    managed = config.get("provider") == "kotlin"
    units = [] if managed else recipe["units"]
    if not managed and (
        not units or len(units) > 32 or len({u["name"] for u in units}) != len(units)
    ):
        raise ValueError("invalid compile unit inventory")
    for u in units:
        if not re.fullmatch("[a-z][a-z0-9-]*", u["name"]) or u["language"] not in ("c", "cpp"):
            raise ValueError("invalid C07 compile unit")
    logs = []
    outputs = []
    log_path = log_name(phase)

    def run(tool, args):
        try:
            return support.command(config, tool, args, cwd=work, capture=logs)
        finally:
            # Retain exact argv/stdout/stderr/exit even when the phase fails.
            # Logs are BuildEvidence references, not reproducible output hashes.
            d.save(work, log_path, logs)

    common, includes = ([], []) if managed else native_arguments(config, inputs, work)
    if phase == "compile":
        if config.get("provider") in ("rust", "kotlin"):
            backend = support.load_module(Path(__file__).with_name(config["provider"] + ".py"))
            try:
                outputs = backend.run_compile(config, inputs, work, capture=logs)
            finally:
                d.save(work, log_path, logs)
        else:
            outputs = compile_native(config, inputs, work, capture=logs)
    else:
        soname = config["native_module"]
        if not re.fullmatch(r"lib[A-Za-z0-9_]+\.so", soname):
            raise ValueError("invalid fixed SONAME")
        target = work / "native" / soname
        target.parent.mkdir()
        if config.get("provider") in ("rust", "kotlin"):
            if recipe.get("strip") is not False:
                raise ValueError(
                    "this profile requires explicit unstripped Build; Package strip=false"
                )
            backend = support.load_module(Path(__file__).with_name(config["provider"] + ".py"))
            try:
                # Actual ohrs performs Rust code generation and DSO linking.
                # Its macro registrar has a different explicit symbol policy.
                outputs = backend.run_link(config, inputs, work, capture=logs)
            finally:
                d.save(work, log_path, logs)
        else:
            objects = [support.input_path(inputs, "objects/" + u["name"] + ".o") for u in units]
            run(
                "clangxx",
                [
                    *common,
                    "-shared",
                    "-Wl,--no-undefined",
                    "-Wl,--build-id=sha1",
                    "-Wl,-soname," + soname,
                    "-Wl,--version-script="
                    + str(support.input_path(inputs, "generated/native/exports.map")),
                    *objects,
                    "-lace_napi.z",
                    "-luv",
                    "-o",
                    target,
                ],
            )
            # Symbols are transformed only by a separately declared Build policy.
            if recipe.get("strip") is not False:
                raise ValueError(
                    "this profile requires explicit unstripped Build; Package strip=false"
                )
            outputs.append("native/" + soname)
            dynsym = run("readelf", ["--dyn-syms", "--wide", target]).decode()
            defined = []
            for line in dynsym.splitlines():
                parts = line.split()
                if (
                    len(parts) >= 8
                    and parts[0].endswith(":")
                    and parts[4] in ("GLOBAL", "WEAK")
                    and parts[6] != "UND"
                ):
                    defined.append(parts[7])
            if defined:
                raise ValueError(
                    "constructor-only NAPI library exported internal symbols: " + repr(defined)
                )
            dynamic_text = run("readelf", ["-d", target]).decode()
            if not re.search(r"\(INIT_ARRAYSZ\).*?[1-9][0-9]* \(bytes\)", dynamic_text):
                raise ValueError("constructor registration has no init-array")
            d.save(
                work,
                "native-symbols.json",
                {
                    "schema": "madb.private.native-symbols/1",
                    "mechanism": "static constructor registers napi_module; no dlsym entry exports",
                    "defined_dynamic_symbols": defined,
                },
            )
            outputs.append("native-symbols.json")
        dependencies = {}

        def dynamic(p):
            text = run("readelf", ["-d", p]).decode()
            return sorted(set(re.findall(r"\(NEEDED\).*?Shared library: \[([^]]+)\]", text)))

        implementation = recipe.get("implementation_libraries", [soname])
        if (
            not implementation
            or len(implementation) != len(set(implementation))
            or soname not in implementation
        ):
            raise ValueError("invalid implementation library inventory")
        declared = {r["soname"]: r for r in recipe["runtime_libraries"]}
        initial_native = {Path(name).name for name in outputs if name.startswith("native/")}
        if not set(implementation) <= initial_native or initial_native - set(
            implementation
        ) - set(declared):
            raise ValueError(
                "backend native output differs from declared implementation/runtime libraries"
            )
        for name in implementation:
            if not re.fullmatch(r"lib[A-Za-z0-9_]+\.so", name):
                raise ValueError("invalid implementation SONAME")
            dependencies[name] = dynamic(support.input_path(work, "native/" + name))
        if len(declared) != len(recipe["runtime_libraries"]):
            raise ValueError("duplicate runtime library SONAME")
        needed = {name for values in dependencies.values() for name in values}
        pending = list(needed - set(recipe["system_libraries"]))
        copied = set()
        while pending:
            name = pending.pop()
            if name in copied:
                continue
            if name not in declared:
                raise ValueError("ELF needs an undeclared runtime library: " + name)
            entry = declared[name]
            source = support.verify_tool(entry)
            out = work / "native" / name
            if out.exists() or out.is_symlink():
                support.verify_refs(
                    work,
                    [
                        {
                            "path": "native/" + name,
                            "sha256": entry["sha256"],
                            "size": entry["size"],
                        }
                    ],
                )
            else:
                support.copy_new(source, out)
            if "native/" + name not in outputs:
                outputs.append("native/" + name)
            copied.add(name)
            dependencies[name] = dynamic(out)
            needed.update(dependencies[name])
            pending.extend(set(dependencies[name]) - set(recipe["system_libraries"]) - copied)
        if copied != set(declared):
            raise ValueError("runtime inventory includes an unused library")
        d.save(
            work,
            "native-dependencies.json",
            {
                "schema": "madb.native-dependencies/1",
                "needed": dependencies,
                "system_libraries": sorted(recipe["system_libraries"]),
            },
        )
        outputs.append("native-dependencies.json")
        d.save(
            work,
            "sdk-inventory.json",
            {"schema": "madb.sdk-inventory/1", "files": sorted(outputs)},
        )
        outputs.append("sdk-inventory.json")
    d.save(work, log_path, logs)
    for tool in config["tools"].values():
        support.verify_tool(tool)
    support.load_module(Path(__file__).with_name("toolchain.py")).verify_runtime(config, inputs)
    verifier.verify_inventory(inventory, Path(config["cli_root"]))
    actual = {p.relative_to(work).as_posix() for p in work.rglob("*") if p.is_file()}
    if actual != set(outputs) | {log_path}:
        raise ValueError("C07 worker output closure mismatch")
    return sorted(outputs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    raw = sys.stdin.buffer.read(support.MAX_JSON + 1)
    if len(raw) > support.MAX_JSON:
        raise ValueError("BuildTask exceeds budget")
    task = json.loads(raw)
    work = Path.cwd().resolve()
    inputs = (work.parent / "inputs").resolve(strict=True)
    config = support.read_json(args.config)
    script = Path(__file__).resolve()
    config_path = args.config.resolve(strict=True)
    if not script.is_relative_to(inputs) or not config_path.is_relative_to(inputs):
        raise ValueError("worker/config are not frozen inputs")
    expected = config["tools"]["python"].get("invocation_arguments", []) + [
        os.path.relpath(script, work),
        "--config",
        os.path.relpath(config_path, work),
    ]
    if (
        task["schema"] != "madb.build-task/1"
        or task["arguments"] != expected
        or task["selector"]["target"] != "aarch64-linux-ohos"
    ):
        raise ValueError("BuildTask does not match C07 recipe")
    declared = support.verify_refs(inputs, task["inputs"])
    if (
        not {script.relative_to(inputs).as_posix(), config_path.relative_to(inputs).as_posix()}
        <= declared
    ):
        raise ValueError("worker/config missing from declared inputs")
    outputs = run_phase(config, task["phase"], inputs, work)
    support.verify_refs(inputs, task["inputs"])
    if sorted(task["outputs"]) != outputs:
        raise ValueError("BuildTask expected output closure mismatch")
    d = support.load_module(inputs / "adapters/toolchains/common/documents.py")
    evidence = dict(
        schema="madb.build-evidence/1",
        task_id=task["id"],
        task_digest=task["task_digest"],
        input_digest=task["input_digest"],
        profile_digest=task["selector"]["profile_digest"],
        generation_id=task["generation_id"],
        tool=task["tool"],
        phase=task["phase"],
        target=task["selector"]["target"],
        status="passed",
        exit_code=0,
        outputs=[d.ref(work, n) for n in outputs],
        logs=[d.ref(work, log_name(task["phase"]))],
    )
    sys.stdout.buffer.write((d.canonical(evidence) + "\n").encode())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        print("C07 build: " + str(error), file=sys.stderr)
        raise SystemExit(2) from error
