"""Read-only ArkTS environment observations, never candidate certification.

The caller selects the DevEco Contents directory, application SDK package root,
and target. CPF compiler sysroots are separate inputs to future build checks;
this module does not discover them or treat them as application SDKs.
"""
from __future__ import annotations

import json
from pathlib import Path
import plistlib
import re
import subprocess


COMPONENTS = ("ets", "native", "toolchains")
SDK_FIELDS = ("apiVersion", "version", "releaseType", "stage", "displayName", "path")
TARGET_PARAMETERS = (
    "const.ohos.apiversion", "const.ohos.releasetype", "const.ohos.fullname",
    "const.product.software.version", "const.product.model", "const.product.name",
    "const.product.cpu.abilist",
)
OUTPUT_LIMIT = 16_384


def issue(errors: list[dict], code: str, component: str, message: str) -> None:
    errors.append({"code": code, "component": component, "message": message})


def metadata(path: Path, component: str, errors: list[dict], *, product: bool = False) -> dict:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
        value = document.get("data") if product else document
        if not isinstance(value, dict):
            raise ValueError("expected a metadata object")
        for name in ("apiVersion", "version", "releaseType"):
            if not isinstance(value.get(name), str) or not value[name].strip():
                raise ValueError(f"missing or invalid {name}")
        if not value["apiVersion"].isdigit() or int(value["apiVersion"]) < 1:
            raise ValueError("apiVersion must be a positive decimal string")
        return {"metadata_path": str(path), **{key: value[key] for key in SDK_FIELDS if key in value}}
    except (OSError, ValueError, AttributeError) as error:
        issue(errors, "sdk_metadata_invalid", component, f"{path}: {error}")
        return {"metadata_path": str(path)}


def inspect_sdk(root: Path, errors: list[dict]) -> dict:
    product = metadata(root / "sdk-pkg.json", "sdk", errors, product=True)
    components = {
        name: metadata(root / "openharmony" / name / "oh-uni-package.json", name, errors)
        for name in COMPONENTS
    }
    rows = {"sdk": product, **components}
    formal = True
    for name, value in rows.items():
        if value.get("releaseType") != "Release" or value.get("stage", "Release") != "Release":
            formal = False
            issue(errors, "sdk_not_release", name, "Release package metadata is required; directory and IDE names are not evidence.")
    consistent = True
    for name, value in components.items():
        if any(not value.get(key) or value.get(key) != product.get(key)
               for key in ("apiVersion", "version", "releaseType")):
            consistent = False
            issue(errors, "sdk_component_mismatch", name, "Component API, version and releaseType must match the selected product SDK.")
    return {"root": str(root), "product": product, "components": components,
            "is_formal": formal, "components_consistent": consistent,
            "cpf_sysroot_checked": False}


def command(arguments: list[str], timeout: float) -> dict:
    """A timeout kills/reaps only this command, never an existing hdc server."""
    try:
        result = subprocess.run(arguments, stdin=subprocess.DEVNULL, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=timeout)
        output = result.stdout + result.stderr
        return {"command": arguments, "state": "observed" if result.returncode == 0 else "failed",
                "exit_code": result.returncode, "output": output[:OUTPUT_LIMIT],
                "output_truncated": len(output) > OUTPUT_LIMIT}
    except subprocess.TimeoutExpired:
        return {"command": arguments, "state": "timeout", "timeout_seconds": timeout}
    except OSError as error:
        return {"command": arguments, "state": "unavailable", "error": str(error)}


def inspect_executable(path: Path, arguments: list[str], name: str,
                       timeout: float, errors: list[dict]) -> dict:
    observed = command([str(path), *arguments], timeout)
    if observed["state"] != "observed" or not observed.get("output", "").strip() or observed.get("output_truncated"):
        issue(errors, "tool_probe_failed", name, f"Version probe did not complete with usable output: {observed['state']}.")
    return {"path": str(path), "version_source": "command", **observed}


def inspect_package(path: Path, executable: Path, expected_name: str, errors: list[dict]) -> dict:
    result = {"path": str(executable), "metadata_path": str(path), "version_source": "package_metadata"}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        if value.get("name") != expected_name or not isinstance(value.get("version"), str) or not value["version"].strip():
            raise ValueError("package name or version is invalid")
        result.update(name=value["name"], version=value["version"])
        if not executable.is_file():
            raise ValueError(f"missing entry point: {executable}")
    except (OSError, ValueError, AttributeError) as error:
        issue(errors, "tool_metadata_invalid", expected_name, str(error))
    return result


def inspect_tools(deveco: Path, sdk: Path, timeout: float, errors: list[dict]) -> dict:
    tools = {}
    try:
        with (deveco / "Info.plist").open("rb") as stream:
            value = plistlib.load(stream)
        if not value.get("CFBundleVersion") or not value.get("CFBundleShortVersionString"):
            raise ValueError("missing DevEco version fields")
        tools["deveco"] = {"path": str(deveco), "version": value["CFBundleShortVersionString"],
                           "build": value["CFBundleVersion"], "version_source": "Info.plist"}
    except (OSError, ValueError, plistlib.InvalidFileException, AttributeError) as error:
        issue(errors, "tool_metadata_invalid", "deveco", str(error))
    for name, path, arguments in (
        ("node", deveco / "tools/node/bin/node", ["--version"]),
        ("clang", sdk / "openharmony/native/llvm/bin/clang", ["--version"]),
        ("hdc", sdk / "openharmony/toolchains/hdc", ["-v"]),
    ):
        tools[name] = inspect_executable(path, arguments, name, timeout, errors)
    for name, package, executable in (
        ("@ohos/hvigor", "tools/hvigor/hvigor/package.json", "tools/hvigor/bin/hvigorw.js"),
        ("@ohos/hvigor-ohos-plugin", "tools/hvigor/hvigor-ohos-plugin/package.json", "tools/hvigor/hvigor-ohos-plugin/package.json"),
        ("@ohos/ohpm", "tools/ohpm/package.json", "tools/ohpm/bin/ohpm"),
    ):
        tools[name] = inspect_package(deveco / package, deveco / executable, name, errors)
    hvigor, plugin = tools["@ohos/hvigor"], tools["@ohos/hvigor-ohos-plugin"]
    if hvigor.get("version") != plugin.get("version"):
        issue(errors, "hvigor_component_mismatch", "hvigor", "Hvigor and its OHOS plugin versions differ.")
    return tools


def inspect_target(hdc: Path, target: str | None, target_abi: str, timeout: float, errors: list[dict]) -> dict:
    result = {"requested": target, "state": "not_checked", "kind": "unknown", "parameters": {}}
    if target is None:
        issue(errors, "target_not_checked", "target", "Supply an explicit --target; no devices are discovered or started.")
        return result
    if not target.strip() or target.startswith("-") or any(character.isspace() for character in target):
        issue(errors, "target_invalid", "target", "Target must be one non-option hdc connect key.")
        return result
    observations = []
    result["observations"] = observations
    for key in TARGET_PARAMETERS:
        # -p is hdc's single-client mode: it must not launch the server.
        observed = command([str(hdc), "-p", "-t", target, "shell", "param", "get", key], timeout)
        observations.append(observed)
        state = observed["state"]
        if state == "timeout":
            result["state"] = "timeout"
            issue(errors, "target_timeout", "target", "Client timed out; this does not establish that the target is absent.")
            return result
        output = observed.get("output", "").strip()
        connection_error = any(marker in output.lower() for marker in (
            "connect server failed", "target not found", "no connected devices", "device not found", "not connected"))
        if state == "unavailable" or connection_error:
            result["state"] = "unavailable"
            issue(errors, "target_unavailable", "target", "The selected target could not be reached by the existing hdc server.")
            return result
        if state != "observed" or not output or observed.get("output_truncated") or "\n" in output or "fail" in output.lower():
            result["state"] = "query_failed"
            issue(errors, "target_query_failed", key, "Target parameter query did not return one usable value.")
            return result
        result["parameters"][key] = output
    result["state"] = "observed"
    values = result["parameters"]
    if any("emulator" in values[key].lower() for key in (
        "const.product.model", "const.product.name", "const.product.software.version")):
        result["kind"] = "emulator"
    else:
        # Absence of the emulator marker is not hardware attestation.
        result["kind"] = "unverified_device"
    if not values["const.ohos.apiversion"].isdigit() or int(values["const.ohos.apiversion"]) < 1:
        issue(errors, "target_api_invalid", "target", "Target API must be a positive decimal value.")
    if values["const.ohos.releasetype"] != "Release":
        issue(errors, "target_not_release", "target", "The observed target runtime is not marked Release.")
    if target_abi not in re.split(r"[,\s]+", values["const.product.cpu.abilist"]):
        issue(errors, "target_abi_mismatch", "target", f"Target ABI list does not contain the requested {target_abi}.")
    return result


def inspect_environment(deveco: Path, sdk: Path, target: str | None, timeout: float = 10,
                        target_abi: str = "arm64-v8a") -> dict:
    if not 0 < timeout <= 30:
        raise ValueError("timeout must be greater than zero and at most 30 seconds per command")
    if not re.fullmatch(r"[A-Za-z0-9_-]+", target_abi):
        raise ValueError("target_abi must be one ABI name")
    deveco, sdk = deveco.absolute(), sdk.absolute()
    errors: list[dict] = []
    report = {
        "diagnostic_kind": "arkts_environment_preflight", "evidence_scope": "environment_only",
        "certifies_c07": False,
        "requirements": {"sdk_release_type": "Release", "target_release_type": "Release",
                         "explicit_target": True, "target_abi": target_abi,
                         "matching_sdk_components": list(COMPONENTS)},
        "limitations": ["No source or distribution provenance closure is verified.",
                        "CPF compiler, akinterop and compiler sysroots are not inspected.",
                        "No build, HAP/HAR consumption, signing, P05/P06 or C07 acceptance is performed.",
                        "Target parameters do not attest physical hardware or SDK/target compatibility."],
        "sdk": inspect_sdk(sdk, errors),
        "tools": inspect_tools(deveco, sdk, timeout, errors),
        "target": inspect_target(sdk / "openharmony/toolchains/hdc", target, target_abi, timeout, errors),
        "errors": errors,
    }
    report["environment_checks_passed"] = not errors
    return report
