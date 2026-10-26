#!/usr/bin/env python3
"""L05 API26 development compile: two Release HARs and isolated HAP consumers.

Uses the C07 SDK profile/environment and bounded HAR reader. No installation,
network dependency resolution, native library, signing or device execution.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
from adapters.artifacts.arkts import har  # noqa: E402
from adapters.toolchains.arkts import toolchain, tools  # noqa: E402

SOURCES = (
    "sdk/arkts/facade/Boundary.ets.in",
    "sdk/arkts/facade/RuntimeFrame.ets",
    "sdk/arkts/native-gateway/Index.d.ts",
    "sdk/arkts/observation/StateObservation.ets",
    "sdk/arkts/observation/ArkUIStateModel.ets",
)
SUPPORT = (
    "scripts/check_arkts_observation.py", ".python-version",
    "adapters/artifacts/arkts/har.py", "adapters/artifacts/arkts/pack.py",
    "adapters/toolchains/arkts/toolchain.py", "adapters/toolchains/arkts/tools.py",
    "adapters/toolchains/arkts/preflight.py", "adapters/generators/arkts/support.py",
)
NATIVE = "libmadbc07sdkc.so"
NATIVE_DIR = "src/main/cpp/types/libmadbc07sdkc"
STORE = "oh_modules/.ohpm/observation-local"
LIMITS = [
    "API26 development probe, not a C04 candidate or C07/L05 certification",
    "Native declarations only; no native DSO, native loading/drain or env execution",
    "Unsigned HAP; no installation, device execution or UI notification proof",
    "Local declaration/HAR resolution is constructed without ohpm; registry install is untested",
    "Tool inventory is a caller-frozen local checksum, not a vendor signature",
    "Java launcher/version are recorded; the complete JBR/OS runtime is not closed",
]


def ref(path: Path) -> dict:
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"path": str(path), "size": path.stat().st_size, "sha256": digest}


def write(root: Path, name: str, value) -> None:
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    data = value if isinstance(value, bytes) else value.encode()
    with path.open("xb") as stream:
        stream.write(data)


def js(root: Path, name: str, value) -> None:
    write(root, name, json.dumps(value, indent=2) + "\n")


def environment(project: Path, cli: Path, java: Path) -> dict:
    for folder in ("home", "hvigor-home", "tmp", "npm-cache"):
        (project / folder).mkdir(parents=True)
    modules = project / "tool-node-modules/@ohos"
    modules.mkdir(parents=True)
    for name in ("hvigor", "hvigor-ohos-plugin"):
        (modules / name).symlink_to(cli / "hvigor" / name, target_is_directory=True)
    return {
        "PATH": os.pathsep.join([str(cli / "tool/node/bin"), str(java / "bin"),
                                 "/usr/bin", "/bin", "/usr/sbin", "/sbin"]),
        "HOME": str(project / "home"), "HVIGOR_USER_HOME": str(project / "hvigor-home"),
        "DEVECO_SDK_HOME": str(cli / "sdk"), "NODE_HOME": str(cli / "tool/node"),
        "JAVA_HOME": str(java), "TMPDIR": str(project / "tmp"),
        "NODE_PATH": str(project / "tool-node-modules"),
        "npm_config_cache": str(project / "npm-cache"),
        "LANG": "en_US.UTF-8", "LC_ALL": "en_US.UTF-8",
    }


def run(project: Path, label: str, argv: list, env: dict, receipt: dict,
        project_inputs: list[dict] | None = None) -> str:
    command = {"label": label, "cwd": str(project), "argv": list(map(str, argv)),
               "environment": env, "exit_code": None}
    if project_inputs is not None:
        command["project_inputs"] = project_inputs
    receipt["commands"].append(command)
    start = time.monotonic()
    try:
        result = subprocess.run(command["argv"], cwd=project, env=env,
                                capture_output=True, timeout=180)
        stdout, stderr = result.stdout, result.stderr
        command["exit_code"] = result.returncode
    except (OSError, subprocess.TimeoutExpired) as error:
        stdout = getattr(error, "stdout", None) or b""
        stderr = getattr(error, "stderr", None) or b""
        command["error"] = str(error)
    command["elapsed_seconds"] = round(time.monotonic() - start, 3)
    for suffix, data in (("stdout", stdout), ("stderr", stderr)):
        write(project, label + "." + suffix, data)
        command[suffix] = ref(project / (label + "." + suffix))
    if command["exit_code"] != 0:
        raise RuntimeError(label + " failed; see logs in " + str(project))
    return stdout.decode(errors="replace") + stderr.decode(errors="replace")


def project_files(project: Path, module: str, cli: Path) -> None:
    # The SDK26/Hvigor configuration follows adapters/artifacts/arkts/pack.py.
    js(project, "build-profile.json5", {
        "app": {"signingConfigs": [], "products": [{"name": "default",
            "targetSdkVersion": "26.0.0", "compatibleSdkVersion": "6.1.0(23)",
            "compileSdkVersion": "26.0.0", "runtimeOS": "HarmonyOS",
            "buildOption": {"strictMode": {"caseSensitiveCheck": True,
                                            "useNormalizedOHMUrl": True}}}],
            "buildModeSet": [{"name": "release"}]},
        "modules": [{"name": module, "srcPath": "./" + module,
                     "targets": [{"name": "default", "applyToProducts": ["default"]}]}]})
    js(project, "oh-package.json5", {"modelVersion": "6.26.8", "name": "l05-probe",
                                      "version": "1.0.0", "dependencies": {}})
    js(project, "hvigor/hvigor-config.json5", {"modelVersion": "6.26.8", "dependencies": {},
        "execution": {}, "logging": {}, "debugging": {}, "nodeOptions": {}})
    for path, task in (("hvigorfile.ts", "appTasks"),
                       (module + "/hvigorfile.ts", "harTasks" if module == "observation" else "hapTasks")):
        write(project, path, "import { " + task + " } from '@ohos/hvigor-ohos-plugin';\n"
              "export default { system: " + task + ", plugins: [] };\n")
    write(project, "local.properties", "sdk.dir=" + str(cli / "sdk") + "\n")
    js(project, "AppScope/app.json5", {"app": {"bundleName": "com.madb.l05probe",
        "vendor": "MadB", "versionCode": 1, "versionName": "1.0.0",
        "icon": "$media:app_icon", "label": "$string:app_name"}})
    js(project, "AppScope/resources/base/element/string.json",
       {"string": [{"name": "app_name", "value": "L05 compile probe"}]})
    write(project, "AppScope/resources/base/media/app_icon.svg",
          '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">'
          '<rect width="64" height="64" fill="#004488"/></svg>\n')
    js(project, module + "/build-profile.json5", {"apiType": "stageMode",
        "buildOption": {"arkOptions": {"obfuscation": {"ruleOptions": {
            "enable": True, "files": ["./obfuscation-rules.txt"]}}}},
        "targets": [{"name": "default"}]})


def local_resolution(project: Path, module: str, name: str, spec: str, store: str,
                     packages: dict | None = None) -> None:
    def row(label, deps):
        return {"name": label, "dependencies": deps, "devDependencies": {},
                "dynamicDependencies": {}, "maskedByOverrideDependencyMap": False}
    package = {"storePath": store, "dependencies": {}, "dynamicDependencies": {},
               "dev": False, "dynamic": False, "maskedByOverrideDependencyMap": False}
    js(project, "oh_modules/.ohpm/lock.json5", {"lockVersion": "1.0",
        "settings": {"resolveConflict": True, "resolveConflictStrict": False, "installAll": True},
        "overrides": {}, "overrideDependencyMap": {},
        "modules": {".": row("", {}), module: row(module, {name: {"specifier": spec, "version": spec}})},
        "packages": packages or {name + "@" + spec: package}})


def har_project(project: Path, ui: bool, cli: Path) -> None:
    project_files(project, "observation", cli)
    mod = project / "observation"
    js(mod, "src/main/module.json5", {"module": {"name": "observation", "type": "har",
                                                 "deviceTypes": ["phone", "tablet"]}})
    js(mod, "oh-package.json5", {"name": "observation", "version": "1.0.0", "main": "Index.ets",
                                 "dependencies": {NATIVE: "file:./" + NATIVE_DIR}})
    js(mod, NATIVE_DIR + "/oh-package.json5", {"name": NATIVE, "version": "1.0.0", "types": "./Index.d.ts"})
    write(mod, NATIVE_DIR + "/Index.d.ts", (REPO / SOURCES[2]).read_bytes())
    boundary = (REPO / SOURCES[0]).read_text()
    for marker, literal in (("@MADB_NATIVE_MODULE@", NATIVE),
                            ("@MADB_EXPECTED_IDENTITY@", "compile-only-l05-no-native-runtime")):
        if boundary.count(marker) != 1:
            raise ValueError("facade placeholder changed: " + marker)
        boundary = boundary.replace(marker, json.dumps(literal))
    write(mod, "src/main/ets/Boundary.ets", boundary)
    for name in (SOURCES[1], SOURCES[3], *((SOURCES[4],) if ui else ())):
        write(mod, "src/main/ets/" + Path(name).name, (REPO / name).read_bytes())
    write(mod, "Index.ets", "export { Engine, Snapshot } from './src/main/ets/Boundary';\n"
        "export { StateObservation, StateObservationError, ObservationPhase } from './src/main/ets/StateObservation';\n"
        + ("export { ArkUIStateModel } from './src/main/ets/ArkUIStateModel';\n" if ui else ""))
    write(mod, "obfuscation-rules.txt", "-enable-property-obfuscation\n-enable-export-obfuscation\n"
        "-enable-filename-obfuscation\n-keep-dts " + str(mod / NATIVE_DIR)
        + "\n-keep-global-name\nEngine\nSnapshot\nStateObservation\nStateObservationError\nObservationPhase\nArkUIStateModel\n"
        "-keep-property-name\nattach\nrequestStop\nclose\nphase\ncloseFailure\nsnapshot\nvalue\ntext\ncode\nobserveState\n"
        "-keep-file-name\nIndex\n")
    (mod / "oh_modules").mkdir()
    (mod / "oh_modules" / NATIVE).symlink_to("../" + NATIVE_DIR, target_is_directory=True)
    local_resolution(project, "observation", NATIVE, "file:observation/" + NATIVE_DIR,
                     "observation/" + NATIVE_DIR)


def consumer_project(project: Path, ui: bool, cli: Path, archive: Path, receipt: dict) -> None:
    project_files(project, "entry", cli)
    write(project, "vendor/observation.har", archive.read_bytes())
    package = project / STORE / "oh_modules/observation"
    members = har.read_members(archive.read_bytes())
    receipt["har_members"] = har.inventory(archive.read_bytes())
    for member in members:
        target = package / Path(member.path).relative_to("package")
        if member.data is None:
            target.mkdir(parents=True, exist_ok=True)
        else:
            write(package, str(target.relative_to(package)), member.data)
    declarations = "\n".join(m.data.decode() for m in members if m.path.endswith(".d.ets"))
    if ("ArkUIStateModel" in declarations) != ui:
        raise ValueError("plain/UI HAR declaration separation failed")
    if ui and "@ObservedV2" not in declarations:
        raise ValueError("ArkUI HAR lost ObservedV2 declaration")
    receipt["declaration_separation_passed"] = True
    (project / "entry/oh_modules").mkdir()
    (project / "entry/oh_modules/observation").symlink_to("../../" + STORE + "/oh_modules/observation", target_is_directory=True)
    (project / STORE / "oh_modules" / NATIVE).symlink_to("observation/" + NATIVE_DIR, target_is_directory=True)
    native_store = STORE + "/oh_modules/observation/" + NATIVE_DIR
    def item(store, dependencies):
        return {"storePath": store, "dependencies": dependencies, "dynamicDependencies": {},
                "dev": False, "dynamic": False, "maskedByOverrideDependencyMap": False}
    local_resolution(project, "entry", "observation", "file:vendor/observation.har", STORE, {
        "observation@file:vendor/observation.har": item(STORE, {NATIVE: native_store}),
        NATIVE + "@file:" + native_store: item(native_store, {})})
    js(project, "entry/oh-package.json5", {"name": "entry", "version": "1.0.0",
        "dependencies": {"observation": "file:../vendor/observation.har"}})
    js(project, "entry/src/main/module.json5", {"module": {"name": "entry", "type": "entry",
        "mainElement": "EntryAbility", "deviceTypes": ["phone", "tablet"],
        "deliveryWithInstall": True, "installationFree": False, "pages": "$profile:main_pages",
        "abilities": [{"name": "EntryAbility", "srcEntry": "./ets/EntryAbility.ets", "exported": False,
                       "icon": "$media:app_icon", "label": "$string:app_name",
                       "startWindowIcon": "$media:app_icon",
                       "startWindowBackground": "$color:window_background"}]}})
    js(project, "entry/src/main/resources/base/profile/main_pages.json", {"src": ["pages/Index"]})
    js(project, "entry/src/main/resources/base/element/color.json",
       {"color": [{"name": "window_background", "value": "#FFFFFF"}]})
    write(project, "entry/obfuscation-rules.txt", "-enable-property-obfuscation\n-enable-export-obfuscation\n"
        "-enable-filename-obfuscation\n-keep-global-name\nEntryAbility\n-keep-property-name\nvalue\ntext\ncode\n")
    body = ("const model: ArkUIStateModel = new ArkUIStateModel();\n    model.attach(engine);\n"
        "    const value: Snapshot | undefined = model.snapshot;\n    model.requestStop();\n    await model.close();\n"
        "    return value === undefined ? model.phase : value.text;" if ui else
        "const control: StateObservation = new StateObservation();\n    let text: string = '';\n"
        "    control.attach(engine, (value: Snapshot) => { text = value.text; });\n"
        "    control.requestStop();\n    await control.close();\n"
        "    const failure: Error | undefined = control.closeFailure;\n"
        "    return failure === undefined ? control.phase + text : failure.message;")
    write(project, "entry/src/main/ets/EntryAbility.ets", "import { UIAbility } from '@kit.AbilityKit';\n"
        "import { Engine, Snapshot, " + ("ArkUIStateModel" if ui else "StateObservation") + " } from 'observation';\n"
        "export default class EntryAbility extends UIAbility {\n"
        "  static async consume(engine: Engine): Promise<string> {\n    " + body + "\n  }\n}\n")
    write(project, "entry/src/main/ets/pages/Index.ets", (
        "import { ArkUIStateModel } from 'observation';\n@Entry\n@ComponentV2\nstruct Index {\n"
        "  @Local model: ArkUIStateModel = new ArkUIStateModel();\n"
        "  build() { Column() { Text(this.model.snapshot?.text ?? '') } }\n}\n" if ui else
        "@Entry\n@Component\nstruct Index {\n  build() { Column() { Text('L05 compile probe') } }\n}\n"))


def compile_project(project: Path, target: str, cli: Path, java: Path, receipt: dict) -> Path:
    env = environment(project, cli, java)
    inputs = [ref(p) for p in sorted(project.rglob('*')) if p.is_file() and not p.is_symlink()]
    output = run(project, target, [cli / "tool/node/bin/node", cli / "hvigor/hvigor/bin/hvigor.js",
        target, "--mode", "module", "-p", "product=default", "-p", "buildMode=release", "--no-daemon"],
        env, receipt, project_inputs=inputs)
    command = receipt["commands"][-1]
    command["task_summary"] = re.findall(r"\d+ tasks in total:.*", output)
    if "CompileArkTS" not in output or "BUILD SUCCESSFUL" not in output or not command["task_summary"]:
        raise ValueError("build lacks a real compiler stage/task result")
    module, name = ("observation", "observation.har") if target == "assembleHar" else ("entry", "entry-default-unsigned.hap")
    artifact = project / module / "build/default/outputs/default" / name
    command["artifact"] = ref(artifact)
    return artifact


def check(args, receipt: dict) -> None:
    cli, java, output = args.cli_root, args.java_home, args.output
    if sys.version.split()[0] != (REPO / ".python-version").read_text().strip():
        raise ValueError("use repository-pinned Python from .python-version")
    receipt["inputs"] = [ref(REPO / name) for name in (*SOURCES, *SUPPORT)] + [ref(args.cli_inventory)]
    inventory = json.loads(args.cli_inventory.read_text())
    if not isinstance(inventory, dict):
        raise ValueError("CLI inventory must be a JSON object")
    verified = tools.verify_inventory(inventory, cli)
    toolchain.check_distribution(verified["metadata"])
    receipt["toolchain"] = verified
    probe = output / "versions"
    env = environment(probe, cli, java)
    node = run(probe, "node-version", [cli / "tool/node/bin/node", "--version"], env, receipt).strip()
    java_version = run(probe, "java-version", [java / "bin/java", "-version"], env, receipt).strip()
    if node != "v24.14.1" or re.search(r'version "21\.', java_version) is None:
        raise ValueError("this probe requires bundled Node 24.14.1 and Java 21")
    receipt["versions"] = {"python": sys.version, "node": node, "java": java_version}
    receipt["host_images"] = [ref(Path(sys.executable).resolve()), ref(java / "bin/java")]
    for name, ui in (("plain", False), ("arkui", True)):
        producer = output / (name + "-har")
        har_project(producer, ui, cli)
        archive = compile_project(producer, "assembleHar", cli, java, receipt)
        archive_record = receipt["commands"][-1]
        consumer = output / (name + "-consumer")
        consumer_project(consumer, ui, cli, archive, archive_record)
        compile_project(consumer, "assembleHap", cli, java, receipt)
    for original in receipt["inputs"] + receipt["host_images"]:
        if ref(Path(original["path"])) != original:
            raise ValueError("input changed during compile: " + original["path"])
    tools.verify_inventory(inventory, cli)
    receipt["inputs_unchanged_after"] = True


def validate_output(output: Path, forbidden_roots: tuple[Path, ...]) -> None:
    """Read-only identity check, including case aliases on insensitive volumes."""
    roots = []
    for root in forbidden_roots:
        resolved = root.resolve(strict=True)
        if not resolved.is_dir():
            raise ValueError("protected root is not a directory: " + str(root))
        roots.append(resolved)
    if output.exists() or output.is_symlink():
        raise ValueError("--output must be a new directory")
    # resolve follows symlink ancestors; samefile checks the actual directory
    # identity when spelling/case differs from the selected protected root.
    resolved_output = output.resolve()
    for ancestor in resolved_output.parents:
        if ancestor.exists() and any(ancestor.samefile(root) for root in roots):
            raise ValueError("--output must be outside the repo and installed tool trees")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli-root", type=Path, required=True, help="installed formal mac-arm64 CLI 26.0.0.851")
    parser.add_argument("--cli-inventory", type=Path, required=True, help="caller-frozen C07 acquisition inventory JSON")
    parser.add_argument("--java-home", type=Path, required=True, help="existing Java 21 runtime directory")
    parser.add_argument("--output", type=Path, required=True, help="new directory; includes private caches and evidence")
    args = parser.parse_args()
    for name in ("cli_root", "cli_inventory", "java_home", "output"):
        setattr(args, name, getattr(args, name).absolute())
    try:
        validate_output(args.output, (REPO, args.cli_root, args.java_home))
    except (OSError, ValueError, RuntimeError) as error:
        parser.error(str(error))
    args.output.mkdir(parents=True)
    receipt = {"scope": "L05 API26 Release HAR and independent HAP development compilation",
               "status": "failed", "target_executed": False, "dependencies_installed": False,
               "commands": [], "known_limits": LIMITS}
    try:
        check(args, receipt)
    except (OSError, ValueError, RuntimeError, KeyError, TypeError, AttributeError,
            subprocess.SubprocessError) as error:
        receipt["error"] = str(error)
        js(args.output, "failure.json", receipt)
        print("L05 SDK compile failed: " + str(error), file=sys.stderr)
        return 1
    receipt["status"] = "passed"
    js(args.output, "success.json", receipt)
    print("L05 SDK compile passed: " + str(args.output / "success.json"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
