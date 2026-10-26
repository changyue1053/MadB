"""Release Observation checks; optional verified C06 development composition, not C10 certification."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def canonical(value) -> bytes:
    """The public integer-only canonical JSON codec, with UTF-16 key ordering."""
    if isinstance(value, dict):
        keys = sorted(value, key=lambda key: key.encode("utf-16-be"))
        return b"{" + b",".join(canonical(key) + b":" + canonical(value[key]) for key in keys) + b"}"
    if isinstance(value, list):
        return b"[" + b",".join(canonical(item) for item in value) + b"]"
    if isinstance(value, float):
        raise ValueError("floating-point metadata is outside the public digest subset")
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode()


def candidate_digest(manifest: dict) -> str:
    projection = {key: value for key, value in manifest.items() if key != "candidate_digest"}
    for name in ("files", "dependencies", "build_evidence"):
        projection[name] = sorted(projection[name], key=lambda item: item["path"])
    for name in ("public_symbols", "required_cases"):
        projection[name] = sorted(projection[name])
    return digest(b"madb.candidate/1\n" + canonical(projection))


def read_json(path: Path) -> dict:
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON key: " + key)
            result[key] = value
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique)


def verify_candidate(root: Path, expected_digest: str) -> dict:
    """Check public bytes against caller-supplied trust, without producer imports."""
    root = root.resolve(strict=True)
    manifest = read_json(root / "candidate.json")
    if not DIGEST.fullmatch(expected_digest):
        raise ValueError("expected candidate digest must be sha256:<64 lowercase hex digits>")
    if manifest.get("schema") != "madb.candidate/1" or manifest.get("manifest_path") != "candidate.json":
        raise ValueError("unsupported candidate manifest")
    if manifest.get("candidate_digest") != expected_digest or candidate_digest(manifest) != expected_digest:
        raise ValueError("candidate digest differs from the explicitly trusted candidate")
    selector = manifest["selector"]
    if (selector.get("route"), selector.get("target"), selector.get("direction")) != (
        "KN-SWIFT-OBJC", "aarch64-apple-darwin", "consumer-to-provider"
    ):
        raise ValueError("native check requires the C06 macOS Arm64 SKIE route")
    if manifest["dependencies"]:
        raise ValueError("this local composition does not resolve external candidate dependencies")
    files = {}
    for entry in manifest["files"]:
        name = entry["path"]
        path = PurePosixPath(name)
        if (not isinstance(name, str) or path.is_absolute() or path.as_posix() != name
                or any(part in ("", ".", "..") for part in name.split("/"))
                or "\\" in name or name == "candidate.json" or name in files):
            raise ValueError("invalid or duplicate candidate path: " + str(name))
        if not DIGEST.fullmatch(entry["sha256"]) or type(entry["size"]) is not int or entry["size"] < 0:
            raise ValueError("invalid candidate file digest/size: " + name)
        files[name] = entry
    actual = set()
    for path in root.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("candidate contains a symlink or special file")
        if path.is_file():
            actual.add(path.relative_to(root).as_posix())
    if actual != set(files) | {"candidate.json"}:
        raise ValueError("candidate inventory differs from public files")
    for name, entry in files.items():
        data = (root / name).read_bytes()
        if len(data) != entry["size"] or digest(data) != entry["sha256"]:
            raise ValueError("candidate file bytes differ: " + name)
    for entry in manifest["build_evidence"]:
        if any(files.get(entry["path"], {}).get(field) != entry[field] for field in ("sha256", "size")):
            raise ValueError("build evidence is outside the verified inventory")
    identity = read_json(root / "identity.json")
    if identity.get("schema") != "madb.native-identity/1" or type(identity.get("bootstrap_version")) is not int or identity.get("bootstrap_version") != 1:
        raise ValueError("unsupported native identity")
    if any(not DIGEST.fullmatch(identity.get(field, "")) for field in ("profile_digest", "interface_digest")):
        raise ValueError("invalid native identity digests")
    if identity["profile_digest"] != selector["profile_digest"]:
        raise ValueError("native identity differs from the selected profile")
    for name in ("framework/MadBSkie.framework/MadBSkie", "swift/MadBSkieConcurrency.swift", "toolchain.json"):
        if name not in files:
            raise ValueError("missing required public artifact: " + name)
    return {"candidate_digest": expected_digest, "identity": identity, "selector": selector,
            "verified_files": len(files), "files": files}


class Commands:
    def __init__(self, output: Path, report: dict):
        self.output = output
        self.report = report
        self.environment = os.environ.copy()
        for key in tuple(self.environment):
            if key.startswith("DYLD_") or key in {"SDKROOT", "SWIFT_EXEC", "SWIFT_DRIVER_SWIFT_FRONTEND_EXEC"}:
                del self.environment[key]
        self.environment["CLANG_MODULE_CACHE_PATH"] = str(output / "clang-cache")
        self.environment["SWIFTPM_MODULECACHE_OVERRIDE"] = str(output / "manifest-cache")
        self.environment["TMPDIR"] = str(output / "tmp")
        (output / "tmp").mkdir()

    def run(self, command, cwd: Path, timeout: int = 240) -> str:
        command = list(map(str, command))
        print("+ " + " ".join(command), flush=True)
        result = subprocess.run(command, cwd=cwd, env=self.environment, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        log = self.output / f"command-{len(self.report['commands']):02d}.log"
        log.write_text(result.stdout + result.stderr)
        self.report["commands"].append({"argv": command, "cwd": str(cwd),
                                        "exit_code": result.returncode, "log": log.name})
        if result.returncode:
            raise RuntimeError(f"command failed ({result.returncode}); see {log}\n" + (result.stdout + result.stderr)[-10000:])
        return result.stdout

    def swiftpm(self, swift: Path, verb: str, package: Path, scratch: Path, extra=()) -> str:
        return self.run([swift, verb, "--package-path", package, "--disable-sandbox",
                         "--scratch-path", scratch, "--cache-path", self.output / "spm-cache",
                         "--config-path", self.output / "spm-config", "--security-path", self.output / "spm-security",
                         "--manifest-cache", "local", *extra], package)


def toolchain(commands: Commands, root: Path, candidate: Path | None) -> tuple[Path, Path]:
    expected = read_json(root / "adapters/toolchains/kotlin_rust/toolchain.json")["xcode"]
    if candidate is not None and read_json(candidate / "toolchain.json")["xcode"] != expected:
        raise ValueError("candidate and current managed Apple toolchain locks differ")
    swift = Path(commands.run(["xcrun", "--find", "swift"], root).strip())
    version = commands.run([swift, "--version"], root).strip()
    xcode = commands.run(["xcodebuild", "-version"], root).strip()
    sdk_version = commands.run(["xcrun", "--sdk", "macosx", "--show-sdk-version"], root).strip()
    sdk_build = commands.run(["xcrun", "--sdk", "macosx", "--show-sdk-build-version"], root).strip()
    sdk = Path(commands.run(["xcrun", "--sdk", "macosx", "--show-sdk-path"], root).strip())
    if (expected["swift"] not in version or xcode != f"Xcode {expected['version']}\nBuild version {expected['build']}"
            or sdk_version != expected["sdk_version"] or sdk_build != expected["sdk_build"]):
        raise ValueError("selected Apple toolchain differs from the managed lock")
    commands.report["toolchain"] = {"swift": version, "xcode": xcode, "sdk": str(sdk),
                                     "sdk_version": sdk_version, "sdk_build": sdk_build}
    return swift, sdk


def copy_sources(root: Path, output: Path) -> tuple[Path, list[dict]]:
    package = root / "sdk/swift/observation"
    selected = [path for path in package.rglob("*") if path.is_file()
                and not any(part in {".build", ".swiftpm", ".git"} for part in path.relative_to(package).parts)]
    selected += list((root / "tests/integration/l04_swift_observation").glob("*Consumer.swift"))
    records = []
    for path in sorted(selected):
        if path.is_symlink():
            raise ValueError("extension/consumer source must not be a symlink")
        relative = path.relative_to(package) if path.is_relative_to(package) else Path("consumers") / path.name
        destination = output / "observation" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        data = path.read_bytes()
        destination.write_bytes(data)
        records.append({"path": path.relative_to(root).as_posix(), "sha256": digest(data), "size": len(data)})
    return output / "observation", records


def package_checks(commands: Commands, swift: Path, package: Path) -> None:
    metadata = json.loads(commands.swiftpm(swift, "package", package, commands.output / "package-build", ["dump-package"]))
    if metadata["dependencies"]:
        raise ValueError("Observation package must remain dependency-free")
    commands.swiftpm(swift, "test", package, commands.output / "package-build",
                     ["--configuration", "release", "-Xswiftc", "-warnings-as-errors", "-Xswiftc", "-strict-concurrency=complete"])
    commands.report["package_release"] = "passed"
    consumer = commands.output / "package-consumer"
    (consumer / "Sources/Consumer").mkdir(parents=True)
    (consumer / "Package.swift").write_text('''// swift-tools-version: 6.0
import PackageDescription
let package = Package(name: "Consumer", platforms: [.macOS(.v14)],
    dependencies: [.package(name: "MadBObservation", path: "../observation")],
    targets: [.executableTarget(name: "Consumer", dependencies: [.product(name: "MadBObservation", package: "MadBObservation")])])
''')
    shutil.copyfile(package / "consumers/PackageConsumer.swift", consumer / "Sources/Consumer/Consumer.swift")
    output = commands.swiftpm(swift, "run", consumer, commands.output / "consumer-build",
                             ["--configuration", "release", "-Xswiftc", "-warnings-as-errors",
                              "-Xswiftc", "-strict-concurrency=complete", "Consumer"])
    if "MADB_OBSERVATION_PACKAGE_CONSUMER_OK" not in output:
        raise RuntimeError("external package consumer did not report successful assertions")
    commands.report["external_package_consumer"] = "passed"


def native_check(commands: Commands, swift: Path, sdk: Path, package: Path, candidate: Path, verified: dict) -> None:
    consumer = commands.output / "native-consumer"
    consumer.mkdir()
    public = [name for name in verified["files"] if name.startswith("framework/MadBSkie.framework/")
              or name == "swift/MadBSkieConcurrency.swift"]
    for name in public:
        target = consumer / name
        target.parent.mkdir(parents=True, exist_ok=True)
        data = (candidate / name).read_bytes()
        if digest(data) != verified["files"][name]["sha256"]:
            raise ValueError("candidate changed after verification: " + name)
        target.write_bytes(data)
    shutil.copyfile(package / "Integration/SKIE/MadBSkieStateSource.swift", consumer / "MadBSkieStateSource.swift")
    shutil.copyfile(package / "consumers/SkieConsumer.swift", consumer / "Consumer.swift")
    library = consumer / "lib"
    library.mkdir()
    swiftc = swift.with_name("swiftc")
    common = ["-swift-version", "6", "-strict-concurrency=complete", "-warnings-as-errors", "-O",
              "-parse-as-library", "-target", "arm64-apple-macosx14.0", "-sdk", sdk,
              "-module-cache-path", commands.output / "native-module-cache"]
    sources = sorted((package / "Sources/MadBObservation").glob("*.swift"))
    commands.run([swiftc, *common, "-emit-library", "-emit-module", "-module-name", "MadBObservation",
                  "-emit-module-path", library / "MadBObservation.swiftmodule", *sources,
                  "-o", library / "libMadBObservation.dylib"], consumer)
    binary = consumer / "consumer"
    commands.run([swiftc, *common, "-I", library, "-L", library, "-lMadBObservation",
                  "-F", consumer / "framework", "-framework", "MadBSkie",
                  "-Xlinker", "-rpath", "-Xlinker", "@executable_path/lib",
                  "-Xlinker", "-rpath", "-Xlinker", "@executable_path/framework",
                  consumer / "swift/MadBSkieConcurrency.swift", consumer / "MadBSkieStateSource.swift",
                  consumer / "Consumer.swift", "-o", binary], consumer)
    identity = verified["identity"]
    output = commands.run([binary, identity["profile_digest"], identity["interface_digest"]], consumer, timeout=45)
    if "MADB_OBSERVATION_SKIE_CONSUMER_OK" not in output:
        raise RuntimeError("native consumer did not report successful assertions")
    commands.report["native_composition"] = "passed"
    commands.report["copied_candidate_artifacts"] = public


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True, help="new directory for isolated inputs, builds, logs and report")
    parser.add_argument("--candidate", type=Path, help="optional frozen public C06 SKIE candidate; missing paths fail")
    parser.add_argument("--expected-candidate-digest", help="independently trusted digest, required with --candidate")
    args = parser.parse_args()
    if bool(args.candidate) != bool(args.expected_candidate_digest):
        parser.error("--candidate and --expected-candidate-digest must be supplied together")
    report = {"scope": "L04 Observation local development composition; not a new C10 candidate",
              "package_release": "not_run", "external_package_consumer": "not_run",
              "native_composition": "not_requested" if args.candidate is None else "not_run",
              "checker_sha256": digest(Path(__file__).read_bytes()), "commands": []}
    output = args.output.resolve()
    try:
        output.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        print(f"Observation checks cannot create a fresh output directory: {error}", file=sys.stderr)
        return 1
    try:
        root = args.root.resolve(strict=True)
        candidate = args.candidate.resolve(strict=True) if args.candidate else None
        verified = verify_candidate(candidate, args.expected_candidate_digest) if candidate else None
        if verified:
            report["base_candidate"] = {key: value for key, value in verified.items() if key != "files"}
        commands = Commands(output, report)
        swift, sdk = toolchain(commands, root, candidate)
        package, report["source_inputs"] = copy_sources(root, output)
        package_checks(commands, swift, package)
        if verified:
            native_check(commands, swift, sdk, package, candidate, verified)
        report["result"] = "passed"
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        report["result"] = "failed"
        report["error"] = str(error)
        print(f"Observation checks failed: {error}", file=sys.stderr)
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"Observation checks {report['result']}; native={report['native_composition']}; report={output / 'report.json'}")
    return 0 if report["result"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
