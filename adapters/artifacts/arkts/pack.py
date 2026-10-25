"""C04 HAR Package port: real Release compiler, then fixed native/public checks."""

from __future__ import annotations
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

_spec = importlib.util.spec_from_file_location(
    "_c07_port", Path(__file__).resolve().parents[3] / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(support)


def native_inputs(inputs, refs):
    """Every native/ member and any ELF byte stream must belong to the HAR set."""
    native = {}
    for r in refs:
        source = support.input_path(inputs, r["path"])
        with source.open("rb") as f:
            is_elf = f.read(4) == b"\x7fELF"
        if r["path"].startswith("native/"):
            basename = Path(r["path"]).name
            if (
                len(Path(r["path"]).parts) != 2
                or not re.fullmatch(r"lib[A-Za-z0-9_+.\-]+\.so", basename)
                or not is_elf
            ):
                raise ValueError("unsupported native Build member: " + r["path"])
            dest = "package/libs/arm64-v8a/" + basename
            if dest in native:
                raise ValueError("duplicate native package library")
            native[dest] = r
        elif is_elf:
            raise ValueError("ELF Build member outside native inventory: " + r["path"])
    return native


def run_har_command(label, args, project, work, env, commands, timeout=900):
    argv = list(map(str, args))
    out = b""
    err = b""
    receipt = {"label": label, "arguments": argv, "exit_code": None}
    try:
        result = subprocess.run(
            argv, cwd=project, env=env, capture_output=True, timeout=timeout
        )
        out = result.stdout
        err = result.stderr
        receipt["exit_code"] = result.returncode
    except (OSError, subprocess.TimeoutExpired) as error:
        out = getattr(error, "stdout", None) or b""
        err = getattr(error, "stderr", None) or b""
        receipt["error"] = str(error)
    receipt.update(
        stdout_bytes=len(out),
        stderr_bytes=len(err),
        output_truncated=len(out) > 16 * support.MAX_JSON or len(err) > support.MAX_JSON,
    )
    (work / (label + ".stdout")).write_bytes(out[: 16 * support.MAX_JSON])
    (work / (label + ".stderr")).write_bytes(err[: support.MAX_JSON])
    (work / (label + "-command.json")).write_text(json.dumps(receipt, indent=2) + "\n")
    if "error" in receipt:
        raise RuntimeError("HAR " + label + " could not complete: " + receipt["error"])
    if receipt["output_truncated"]:
        raise ValueError("HAR " + label + " output exceeded budget; partial output retained")
    if receipt["exit_code"]:
        raise RuntimeError("HAR " + label + " failed: " + err.decode(errors="replace")[-12000:])
    commands.append(
        {
            "label": label,
            "command": [a.replace(str(project), "$PACKAGE_WORK") for a in argv],
            "exit_code": 0,
        }
    )


def main():
    raw = sys.stdin.buffer.read(support.MAX_JSON + 1)
    if len(raw) > support.MAX_JSON:
        raise ValueError("PackageRequest exceeds budget")
    request = json.loads(raw)
    stage = Path.cwd()
    inputs = stage.parent / "inputs"
    # Scratch is retained on error, removed before publishing a successful result.
    work = Path(tempfile.mkdtemp(prefix="private-package-", dir=stage))
    if request["schema"] != "madb.package-request/1":
        raise ValueError("expected C04 PackageRequest")
    config = support.read_json(support.input_path(inputs, "backend.json"))
    if config["schema"] != "madb.private.c07-native-backend/1":
        raise ValueError("unsupported C07 package configuration")
    d = support.load_module(inputs / "adapters/toolchains/common/documents.py")
    har = support.load_module(inputs / "adapters/artifacts/arkts/har.py")
    # har_inputs uses the existing adapter import, never a parallel tar parser.
    sys.path.insert(0, str(inputs))
    bind = support.load_module(Path(__file__).with_name("har_inputs.py"))
    public = support.load_module(Path(__file__).with_name("har_public.py"))
    verifier = support.load_module(inputs / "adapters/toolchains/arkts/tools.py")
    cli = Path(config["cli_root"])
    inventory = support.read_json(
        support.input_path(inputs, config["cli_inventory"]), 64 * support.MAX_JSON
    )
    verifier.verify_inventory(inventory, cli)
    for t in config["tools"].values():
        support.verify_tool(t)
    support.load_module(
        Path(__file__).resolve().parents[3] / "adapters/toolchains/arkts/toolchain.py"
    ).verify_runtime(config, inputs)
    refs = support.read_json(support.input_path(inputs, "package-files.json"))
    support.verify_refs(inputs, refs)
    known = {r["path"]: r for r in refs}
    for evidence_ref in request["build_evidence"]:
        support.verify_refs(inputs, [evidence_ref])
        evidence = support.read_json(support.input_path(inputs, evidence_ref["path"]))
        for log in evidence["logs"]:
            if log["path"] in known and known[log["path"]] != log:
                raise ValueError("Build log path has conflicting identities")
            support.verify_refs(inputs, [log])
            if log["path"] not in known:
                refs.append(log)
                known[log["path"]] = log
    names = {r["path"] for r in refs}
    required = {
        "identity.json",
        "declarations.json",
        "generated/facade/Boundary.ets",
        "generated/facade/RuntimeFrame.ets",
        "generated/native/Index.d.ts",
        "generated/keep-rules.txt",
    }
    if not required <= names:
        raise ValueError("HAR is missing frozen Source parent products")
    descriptor = support.read_json(support.input_path(inputs, "declarations.json"))
    module = config["package_name"]
    native_module = config["native_module"]
    native_package = native_module.removesuffix(".so")
    if native_module != "lib" + module + ".so" or not re.fullmatch(
        r"madbc07sdk(?:c|cpp|rust|kotlin)(?:test)?", module
    ):
        raise ValueError("native package/module identity differs from the frozen route")
    additional_notices = config.get("additional_notices", [])
    if any(
        not name.startswith("notices/" + config["provider"] + "/")
        for name in additional_notices
    ):
        raise ValueError("unsupported additional runtime notice path")
    additional_assets = [
        "THIRD_PARTY_NOTICES/" + name.removeprefix("notices/") for name in additional_notices
    ]
    project = work / "har-project"
    project.mkdir()
    mod = project / module
    commands = []

    def write(name, value):
        path = project / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as f:
            f.write(value.encode() if isinstance(value, str) else value)

    def js(name, value):
        write(name, json.dumps(value, indent=2) + "\n")

    def copied(source, name):
        write(name, support.input_path(inputs, source).read_bytes())

    js(
        "build-profile.json5",
        {
            "app": {
                "signingConfigs": [],
                "products": [
                    {
                        "name": "default",
                        "targetSdkVersion": "26.0.0",
                        "compatibleSdkVersion": "6.1.0(23)",
                        "runtimeOS": "HarmonyOS",
                        "buildOption": {
                            "strictMode": {
                                "caseSensitiveCheck": True,
                                "useNormalizedOHMUrl": True,
                            }
                        },
                        "compileSdkVersion": "26.0.0",
                    }
                ],
                "buildModeSet": [{"name": "release"}],
            },
            "modules": [
                {
                    "name": module,
                    "srcPath": "./" + module,
                    "targets": [{"name": "default", "applyToProducts": ["default"]}],
                }
            ],
        },
    )
    js(
        "oh-package.json5",
        {
            "modelVersion": "6.26.8",
            "name": "madb-c07-c-package",
            "version": "0.0.1",
            "dependencies": {},
        },
    )
    js(
        "hvigor/hvigor-config.json5",
        {
            "modelVersion": "6.26.8",
            "dependencies": {},
            "execution": {},
            "logging": {},
            "debugging": {},
            "nodeOptions": {},
        },
    )
    write(
        "hvigorfile.ts",
        "import { appTasks } from '@ohos/hvigor-ohos-plugin';\nexport default { system: appTasks, plugins: [] };\n",
    )
    js(
        "AppScope/app.json5",
        {
            "app": {
                "bundleName": "com.madb.c07sdk",
                "vendor": "MadB",
                "versionCode": 1,
                "versionName": "0.0.1",
            }
        },
    )
    write("local.properties", "sdk.dir=" + str(cli / "sdk") + "\n")
    write(
        module + "/hvigorfile.ts",
        "import { harTasks } from '@ohos/hvigor-ohos-plugin';\nexport default { system: harTasks, plugins: [] };\n",
    )
    js(
        module + "/build-profile.json5",
        {
            "apiType": "stageMode",
            "targets": [{"name": "default"}],
            "buildOption": {
                "packingOptions": {
                    "asset": {
                        "include": [
                            "THIRD_PARTY_NOTICES/native-NOTICE.txt",
                            "THIRD_PARTY_NOTICES/llvm-NOTICE.txt",
                            "LICENSE",
                            "NOTICE",
                            *additional_assets,
                        ]
                    }
                },
                "nativeLib": {"debugSymbol": {"strip": False}},
                "arkOptions": {
                    "obfuscation": {
                        "ruleOptions": {"enable": True, "files": ["./obfuscation-rules.txt"]}
                    }
                },
            },
        },
    )
    js(
        module + "/src/main/module.json5",
        {"module": {"name": module, "type": "har", "deviceTypes": ["phone", "tablet"]}},
    )
    local = "src/main/cpp/types/" + native_package
    js(
        module + "/oh-package.json5",
        {
            "name": module,
            "version": "0.0.1",
            "main": "Index.ets",
            "dependencies": {native_module: "file:./" + local},
        },
    )
    js(
        module + "/" + local + "/oh-package.json5",
        {"name": native_module, "version": "0.0.1", "types": "./Index.d.ts"},
    )
    copied("generated/native/Index.d.ts", module + "/" + local + "/Index.d.ts")
    if config["provider"] == "kotlin":
        copied(
            "generated/native/record-export.d.ts", module + "/" + local + "/record-export.d.ts"
        )
    notice_passthrough = {}
    by_name = {r["path"]: r for r in refs}
    for name in ("LICENSE", "NOTICE"):
        if name not in by_name:
            raise ValueError("package attribution is missing: " + name)
        copied(name, module + "/" + name)
        notice_passthrough["package/" + name] = by_name[name]
    for notice in config["runtime_notices"]:
        basename = Path(notice["source"]).name
        source = "generated/notices/" + basename
        if source not in by_name:
            raise ValueError("Source parent runtime notice is missing: " + source)
        copied(source, module + "/THIRD_PARTY_NOTICES/" + basename)
        notice_passthrough["package/THIRD_PARTY_NOTICES/" + basename] = by_name[source]
    for name, asset in zip(additional_notices, additional_assets):
        source = "generated/" + name
        if source not in by_name:
            raise ValueError("Source parent Rust runtime notice is missing: " + source)
        copied(source, module + "/" + asset)
        notice_passthrough["package/" + asset] = by_name[source]
    copied("generated/facade/Boundary.ets", module + "/src/main/ets/Boundary.ets")
    copied("generated/facade/RuntimeFrame.ets", module + "/src/main/ets/RuntimeFrame.ets")
    if config["provider"] == "kotlin":
        copied("generated/facade/KotlinPort.ets", module + "/src/main/ets/KotlinPort.ets")
    if config["variant"] == "test" and config["provider"] != "kotlin":
        for name in ("TestFrame.ets", "FacadeFrame.ets"):
            source = "generated/facade/" + name
            if source not in by_name:
                raise ValueError("test facade dependency omitted from frozen Source")
            copied(source, module + "/src/main/ets/" + name)
    exported = sorted(
        r["name"]
        for key in ("records", "resources", "errors", "functions")
        for r in descriptor["public"][key]
    )
    write(
        module + "/Index.ets",
        "export { " + ", ".join(exported) + " } from './src/main/ets/Boundary';\n",
    )
    rules = (
        "-enable-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-keep-dts "
        + str(mod / local)
        + "\n-keep-global-name\n"
        + "\n".join(exported)
        + "\n"
        + support.input_path(inputs, "generated/keep-rules.txt").read_text()
        + "-keep-file-name\nIndex\n-print-namecache "
        + str(project / "namecache.json")
        + "\n"
    )
    write(module + "/obfuscation-rules.txt", rules)
    native = native_inputs(inputs, refs)
    if "package/libs/arm64-v8a/" + native_module not in native:
        raise ValueError("Build parent lacks route native library")
    for dest, r in native.items():
        copied(r["path"], module + "/libs/arm64-v8a/" + Path(dest).name)
    modules = project / "tool-node-modules/@ohos"
    modules.mkdir(parents=True)
    (modules / "hvigor").symlink_to(cli / "hvigor/hvigor", target_is_directory=True)
    (modules / "hvigor-ohos-plugin").symlink_to(
        cli / "hvigor/hvigor-ohos-plugin", target_is_directory=True
    )
    for folder in ("home", "hvigor-home", "tmp", "ohpm-cache", "npm-cache"):
        (project / folder).mkdir()
    env = {
        "PATH": os.pathsep.join(
            [
                str(cli / "tool/node/bin"),
                str(cli / "ohpm/bin"),
                str(Path(config["java_home"]) / "bin"),
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
            ]
        ),
        "HOME": str(project / "home"),
        "HVIGOR_USER_HOME": str(project / "hvigor-home"),
        "DEVECO_SDK_HOME": str(cli / "sdk"),
        "NODE_HOME": str(cli / "tool/node"),
        "JAVA_HOME": config["java_home"],
        "TMPDIR": str(project / "tmp"),
        "NODE_PATH": str(project / "tool-node-modules"),
        "npm_config_cache": str(project / "npm-cache"),
        "LANG": "en_US.UTF-8",
        "LC_ALL": "en_US.UTF-8",
    }

    def run(label, args):
        run_har_command(label, args, project, work, env, commands)

    # Only a local native declaration package is declared; no registry dependency.
    run(
        "ohpm",
        [
            config["tools"]["ohpm"]["path"],
            "install",
            "--all",
            "--cache",
            project / "ohpm-cache",
        ],
    )
    run(
        "hvigor",
        [
            config["tools"]["node"]["path"],
            config["tools"]["hvigor"]["path"],
            "assembleHar",
            "--mode",
            "module",
            "-p",
            "product=default",
            "-p",
            "buildMode=release",
            "--no-daemon",
        ],
    )
    archive = mod / "build/default/outputs/default" / f"{module}.har"
    if archive.is_symlink() or not archive.is_file():
        raise ValueError("compiler did not emit the exact declared HAR")
    passthrough = {
        "package/"
        + local
        + "/Index.d.ts": next(r for r in refs if r["path"] == "generated/native/Index.d.ts"),
        **notice_passthrough,
    }
    canonical = bind.bind_har(archive.read_bytes(), inputs, native, passthrough)
    public_receipt = public.verify_public_har(
        canonical, descriptor["public"], config, work, har
    )
    support.verify_refs(inputs, refs)
    verifier.verify_inventory(inventory, cli)
    for t in config["tools"].values():
        support.verify_tool(t)
    support.load_module(
        Path(__file__).resolve().parents[3] / "adapters/toolchains/arkts/toolchain.py"
    ).verify_runtime(config, inputs)
    # Preserve every selected parent/evidence reference as standard Package does.
    for r in refs:
        support.copy_new(support.input_path(inputs, r["path"]), work / r["path"])
    har_name = "har/" + module + ".har"
    (work / "har").mkdir()
    (work / har_name).write_bytes(canonical)
    d.save(
        work,
        "har-verification.json",
        {
            "schema": "madb.private.har-verification/1",
            "public": public_receipt,
            "native": native,
            "archive": d.ref(work, har_name),
            "configuration": "Release, property/export/filename obfuscation, native strip=false",
            "commands": commands,
            "runtime_notices": notice_passthrough,
            "target_executed": False,
        },
    )
    names = sorted([r["path"] for r in refs] + [har_name, "har-verification.json"])
    if len(names) != len(set(names)) or "candidate.json" in names:
        raise ValueError("candidate file inventory collides")
    value = {
        k: request[k]
        for k in (
            "selector",
            "generation_id",
            "contract_digest",
            "policy_digest",
            "dependencies",
            "public_symbols",
            "required_cases",
            "build_evidence",
        )
    }
    value.update(
        schema="madb.candidate/1",
        lock_digest=request["lock"]["sha256"],
        manifest_path="candidate.json",
        files=[d.ref(work, n) for n in names],
    )
    for k in ("dependencies", "build_evidence"):
        value[k].sort(key=lambda r: r["path"])
    for k in ("public_symbols", "required_cases"):
        value[k].sort()
    value["candidate_digest"] = d.semantic("madb.candidate/1", value)
    output = (d.canonical(value) + "\n").encode()
    if len(output) > support.MAX_JSON:
        raise ValueError("Candidate response exceeds 1 MiB")
    support.publish_diagnostics(
        work, extra=["ohpm.stdout", "ohpm.stderr", "hvigor.stdout", "hvigor.stderr"]
    )
    support.verify_refs(work, value["files"])
    for item in value["files"]:
        support.copy_new(support.input_path(work, item["path"]), stage / item["path"])
    shutil.rmtree(work)
    with (stage / "candidate.json").open("xb") as f:
        f.write(output)
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        print("C07 HAR Package: " + str(error), file=sys.stderr)
        raise SystemExit(2) from error
