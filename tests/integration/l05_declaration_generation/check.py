"""L05 declaration projection: real five-stage source, generation and SDK builds."""
from pathlib import Path
import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts.value_declarations import generate

spec = importlib.util.spec_from_file_location("l05_build", ROOT / "scripts/check_arkts_observation.py")
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)
SOURCES = ("adapters/generators/arkts/read_facade.cjs", "adapters/generators/arkts/value_declarations.py",
           "tests/integration/l05_declaration_generation/check.cjs",
           "tests/integration/l05_declaration_generation/check.py",
           "tests/integration/l05_declaration_generation/README.md",
           "docs/examples/language-api/second-business.ets", "scripts/check_arkts_observation.py",
           "sdk/arkts/facade/Boundary.ets.in",
           "tests/fixtures/c07_arkts_generation/rename-record/Boundary.ets.in",
           "tests/fixtures/c07_arkts_generation/reject-async/Boundary.ets.in",
           "tests/fixtures/c07_arkts_generation/reject-async-listener/Boundary.ets.in",
           "tests/fixtures/c07_arkts_generation/reject-callback-result/Boundary.ets.in",
           ".python-version", ".node-version")
STAGES = ("initial", "add-function", "add-field", "rename", "delete-function")
STORE = "oh_modules/.ohpm/business-local"


def command(cwd, label, argv, receipt, *, env=None, expected_failure=False):
    entry = {"label": label, "cwd": str(cwd), "argv": list(map(str, argv))}
    receipt["commands"].append(entry)
    result = subprocess.run(entry["argv"], cwd=cwd, env=env, capture_output=True, timeout=240)
    entry["exit_code"] = result.returncode
    for kind in ("stdout", "stderr"):
        build.write(cwd, label + "." + kind, getattr(result, kind))
        entry[kind] = build.ref(cwd / (label + "." + kind))
    text = (result.stdout + result.stderr).decode(errors="replace")
    if expected_failure:
        if result.returncode == 0 or "EntryAbility.ets" not in text or re.search(r"has no exported member[^\n]*describeReading", text) is None:
            raise ValueError("deleted-symbol consumer did not receive its exact missing-export failure")
    elif result.returncode:
        raise ValueError(label + " failed; inspect retained compiler logs")
    return text


def compile_sdk(project, task, args, receipt, *, deleted=False):
    environment = build.environment(project, args.cli_root, args.java_home)
    # Keep the real home; explicit Hvigor/npm/temp locations isolate build writes.
    environment["HOME"] = os.environ["HOME"]
    text = command(project, task, [args.cli_root / "tool/node/bin/node",
        args.cli_root / "hvigor/hvigor/bin/hvigor.js", task, "--mode", "module",
        "-p", "product=default", "-p", "buildMode=release", "--no-daemon"], receipt,
        env=environment, expected_failure=deleted)
    if deleted:
        return None
    if "CompileArkTS" not in text or "BUILD SUCCESSFUL" not in text:
        raise ValueError("missing actual ArkTS compile completion")
    artifact = project / ("observation/build/default/outputs/default/observation.har" if task == "assembleHar"
                          else "entry/build/default/outputs/default/entry-default-unsigned.hap")
    receipt["commands"][-1]["artifact"] = build.ref(artifact)
    return artifact


def producer(project, outputs, cli):
    build.project_files(project, "observation", cli)
    mod = project / "observation"
    build.js(mod, "src/main/module.json5", {"module": {"name": "observation", "type": "har", "deviceTypes": ["phone"]}})
    build.js(mod, "oh-package.json5", {"name": "business", "version": "1.0.0", "main": "Index.ets", "dependencies": {}})
    build.write(mod, "obfuscation-rules.txt", "")
    for name in ("Business.ets", "Index.ets"):
        build.write(mod, name, outputs[name])


def ordinary_call(stage):
    renamed = stage in ("rename", "delete-function")
    model, normalize = ("Measurement", "normalizeReading") if renamed else ("Reading", "normalize")
    extra = "describeReading" if renamed else "describe"
    has_extra = stage not in ("initial", "delete-function")
    names = model + ", " + normalize + (", " + extra if has_extra else "")
    revision = ", revision: 3" if stage in ("add-field", "rename", "delete-function") else ""
    body = "const input: " + model + " = { value: 7, label: ' hello '" + revision + " };\n"
    body += "const result: " + model + " = " + normalize + "(input);\n"
    if revision:
        body += "const revision: number = result.revision;\n"
    body += "return " + (extra + "(result)" if has_extra else "result.label") + ";"
    return names, body


def declaration_shape(descriptor):
    # The SDK may rename positional parameters in obfuscated .d.ets files.
    return {"records": [(r["name"], [(f["name"], f["type"]) for f in r["fields"]]) for r in descriptor["records"]],
            "functions": [(f["name"], [p["type"] for p in f["params"]], f["returns"]) for f in descriptor["functions"]]}


def consumer(project, archive, stage, cli, *, deleted=False):
    build.project_files(project, "entry", cli)
    build.write(project, "vendor/business.har", archive.read_bytes())
    package = project / STORE / "oh_modules/business"
    members = build.har.read_members(archive.read_bytes())
    for member in members:
        relative = Path(member.path).relative_to("package")
        if member.data is None:
            (package / relative).mkdir(parents=True, exist_ok=True)
        else:
            build.write(package, str(relative), member.data)
    (project / "entry/oh_modules").mkdir()
    (project / "entry/oh_modules/business").symlink_to("../../" + STORE + "/oh_modules/business", target_is_directory=True)
    build.local_resolution(project, "entry", "business", "file:vendor/business.har", STORE)
    build.js(project, "entry/oh-package.json5", {"name": "entry", "version": "1.0.0", "dependencies": {"business": "file:../vendor/business.har"}})
    build.js(project, "entry/src/main/module.json5", {"module": {"name": "entry", "type": "entry", "mainElement": "EntryAbility",
        "deviceTypes": ["phone"], "deliveryWithInstall": True, "installationFree": False, "pages": "$profile:main_pages",
        "abilities": [{"name": "EntryAbility", "srcEntry": "./ets/EntryAbility.ets", "exported": False,
                       "icon": "$media:app_icon", "label": "$string:app_name", "startWindowIcon": "$media:app_icon",
                       "startWindowBackground": "$color:window_background"}]}})
    build.js(project, "entry/src/main/resources/base/profile/main_pages.json", {"src": ["pages/Index"]})
    build.js(project, "entry/src/main/resources/base/element/color.json", {"color": [{"name": "window_background", "value": "#FFFFFF"}]})
    build.write(project, "entry/obfuscation-rules.txt", "")
    names, body = ordinary_call(stage)
    if deleted:
        names += ", describeReading"
    build.write(project, "entry/src/main/ets/EntryAbility.ets", "import { UIAbility } from '@kit.AbilityKit';\n"
        "import { " + names + " } from 'business';\nexport default class EntryAbility extends UIAbility {\n"
        "  static consume(): string {\n" + body + "\n  }\n}\n")
    build.write(project, "entry/src/main/ets/pages/Index.ets", "@Entry\n@Component\nstruct Index {\n"
        "  build() { Column() { Text('Value declaration consumer') } }\n}\n")


def check(args, receipt):
    if sys.version.split()[0] != (ROOT / ".python-version").read_text().strip():
        raise ValueError("use repository-pinned Python")
    receipt["inputs"] = [build.ref(ROOT / name) for name in sorted(set(SOURCES + build.SUPPORT))] + [build.ref(args.cli_inventory), build.ref(args.typescript)]
    receipt["inputs"] += [build.ref(Path(sys.executable).resolve()),
                          build.ref(Path(shutil.which("node")).resolve()), build.ref(args.java_home / "bin/java")]
    if build.ref(args.typescript)["sha256"] != args.typescript_sha256:
        raise ValueError("TypeScript module differs from external digest")
    inventory = json.loads(args.cli_inventory.read_text())
    verified = build.tools.verify_inventory(inventory, args.cli_root)
    build.toolchain.check_distribution(verified["metadata"])
    receipt["toolchain"] = verified
    node = command(args.output, "bundled-node-version", [args.cli_root / "tool/node/bin/node", "--version"], receipt).strip()
    java = command(args.output, "java-version", [args.java_home / "bin/java", "-version"], receipt).strip()
    if node != "v24.14.1" or re.search(r'version "21\.', java) is None:
        raise ValueError("formal build needs bundled Node24.14.1 and Java21")
    receipt["versions"] = {"python": sys.version, "bundled_node": node, "java": java}
    stage_root = args.output / "authored-inputs"
    command(args.output, "read-declarations", ["node", Path(__file__).with_suffix(".cjs"), args.typescript,
            args.typescript_sha256, stage_root], receipt)
    receipt["reader"] = json.loads((stage_root / "reader-checks.json").read_text())
    receipt["stages"] = []
    for stage in STAGES:
        authored = stage_root / stage
        source = (authored / "Business.ets").read_bytes()
        descriptor = json.loads((authored / "descriptor.json").read_text())
        outputs = generate(descriptor, source)
        if outputs != generate(descriptor, source):
            raise ValueError("non-deterministic declaration generation")
        corrupt = dict(descriptor, source=dict(descriptor["source"], size=len(source) + 1))
        try:
            generate(corrupt, source)
        except ValueError:
            pass
        else:
            raise ValueError("source identity mismatch was accepted")
        generated = args.output / "generated" / stage
        for name, data in outputs.items():
            build.write(generated, name, data)
        expected = next(value for value in receipt["reader"]["stages"] if value["name"] == stage)
        declarations = outputs["Index.d.ts"].decode()
        if re.findall(r"export declare function (\w+)", declarations) != expected["functions"]:
            raise ValueError("generated callable surface differs from independent expectation")
        if re.findall(r"export interface (\w+)", declarations) != [expected["record"]]:
            raise ValueError("generated record differs from independent expectation")
        if re.findall(r"^  (\w+):", declarations, re.M) != expected["fields"]:
            raise ValueError("generated fields differ from independent expectation")
        record = {"name": stage, "generated": [build.ref(generated / name) for name in outputs]}
        receipt["stages"].append(record)
        names, body = ordinary_call(stage)
        build.write(generated, "Consumer.ts", "import { " + names + " } from './Index';\nexport function consume(): string {\n" + body + "\n}\n")
        checker = ["node", Path(__file__).with_suffix(".cjs"), args.typescript, args.typescript_sha256]
        command(generated, "generated-typecheck", checker + [generated, "typecheck"], receipt)
        generated_descriptor = json.loads(command(generated, "generated-declarations",
            checker + [generated / "Index.d.ts", "declaration"], receipt))
        if declaration_shape(generated_descriptor) != declaration_shape(descriptor):
            raise ValueError("generated declaration shape changed source types")
        har_root = args.output / "builds" / (stage + "-har")
        producer(har_root, outputs, args.cli_root)
        archive = compile_sdk(har_root, "assembleHar", args, receipt)
        record["har"] = build.ref(archive)
        hap_root = args.output / "builds" / (stage + "-consumer")
        consumer(hap_root, archive, stage, args.cli_root)
        compiled_declaration = hap_root / STORE / "oh_modules/business/Business.d.ets"
        compiled_descriptor = json.loads(command(generated, "compiled-declarations",
            checker + [compiled_declaration, "declaration"], receipt))
        if declaration_shape(compiled_descriptor) != declaration_shape(descriptor):
            raise ValueError("actual HAR compiled declarations changed source types")
        record["compiled_declaration"] = build.ref(compiled_declaration)
        record["source_generated_compiled_shapes_match"] = True
        record["hap"] = build.ref(compile_sdk(hap_root, "assembleHap", args, receipt))
        if stage == "delete-function":
            bad_root = args.output / "builds" / "deleted-symbol-consumer"
            consumer(bad_root, archive, stage, args.cli_root, deleted=True)
            compile_sdk(bad_root, "assembleHap", args, receipt, deleted=True)
            record["deleted_symbol_rejected"] = True
    for original in receipt["inputs"]:
        if build.ref(Path(original["path"])) != original:
            raise ValueError("source/tool changed during check")
    build.tools.verify_inventory(inventory, args.cli_root)
    receipt["inputs_unchanged_after"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("cli-root", "cli-inventory", "java-home", "typescript", "output"):
        parser.add_argument("--" + flag, type=Path, required=True)
    parser.add_argument("--typescript-sha256", required=True)
    args = parser.parse_args()
    for name in ("cli_root", "cli_inventory", "java_home", "typescript", "output"):
        setattr(args, name, getattr(args, name).absolute())
    build.validate_output(args.output, (ROOT, args.cli_root, args.java_home, args.typescript.parent))
    args.output.mkdir(parents=True)
    receipt = {"scope": "L05 real value-declaration projection and formal SDK compilation only", "status": "failed",
               "commands": [], "target_executed": False, "native_backend": False,
               "limits": ["No numeric ABI inference or cross-language provider generation", "No C04 committed Source or formal candidate",
                          "No device execution, C07/L05 certification, registry installation or release"]}
    try:
        check(args, receipt)
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        receipt["error"] = str(error)
        build.js(args.output, "failure.json", receipt)
        print(str(error), file=sys.stderr)
        return 1
    receipt["status"] = "passed"
    build.js(args.output, "success.json", receipt)
    print(args.output / "success.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
