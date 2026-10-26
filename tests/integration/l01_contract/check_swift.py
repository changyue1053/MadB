"""Compile public Observation contracts and exact rejection cases; no SDK runtime."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[3]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.python_version() != (ROOT / ".python-version").read_text().strip():
        raise ValueError("use repository-pinned Python")
    output = args.output.absolute()
    if output.exists() or output.is_symlink():
        raise ValueError("output must be a new directory")
    # Resolve only existing parents; do not permit output in the input tree or
    # the managed tool installation. Every compiler cache is local to output.
    output = output.parent.resolve(strict=True) / output.name
    if output.is_relative_to(ROOT.resolve()):
        raise ValueError("output must be outside the source repository")
    output.mkdir(mode=0o700)
    report = {"scope": "L01 Swift compile/macro expansion; no native or UI runtime", "commands": []}
    environment = os.environ.copy()
    environment["TMPDIR"] = str(output / "tmp")
    for name in ("tmp", "inputs", "module", "cache"):
        (output / name).mkdir()

    def run(arguments, *, expect_success=True):
        arguments = [str(value) for value in arguments]
        result = subprocess.run(arguments, cwd=output, env=environment, capture_output=True, timeout=120)
        number = len(report["commands"])
        (output / f"{number:02d}.stdout").write_bytes(result.stdout)
        (output / f"{number:02d}.stderr").write_bytes(result.stderr)
        report["commands"].append({
            "argv": arguments, "exit_code": result.returncode,
            "stdout_sha256": digest(result.stdout), "stderr_sha256": digest(result.stderr),
        })
        if expect_success and result.returncode != 0:
            raise RuntimeError(f"command failed ({result.returncode}): {arguments[0]}\n{result.stderr.decode(errors='replace')}")
        return result

    try:
        lock = json.loads((ROOT / "adapters/toolchains/kotlin_rust/toolchain.json").read_text())["xcode"]
        swiftc = Path(run(["xcrun", "--find", "swiftc"]).stdout.decode().strip())
        version = run([swiftc, "--version"]).stdout.decode().strip()
        xcode = run(["xcodebuild", "-version"]).stdout.decode().strip()
        sdk = run(["xcrun", "--sdk", "macosx", "--show-sdk-path"]).stdout.decode().strip()
        sdk_version = run(["xcrun", "--sdk", "macosx", "--show-sdk-version"]).stdout.decode().strip()
        sdk_build = run(["xcrun", "--sdk", "macosx", "--show-sdk-build-version"]).stdout.decode().strip()
        if (lock["swift"] not in version or xcode != f"Xcode {lock['version']}\nBuild version {lock['build']}"
                or sdk_version != lock["sdk_version"] or sdk_build != lock["sdk_build"]):
            raise ValueError("Apple compiler/SDK differs from existing managed lock")
        if output.is_relative_to(swiftc.parents[5]) or output.is_relative_to(Path(sdk)):
            raise ValueError("output overlaps tool installation")
        report["toolchain"] = {"swift": version, "xcode": xcode, "sdk": sdk, "sdk_version": sdk_version, "sdk_build": sdk_build,
                               "compiler_sha256": digest(swiftc.read_bytes())}
        sources = sorted((ROOT / "sdk/swift/observation/Sources/MadBObservation").glob("*.swift"))
        fixtures = sorted((Path(__file__).parent / "fixtures").glob("*.swift"))
        inputs = {}
        for source in sources + fixtures + [Path(__file__).resolve(), ROOT / "adapters/toolchains/kotlin_rust/toolchain.json"]:
            relative = source.relative_to(ROOT).as_posix()
            data = source.read_bytes()
            inputs[relative] = {"sha256": digest(data), "size": len(data)}
            target = output / "inputs" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        report["inputs"] = inputs
        common = ["-swift-version", "6", "-strict-concurrency=complete", "-warnings-as-errors",
                  "-parse-as-library", "-target", "arm64-apple-macosx14.0", "-sdk", sdk,
                  "-module-cache-path", output / "cache"]
        captured = lambda source: output / "inputs" / source.relative_to(ROOT)
        expansion = run([swiftc, *common, "-emit-module", "-module-name", "MadBObservation",
                         "-emit-module-path", output / "module/MadBObservation.swiftmodule",
                         "-Xfrontend", "-dump-macro-expansions", *map(captured, sources)])
        if b"ObservationRegistrar" not in expansion.stderr or b"withMutation" not in expansion.stderr:
            raise ValueError("actual system Observation macro expansion was not captured")
        fixture_root = output / "inputs/tests/integration/l01_contract/fixtures"
        run([swiftc, *common, "-I", output / "module", "-typecheck", fixture_root / "OrdinaryObservation.swift"])
        failures = [
            ("RejectMutableSnapshot.swift", "does not conform to the 'Sendable' protocol"),
            ("RejectUnsynchronizedActor.swift", "main actor-isolated instance method 'requestStop()'"),
        ]
        for name, required in failures:
            result = run([swiftc, *common, "-I", output / "module", "-typecheck", fixture_root / name], expect_success=False)
            stderr = result.stderr.decode(errors="replace")
            if result.returncode == 0 or required not in stderr or name + ":" not in stderr:
                raise ValueError("negative did not fail at the expected consumer source/reason: " + name)
        for relative, expected in inputs.items():
            if digest((ROOT / relative).read_bytes()) != expected["sha256"]:
                raise ValueError("source changed during check: " + relative)
        if digest(swiftc.read_bytes()) != report["toolchain"]["compiler_sha256"]:
            raise ValueError("compiler changed during check")
        report["result"] = "passed"
        report["observations"] = ["actual Observation macro expansion", "ordinary source/public model typecheck",
                                  "non-Sendable snapshot rejected", "unsynchronized MainActor call rejected"]
        (output / "result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({"result": "passed", "scope": report["scope"], "report": str(output / "result.json")}))
    except BaseException as error:
        report["result"] = "failed"
        report["error"] = str(error)
        (output / "result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        raise


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
