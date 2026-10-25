"""Read-only ArkTS environment observations, never candidate certification.

The caller selects either a DevEco Contents or command-line-tools directory,
an application SDK package root, and target. CPF compiler sysroots are separate inputs to future build checks;
this module does not discover them or treat them as application SDKs.
"""
from __future__ import annotations

import json
from pathlib import Path
import plistlib
import re
import subprocess


COMPONENTS = ("ets", "native", "toolchains")
SDK_FIELDS = ("apiVersion", "version", "releaseType", "stage", "displayName", "path", "platformVersion")
TARGET_PARAMETERS = (
    "const.ohos.apiversion", "const.ohos.releasetype", "const.ohos.fullname",
    "const.product.software.version", "const.product.model", "const.product.name",
    "const.product.cpu.abilist",
)
OUTPUT_LIMIT = 16_384


def issue(errors: list[dict], code: str, component: str, message: str) -> None:
    errors.append({"code": code, "component": component, "message": message})


def parse_json_metadata(content: str | bytes) -> object:
    def unique_object(pairs: list[tuple[str, object]]) -> dict:
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON metadata field: {key}")
            result[key] = value
        return result

    text = content.decode("utf-8") if isinstance(content, bytes) else content
    return json.loads(text, object_pairs_hook=unique_object)


def read_json_metadata(path: Path) -> object:
    return parse_json_metadata(path.read_bytes())


def metadata(path: Path, component: str, errors: list[dict], *, product: bool = False) -> dict:
    try:
        document = read_json_metadata(path)
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
        value = read_json_metadata(path)
        if value.get("name") != expected_name or not isinstance(value.get("version"), str) or not value["version"].strip():
            raise ValueError("package name or version is invalid")
        result.update(name=value["name"], version=value["version"])
        if not executable.is_file():
            raise ValueError(f"missing entry point: {executable}")
    except (OSError, ValueError, AttributeError) as error:
        issue(errors, "tool_metadata_invalid", expected_name, str(error))
    return result


def inspect_command_line_tools(root: Path, errors: list[dict]) -> dict:
    path = root / "version.txt"
    result = {"path": str(root), "metadata_path": str(path), "version_source": "version.txt"}
    try:
        text = path.read_text(encoding="utf-8")
        if len(text) > OUTPUT_LIMIT:
            raise ValueError("version.txt exceeds the metadata limit")
        platforms, versions, fields = [], [], {}
        for raw in text.splitlines():
            line = raw.strip()
            if re.match(r"#\s*Command Line Tools", line):
                match = re.fullmatch(r"#\s*Command Line Tools\(([^()]+)\)", line)
                if match is None:
                    raise ValueError("invalid Command Line Tools platform header")
                platforms.append(match[1])
            elif re.match(r"#\s*Version\b", line):
                match = re.fullmatch(r"#\s*Version:\s*(\d+(?:\.\d+){3})", line)
                if match is None:
                    raise ValueError("invalid Command Line Tools Version header")
                versions.append(match[1])
            elif line and not line.startswith("#"):
                key, separator, value = line.partition(":")
                key, value = key.strip(), value.strip()
                if not separator or not key or not value or key in fields:
                    raise ValueError(f"invalid or duplicate version.txt field: {key}")
                fields[key] = value
        if len(platforms) != 1 or len(versions) != 1:
            raise ValueError("expected one Command Line Tools platform and one four-part Version header")
        for key in ("releaseType", "hvigor", "ohpm", "HarmonyOS SDK", "apiVersion", "platformVersion"):
            if key not in fields:
                raise ValueError(f"missing version.txt field: {key}")
        for key in ("hvigor", "ohpm", "platformVersion"):
            if not re.fullmatch(r"\d+(?:\.\d+){2,3}", fields[key]):
                raise ValueError(f"invalid version.txt version: {key}")
        if not fields["apiVersion"].isdigit() or int(fields["apiVersion"]) < 1:
            raise ValueError("invalid version.txt apiVersion")
        descriptor = re.fullmatch(
            r"HarmonyOS\s+(\d+(?:\.\d+){2})\s+(\w+)\s+"
            r"\(include\s+Ohos_sdk_public\s+(\d+(?:\.\d+){3})\s+"
            r"\(API Version\s+(\d+)\s+(\w+)\)\)", fields["HarmonyOS SDK"])
        if descriptor is None:
            raise ValueError("invalid version.txt HarmonyOS SDK descriptor")
        platform_version, platform_release, sdk_version, sdk_api, sdk_release = descriptor.groups()
        result.update(version=versions[0], distribution_platform=platforms[0], releaseType=fields["releaseType"],
                      hvigor_version=fields["hvigor"], ohpm_version=fields["ohpm"],
                      bundled_sdk={"version": sdk_version, "apiVersion": sdk_api,
                                   "platformVersion": platform_version, "releaseType": sdk_release})
        if platforms[0] != "mac-arm64":
            issue(errors, "tool_platform_unsupported", "command_line_tools", "This layout requires the mac-arm64 Command Line Tools distribution.")
        if fields["releaseType"].casefold() != "release":
            issue(errors, "tool_not_release", "command_line_tools", "Command Line Tools metadata must identify a Release distribution.")
        if (fields["apiVersion"] != sdk_api or fields["platformVersion"] != platform_version
                or len({fields["releaseType"].casefold(), platform_release.casefold(), sdk_release.casefold()}) != 1):
            issue(errors, "tool_metadata_conflict", "command_line_tools", "version.txt API/platform/release fields disagree with its SDK descriptor.")
    except (OSError, ValueError) as error:
        issue(errors, "tool_metadata_invalid", "command_line_tools", f"{path}: {error}")
    return result


def inspect_tools(deveco: Path, sdk: Path, timeout: float, errors: list[dict], *,
                  command_line_tools: bool = False, sdk_observation: dict | None = None) -> dict:
    tools = {}
    if command_line_tools:
        tools["command_line_tools"] = inspect_command_line_tools(deveco, errors)
    else:
        try:
            with (deveco / "Info.plist").open("rb") as stream:
                value = plistlib.load(stream)
            if not value.get("CFBundleVersion") or not value.get("CFBundleShortVersionString"):
                raise ValueError("missing DevEco version fields")
            tools["deveco"] = {"path": str(deveco), "version": value["CFBundleShortVersionString"],
                               "build": value["CFBundleVersion"], "version_source": "Info.plist"}
        except (OSError, ValueError, plistlib.InvalidFileException, AttributeError) as error:
            issue(errors, "tool_metadata_invalid", "deveco", str(error))
    tool_root = deveco if command_line_tools else deveco / "tools"
    node = deveco / "tool/node/bin/node" if command_line_tools else tool_root / "node/bin/node"
    for name, path, arguments in (
        ("node", node, ["--version"]),
        ("clang", sdk / "openharmony/native/llvm/bin/clang", ["--version"]),
        ("hdc", sdk / "openharmony/toolchains/hdc", ["-v"]),
    ):
        tools[name] = inspect_executable(path, arguments, name, timeout, errors)
    for name, package, executable in (
        ("@ohos/hvigor", "hvigor/hvigor/package.json", "hvigor/bin/hvigorw.js"),
        ("@ohos/hvigor-ohos-plugin", "hvigor/hvigor-ohos-plugin/package.json", "hvigor/hvigor-ohos-plugin/package.json"),
        ("@ohos/ohpm", "ohpm/package.json", "ohpm/bin/ohpm"),
    ):
        tools[name] = inspect_package(tool_root / package, tool_root / executable, name, errors)
    hvigor, plugin = tools["@ohos/hvigor"], tools["@ohos/hvigor-ohos-plugin"]
    if hvigor.get("version") != plugin.get("version"):
        issue(errors, "hvigor_component_mismatch", "hvigor", "Hvigor and its OHOS plugin versions differ.")
    if command_line_tools:
        identity = tools["command_line_tools"]
        for name, expected in (("@ohos/hvigor", identity.get("hvigor_version")),
                               ("@ohos/ohpm", identity.get("ohpm_version"))):
            if expected is not None and tools[name].get("version") != expected:
                issue(errors, "tool_component_mismatch", name, "Package version differs from Command Line Tools version.txt.")
        expected_sdk = identity.get("bundled_sdk")
        if expected_sdk and sdk_observation is not None:
            for name, observed in {"sdk": sdk_observation["product"], **sdk_observation["components"]}.items():
                if any(observed.get(key) != value for key, value in expected_sdk.items()):
                    issue(errors, "tool_sdk_mismatch", name, "Selected SDK metadata differs from Command Line Tools version.txt.")
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


def inspect_environment(deveco: Path | None, sdk: Path, target: str | None, timeout: float = 10,
                        target_abi: str = "arm64-v8a", *, command_line_tools: Path | None = None) -> dict:
    if (deveco is None) == (command_line_tools is None):
        raise ValueError("Select exactly one DevEco Contents or Command Line Tools directory")
    if not 0 < timeout <= 30:
        raise ValueError("timeout must be greater than zero and at most 30 seconds per command")
    if not re.fullmatch(r"[A-Za-z0-9_-]+", target_abi):
        raise ValueError("target_abi must be one ABI name")
    tool_root = (deveco if deveco is not None else command_line_tools).absolute()
    sdk = sdk.absolute()
    errors: list[dict] = []
    sdk_observation = inspect_sdk(sdk, errors)
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
        "sdk": sdk_observation,
        "tools": inspect_tools(tool_root, sdk, timeout, errors, command_line_tools=command_line_tools is not None,
                               sdk_observation=sdk_observation),
        "target": inspect_target(sdk / "openharmony/toolchains/hdc", target, target_abi, timeout, errors),
        "errors": errors,
    }
    report["environment_checks_passed"] = not errors
    return report
