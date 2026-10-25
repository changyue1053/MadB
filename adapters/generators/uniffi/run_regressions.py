"""Reproduce C06 managed-runtime regressions; this is not Lab candidate certification."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
spec = importlib.util.spec_from_file_location("madb_uniffi_driver", HERE / "driver.py")
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


def execute(command, name, output, environment=None, expect_failure=False):
    result = subprocess.run(command, cwd=REPO, env=environment, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600)
    log = output / (name + ".log")
    log.write_text(result.stdout)
    if expect_failure:
        # A compiler/network failure is not evidence for the historical allocation bug.
        if result.returncode == 0 or "left: 0" not in result.stdout or "right: 1" not in result.stdout:
            raise RuntimeError(f"Expected exact zero-release regression was not observed: {log}")
    elif result.returncode:
        raise RuntimeError(f"Regression failed ({result.returncode}): {log}")
    return {"check": name, "command": command, "exit_code": result.returncode,
            "expected_failure": expect_failure, "log": str(log), "log_sha256": driver.digest(log)}


def provenance_checks(tools, output):
    driver.verify(tools)
    copied = output / "tamper-probe"
    copied.mkdir()
    original = Path(tools["root"])
    for name in ("sources", "downloads", "bin", "build-source"):
        shutil.copytree(original / name, copied / name)
    shutil.copy2(original / "receipt.json", copied / "receipt.json")
    probe = dict(tools, root=str(copied))
    cases = ["sources/uniffi_core-0.32.2/src/lib.rs", "build-source/sdk/rust/uniffi/src/bin/madb-uniffi-metadata.rs",
             "downloads/uniffi_core-0.32.2.crate", "bin/madb-uniffi-metadata"]
    results = []
    for name in cases:
        target = copied / name
        before = target.read_bytes()
        target.write_bytes(before + b"\nchanged\n")
        try:
            driver.verify(probe)
        except ValueError as error:
            results.append({"check": "tamper-rejected", "path": name, "diagnostic": str(error)})
        else:
            raise AssertionError(f"Installation tamper was accepted: {name}")
        finally:
            target.write_bytes(before)
    target = copied / "sources/uniffi_core-0.32.2/src/lib.rs"
    before = target.read_bytes()
    target.unlink()
    target.symlink_to(original / "sources/uniffi_core-0.32.2/src/lib.rs")
    try:
        driver.verify(probe)
    except ValueError as error:
        results.append({"check": "same-content-symlink-rejected", "diagnostic": str(error)})
    else:
        raise AssertionError("Same-content source symlink was accepted")
    finally:
        target.unlink()
        target.write_bytes(before)
    special = copied / "sources/uniffi_core-0.32.2/not-a-source-file"
    os.mkfifo(special)
    try:
        driver.verify(probe)
    except ValueError as error:
        results.append({"check": "special-file-rejected", "diagnostic": str(error)})
    else:
        raise AssertionError("Special source file was accepted")
    finally:
        special.unlink()
    # An internally self-consistent receipt for an old patch set is still stale.
    receipt = json.loads((copied / "receipt.json").read_text())
    receipt["sources"]["uniffi_core"]["patches"] = []
    (copied / "receipt.json").write_text(json.dumps(receipt))
    probe["receipt_sha256"] = driver.digest(copied / "receipt.json")
    try:
        driver.verify(probe)
    except ValueError as error:
        results.append({"check": "stale-patch-set-rejected", "diagnostic": str(error)})
    else:
        raise AssertionError("Stale controlled patch receipt accepted")
    driver.verify(tools)
    return results


def runtime_checks(tools, output):
    receipt = driver.verify(tools)
    root = Path(tools["root"])
    archive = root / "downloads/uniffi_core-0.32.2.crate"
    if driver.digest(archive) != receipt["sources"]["uniffi_core"]["source_sha256"]:
        raise ValueError("Regression baseline archive differs from pinned upstream")
    original = output / "upstream"
    original.mkdir()
    with tarfile.open(archive) as package:
        package.extractall(original, filter="data")
    sources = {"original": original / "uniffi_core-0.32.2",
               "patched": root / "sources/uniffi_core-0.32.2"}
    fixtures = {"foreign": ("owned_result.rs", "src/ffi/foreignfuture.rs"),
                "native": ("native_ready_result.rs", "src/ffi/rustfuture/future.rs")}
    results = []
    environment = driver._environment({"CARGO_NET_OFFLINE": "true"})
    for version, source in sources.items():
        for kind, (fixture, module) in fixtures.items():
            destination = output / (version + "-" + kind)
            shutil.copytree(source, destination)
            module_path = destination / module
            module_path.write_text(module_path.read_text() + "\n#[cfg(test)]\nmod madb_allocation_regression {\n" +
                                   (HERE / "tests" / fixture).read_text() + "\n}\n")
            command = ["cargo", "+" + driver.RUST, "test", "--locked", "--offline",
                       "--manifest-path", str(destination / "Cargo.toml"), "--lib", "madb_allocation_regression",
                       "--", "--test-threads=1"]
            results.append(execute(command, version + "-" + kind, output, environment, version == "original"))
    # Include upstream unit tests and doctests, not only MadB's two reproductions.
    results.append(execute(["cargo", "+" + driver.RUST, "test", "--locked", "--offline", "--manifest-path",
                            str(output / "patched-native/Cargo.toml"), "--", "--test-threads=1"],
                           "patched-upstream-suite", output, environment))
    driver.verify(tools)  # Tests never mutate the production patched-source inventory.
    return results


def swift_command(source, generated, library, executable):
    native = str(Path(library).parent)
    return ["swiftc", "-swift-version", "6", "-strict-concurrency=complete", "-parse-as-library", "-O",
            "-I", str(generated), "-Xcc", "-fmodule-map-file=" + str(generated / "MadbBoundaryFFI.modulemap"),
            *map(str, source), "-L", native, "-lmadb_boundary_uniffi", "-Xlinker", "-rpath", "-Xlinker", native,
            "-o", str(executable)]


def consumer_checks(tools, output, kotlin_home, java_home, classpath):
    provider = driver.build_sdk(REPO, tools, output / "provider", "madb-c06-bounded-v1", True,
                                {"CARGO_NET_OFFLINE": "true"})
    swift, kotlin = output / "swift", output / "kotlin"
    driver.generate(REPO, tools, swift, "swift", library=provider["library"])
    driver.generate(REPO, tools, kotlin, "kotlin", library=provider["library"])
    results = []
    executable = output / "p03-swift"
    sources = [swift / "MadbBoundary.swift", HERE / "tests/P03Swift.swift",
               REPO / "sdk/swift/uniffi/MadbBoundaryClient.swift"]
    results.append(execute(swift_command(sources, swift, provider["library"], executable), "swift-build", output))
    results.append(execute([str(executable)], "swift-run", output))
    # Private generated helper tests must compile in the generated source file's scope.
    combined = output / "swift-cancellation.swift"
    combined.write_text((swift / "MadbBoundary.swift").read_text() + "\n" +
                        (HERE / "tests/swift_cancellation.swift").read_text())
    executable = output / "cancellation-swift"
    results.append(execute(swift_command([combined], swift, provider["library"], executable), "swift-cancel-build", output))
    results.append(execute([str(executable)], "swift-cancel-run", output))
    jar = output / "p03-jvm.jar"
    environment = driver._environment({"JAVA_HOME": str(java_home)})
    results.append(execute([str(kotlin_home / "bin/kotlinc"), str(kotlin / "madb/boundary/madb_boundary.kt"),
                            str(HERE / "tests/P03Jvm.kt"), "-classpath", classpath, "-jvm-target", "21", "-d", str(jar)],
                           "jvm-build", output, environment))
    results.append(execute([str(java_home / "bin/java"), "-Djna.library.path=" + provider["native"],
                            "-cp", str(jar) + os.pathsep + classpath, "madb.boundary.P03JvmKt"],
                           "jvm-run", output, environment))
    return {"provider": provider, "checks": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runtime-only", action="store_true")
    parser.add_argument("--kotlin-home", type=Path)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--jvm-classpath")
    args = parser.parse_args()
    if not args.runtime_only and not all((args.kotlin_home, args.java_home, args.jvm_classpath)):
        parser.error("Full regressions require explicit --kotlin-home, --java-home and --jvm-classpath")
    tools = json.loads(args.tools.read_text())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema_version": "1.0.0", "scope": "managed-toolchain-regressions-not-candidate-certification",
              "toolchain": tools, "fixtures": driver.inventory(HERE / "tests"),
              "provenance": provenance_checks(tools, output),
              "runtime": runtime_checks(tools, output)}
    if not args.runtime_only:
        report["consumers"] = consumer_checks(tools, output, args.kotlin_home, args.java_home, args.jvm_classpath)
    report["result"] = "passed"
    (output / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"result": "passed", "report": str(output / "report.json")}))


if __name__ == "__main__":
    main()
