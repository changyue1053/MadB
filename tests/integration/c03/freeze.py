"""Convert observed probe inputs to C02 project/profile/lock documents.

These describe the stateless C03 subset. They do not claim C05 engine support.
"""
from pathlib import Path
import copy
import hashlib
import json
import shutil
import sys

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from common import ROOT, digest, file_ref, write_json

sys.path.insert(0, str(ROOT / "tests/contract"))
from test_formats import canonical


def semantic(domain, value):
    return "sha256:" + hashlib.sha256((domain + "\n" + canonical(value)).encode()).hexdigest()


def freeze(output):
    observed = json.loads((output / "toolchain-lock.json").read_text())
    recipe = observed["recipe"]
    commands = json.loads((output / "commands.json").read_text())
    registry = Registry()
    schemas = {}
    for path in (ROOT / "docs/schemas").glob("*.schema.json"):
        schema = json.loads(path.read_text())
        if schema.get("$id", "").startswith("urn:madb:schema:"):
            registry = registry.with_resource(schema["$id"], Resource.from_contents(schema))
            schemas[path.name.removesuffix(".schema.json")] = schema
    def save(name, kind, value):
        Draft202012Validator(schemas[kind], registry=registry).validate(value)
        destination = output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        write_json(destination, value)
        return file_ref(destination, output, f"madb.{kind}/1")

    inputs = []
    for reference in observed["inputs"]:
        destination = output / reference["path"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / reference["path"], destination)
        if file_ref(destination, output) != reference:
            raise RuntimeError("Probe input changed while copying: " + reference["path"])
        inputs.append(file_ref(destination, output))
    for name in ("commands.json", "toolchain-lock.json"):
        inputs.append(file_ref(output / name, output))
    schemas_locked = []
    for name in ("common", "project", "profile", "lock"):
        relative = f"docs/schemas/{name}.schema.json"
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, destination)
        schemas_locked.append(file_ref(destination, output))
    boundaries, profiles, locked_tools = [], [], []
    sdk_info = output / "observed-sdk.json"
    write_json(sdk_info, {"xcode": recipe["xcode"], "sdk_version": recipe["sdk_version"],
                         "sdk_build": recipe["sdk_build"], "inputs": observed["sdk_inputs"],
                         "headers": {t: json.loads((output / t / "header-inputs.json").read_text())
                                     for t in recipe["targets"]}})
    inputs.append(file_ref(sdk_info, output))
    for target in recipe["targets"]:
        tools = []
        for tool in observed["tools"]:
            name = tool["id"]
            spec = recipe["generators"].get(name)
            is_rust = name in ("rustc", "rust-nm")
            # All command variants are additionally bound by commands.json.
            invocations = [c["argv"][1:] for c in commands if c["argv"][0] == tool["path"]
                           and (target in " ".join(c["argv"]) or not any(t in " ".join(c["argv"]) for t in recipe["targets"]))]
            tools.append({"id": f"{name}-{target}", "version": spec["version"] if spec else recipe["rust"] if is_rust else recipe["xcode"].replace("\n", "; "),
                          "sha256": tool["sha256"], "host": recipe["host"], "target": target,
                          "source": spec["source"] if spec else "https://static.rust-lang.org/dist/channel-rust-1.99.1.toml" if is_rust else "https://developer.apple.com/download/all/",
                          "arguments": invocations[0] if invocations else []})
        locked_tools.extend(tools)
        for provider, consumer, source in [("rust", "c", "rust_provider.rs"), ("c", "rust", "c_provider.h")]:
            identity = f"c03-{consumer}-to-{provider}-{target}"
            profile = {"schema": "madb.profile/1", "id": identity, "route": "RUST-C-ABI",
                       "provider": {"language": provider, "runtime": "native"},
                       "consumer": {"language": consumer, "runtime": "native"}, "direction": "consumer-to-provider",
                       "target": target, "abi": "c11", "artifact_kind": "static-library", "build_system": "rustc-clang-probe",
                       "deployment_target": recipe["deployment_target"], "tools": tools,
                       "sdk": file_ref(sdk_info, output, "madb.c03-sdk/1"), "macros": [],
                       "required_features": ["core.exact-numbers@1", "core.values@1"],
                       "supported_features": ["core.exact-numbers@1", "core.values@1"], "restrictions": [],
                       "defaults": {"values": {"ownership": "copy"}, "origin": {"authority": "profile-default",
                           "location": {"path": "tests/integration/c03/toolchain.json", "line": 1, "column": 1}, "id": "c03.copy"}},
                       "backend_config": None, "extensions": {"madb.probe": "C03/P02 stateless only; no SDK certification"}}
            reference = save(f"profiles/{identity}.json", "profile", profile)
            projection = copy.deepcopy(profile)
            projection.pop("extensions")
            projection["defaults"] = projection["defaults"]["values"]
            projection["tools"].sort(key=lambda value: value["id"])
            profiles.append({"id": identity, "semantic_digest": semantic("madb.profile/1", projection), "file": reference})
            boundaries.append({"id": identity, "route": "RUST-C-ABI", "direction": "consumer-to-provider",
                               "source": {"kind": "rust-c-declaration" if provider == "rust" else "c-header",
                                          "path": "tests/fixtures/native-probe/" + source},
                               "profile": reference, "target": target, "output": f"generated/{identity}"})
    project = {"schema": "madb.project/1", "project": "c03-native-probe", "sdk_namespace": "madb.c03",
               "boundaries": sorted(boundaries, key=lambda b: b["id"]), "policies": [], "policy_families": [],
               "artifacts": {"package": "c03-probe-only", "manifest": "future-c05-candidate.json"}}
    save("project.json", "project", project)
    lock = {"schema": "madb.lock/1", "project_digest": semantic("madb.project/1", project), "profiles": profiles,
            "tools": locked_tools, "components": [{"id": "madb.c03/probe", "version": "0.1.0-probe",
                                                    "sha256": digest(output / "toolchain-lock.json")}],
            "inputs": sorted(inputs, key=lambda value: value["path"]), "schemas": schemas_locked,
            "extensions": {"madb.probe": "C03 observations; no source-reader IR or release candidate yet"}}
    save("madb.lock.json", "lock", lock)


if __name__ == "__main__":
    freeze(Path(sys.argv[1]).resolve())
