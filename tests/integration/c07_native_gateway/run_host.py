"""Real native host regressions, explicitly not an ArkTS target execution."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.arkts.native import compose_native
from adapters.toolchains.arkts.preflight import read_json_metadata


def record(path):
    data = path.read_bytes()
    return {"path": str(path), "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-sdk", type=Path, required=True)
    parser.add_argument("--cc", type=Path, required=True)
    parser.add_argument("--cxx", type=Path, required=True)
    parser.add_argument("--uv-include", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--route", choices=("c", "cpp"), default="c")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    composition = compose_native(ROOT, args.output / "source", route=args.route, variant="test")
    generated = args.output / "generated"
    generated.mkdir()
    fields = read_json_metadata(ROOT / "sdk/arkts/facade/frame-layout.json")
    test_layout = read_json_metadata(ROOT / "sdk/arkts/test-support/contracts/test-frame-layout.json")
    for filename, name, mapping, offset, count in (
        ("frame.generated.hpp", "Frame", fields, 0, "Count"),
        ("test-frame.generated.hpp", "TestFrame", test_layout["fields"], 54, "TestCount"),
    ):
        members = [f"{value}={int(key)-offset}," for key, value in mapping.items()]
        (generated / filename).write_text("#pragma once\n#include <cstddef>\nnamespace gateway { enum "
                                         + name + " : size_t {\n" + "\n".join(members) + "\n"
                                         + count + "=" + str(len(mapping)) + "\n}; }\n")
    # A host-only identity binds this composition; it is not a package identity.
    digest = hashlib.sha256("".join(x.sha256 for x in composition.source_inputs).encode()).hexdigest()
    identity = f"madb-arkts/1:{digest}:{digest}:{digest}"
    constants = {"GatewayAbi": digest, "GatewayProfile": digest, "GatewayBuild": digest,
                 "GatewayIdentity": identity, "GatewayRouteIdentity": args.route + "test/" + identity,
                 "GatewayModuleName": "host_test_only"}
    (generated / "identity.generated.hpp").write_text(
        '#pragma once\n#include "provider_abi.h"\n#include "memory_hook.h"\nnamespace gateway {\n'
        + "\n".join(f'inline constexpr const char* {key}="{value}";' for key, value in constants.items())
        + f'\ninline constexpr auto GatewayProviderCreate = &{composition.provider_factory};\ninline constexpr auto GatewayProviderMemoryBind = &mp_{args.route}_test_memory_bind;\n}}\n')
    # Make only the needed platform declaration headers visible to the host.
    headers = args.output / "host-headers"
    headers.mkdir()
    header_root = args.native_sdk / "sysroot/usr/include"
    for name in ("napi", "info", "node_api.h", "node_api_types.h", "js_native_api.h", "js_native_api_types.h"):
        (headers / name).symlink_to(header_root / name, target_is_directory=(header_root / name).is_dir())
    tests = Path(__file__).resolve().parent
    runtime = ROOT / "sdk/arkts/native-runtime"
    inputs = [x.path for x in composition.source_inputs] + [x.path for x in composition.derived_files]
    inputs += [tests / name for name in ("test_native.cpp", "test_registry.cpp", "host-platform.hpp", "test_observer.cpp", "test_memory.cpp", "run_host.py")]
    inputs += sorted(generated.iterdir()) + [args.native_sdk / "oh-uni-package.json"]
    inputs += sorted(p for p in header_root.joinpath("napi").rglob("*") if p.is_file())
    inputs += [header_root / name for name in ("node_api.h", "node_api_types.h", "js_native_api.h", "js_native_api_types.h")]
    before = [record(path) for path in inputs]
    commands = []
    def run(command):
        command = list(map(str, command))
        result = subprocess.run(command, capture_output=True, text=True, timeout=45)
        commands.append({"command": command, "exit_code": result.returncode,
                         "stdout": result.stdout, "stderr": result.stderr})
        (args.output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
        print(result.stdout + result.stderr, end="", flush=True)
        if result.returncode:
            raise RuntimeError(f"host command failed ({result.returncode}): {command[0]}")
    run([args.cc, "--version"])
    run([args.cxx, "--version"])
    includes = [*composition.include_dirs, composition.units[2].path.parent, generated, headers, args.uv_include]
    for label, flags in (("release", ["-O2"]),
                         ("asan-ubsan", ["-O1", "-fsanitize=address,undefined", "-fno-omit-frame-pointer"])):
        provider = args.output / (label + "-provider.o")
        unit = composition.units[-1]
        run([args.cc if unit.language == "c" else args.cxx,
             "-std=c17" if unit.language == "c" else "-std=c++17", *flags,
             "-I", runtime / "include", "-I", args.output / "source/include", "-c", unit.path, "-o", provider])
        common = [args.cxx, "-std=c++17", *flags, "-g", "-Wall", "-Wextra", "-Werror",
                  "-Wno-deprecated-declarations", "-include", tests / "host-platform.hpp",
                  *[value for path in includes for value in ("-I", path)]]
        dependencies = [args.output / "source/src/support.cpp", args.output / "source/src/observer.cpp", args.output / "source/worker/worker.cpp",
                        composition.units[0].path, args.output / "source/src/memory.cpp", provider]
        for kind, extra in (("native", ["-Wl,-undefined,dynamic_lookup", "-Wl,-dead_strip"]),
                            ("registry", ["-Wl,-undefined,dynamic_lookup", "-Wl,-dead_strip"]),
                            ("memory", ["-Wl,-undefined,dynamic_lookup", "-Wl,-dead_strip"])):
            executable = args.output / (label + "-" + kind)
            run([*common, tests / f"test_{kind}.cpp", *dependencies, *extra, "-o", executable])
            run([executable])
        observer = args.output / (label + "-observer")
        run([args.cxx, "-std=c++17", *flags, "-Wall", "-Wextra", "-Werror",
             "-Wno-deprecated-declarations", "-I", headers, "-I", args.output / "source/include",
             args.output / "source/src/observer.cpp", tests / "test_observer.cpp", "-o", observer])
        run([observer])
    if before != [record(path) for path in inputs]:
        raise ValueError("host regression inputs changed during execution")
    (args.output / "receipt.json").write_text(json.dumps({
        "scope": "real Core/provider/worker host regression; registry Node-API return harness; no target claim",
        "route": args.route, "checks_per_configuration": 33, "memory_checks_per_configuration": 7, "observer_stub_calls_per_configuration": 32,
        "configurations": ["Release", "ASan/UBSan"], "inputs": before,
        "commands": record(args.output / "commands.json"),
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
