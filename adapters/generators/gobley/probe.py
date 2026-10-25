"""Reproduce P03 with original and patched generated Native code and runtime."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess

from driver import HERE, digest, generate, verify_install
from native import compile_native


def run_probe(repository: Path, tools: Path, compiler_root: Path, dependencies: Path,
              library: Path, static_library: Path, output: Path) -> dict:
    verify_install(tools)
    output.mkdir(parents=True, exist_ok=True)
    result = {"generator_patches": [digest(HERE / ("patches/" + name)) for name in (
                  "gobley-0.3.7-once-completion.patch", "gobley-0.3.7-native-abi.patch")],
              "runtime_patches": [digest(HERE / ("patches/" + name)) for name in (
                  "uniffi_core-0.29.4-owned-result.patch", "uniffi_core-0.29.4-native-ready.patch")],
              "library_sha256": digest(library), "cases": []}
    for baseline in (True, False):
        label = "original" if baseline else "patched"
        generated = output / f"generated-{label}"
        generate(repository, tools, library, generated, baseline=baseline)
        binary = compile_native(generated, compiler_root, dependencies, static_library,
                                output / f"native-{label}", consumer=HERE / "fixtures/ForeignFutureProbe.kt", embedded_static=True)
        execution = subprocess.run([str(binary)], text=True, capture_output=True, timeout=60)
        (output / f"native-{label}.log").write_text(execution.stdout + execution.stderr)
        expected_line = "notification transfer=1 error=1 released=1" if baseline else "P03_NATIVE_PATCH_PASSED"
        if expected_line not in execution.stdout or (execution.returncode == 0) == baseline:
            raise RuntimeError(f"unexpected {label} generated Native regression result")
        result["cases"].append({"case": f"native-{label}", "exit_code": execution.returncode,
                                "expected_failure": baseline, "output": execution.stdout.strip()})
    families = [
        ("foreign", "foreign_future_allocations.rs", "src/ffi/foreignfuture.rs", 1, 12),
        ("native-ready", "native_ready_allocations.rs", "src/ffi/rustfuture/future.rs", 2, 5),
    ]
    for family, fixture_name, module_name, test_count, allocation_cases in families:
        for baseline in (True, False):
            label = "original" if baseline else "patched"
            source = tools / ("uniffi_core-0.29.4" if baseline else "uniffi_core-0.29.4-madb")
            runtime = output / f"runtime-{family}-{label}"
            shutil.copytree(source, runtime)
            module = runtime / module_name
            shutil.copyfile(HERE / "fixtures" / fixture_name, module.with_name("madb_allocation_probe.rs"))
            with module.open("a") as stream:
                stream.write('\n#[cfg(test)]\nmod madb_cleanup_regression { include!("madb_allocation_probe.rs"); }\n')
            command = ["cargo", "+1.99.1", "test", "--locked", "madb_cleanup_regression", "--", "--test-threads=1"]
            execution = subprocess.run(command, cwd=runtime, text=True, capture_output=True, timeout=180)
            log = execution.stdout + execution.stderr
            (output / f"runtime-{family}-{label}.log").write_text(log)
            valid = (execution.returncode == 101 and "left: 0" in log and "right: 1" in log) if baseline else (
                execution.returncode == 0 and f"{test_count} passed; 0 failed" in log)
            if not valid:
                raise RuntimeError(f"unexpected {family}/{label} allocated-result regression result")
            result["cases"].append({"case": f"runtime-{family}-{label}", "exit_code": execution.returncode,
                                    "expected_failure": baseline,
                                    "allocated_result_cases": allocation_cases if not baseline else test_count})
    verify_install(tools)
    (output / "observations.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    for name in ("tools", "compiler-root", "dependencies", "library", "static-library", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--repository", type=Path, default=HERE.parents[2])
    args = parser.parse_args()
    print(json.dumps(run_probe(args.repository.resolve(), args.tools.resolve(), args.compiler_root.resolve(),
                               args.dependencies.resolve(), args.library.resolve(), args.static_library.resolve(), args.output.resolve())))


if __name__ == "__main__":
    main()
