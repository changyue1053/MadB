"""Run the real ordinary C07 profile/2 local generation and binding check.

Requires explicit already installed tool observations and the locally built
composition_generation_probe. Does not install tools or enter C04/E02 execution.
"""
from copy import deepcopy
from dataclasses import asdict
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

import check_generate_inputs as fixture
from adapters.generators.arkts import composition_recipe


def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


class Check:
    def __init__(self, output, bridge):
        self.output = output
        self.bridge = bridge
        self.checks = []
        self.commands = []
        (output / "logs").mkdir()

    def command(self, name, argv, cwd, *, data=None, reject=None):
        result = subprocess.run(list(map(str, argv)), cwd=cwd, input=data,
                                env=fixture.support.clean_environment(), capture_output=True, timeout=180)
        if len(result.stdout) > 16 * fixture.support.MAX_JSON or len(result.stderr) > 16 * fixture.support.MAX_JSON:
            raise AssertionError("check command exceeded output budget")
        prefix = self.output / "logs" / name
        prefix.with_suffix(".stdout").write_bytes(result.stdout)
        prefix.with_suffix(".stderr").write_bytes(result.stderr)
        if data is not None:
            prefix.with_suffix(".stdin").write_bytes(data)
        self.commands.append({"name": name, "argv": list(map(str, argv)), "cwd": str(cwd),
                              "exit_code": result.returncode, "stdout_sha256": sha(result.stdout),
                              "stderr_sha256": sha(result.stderr)})
        if reject is None:
            if result.returncode:
                raise AssertionError(name + ": " + result.stderr.decode(errors="replace")[-5000:])
        else:
            if result.returncode == 0 or reject not in result.stderr.decode(errors="replace"):
                raise AssertionError(name + ": expected rejection " + reject + "\n" + result.stderr.decode(errors="replace")[-5000:])
            self.checks.append({"name": name, "result": "rejected", "diagnostic": reject})
        return result.stdout

    def generator(self, name, operation, root, work, *, request=None, reject=None):
        config = fixture.support.read_json(root / "backend.json")
        tool = config["tools"]["python"]
        fixture.support.verify_tool(tool)
        argv = [tool["path"], *tool["invocation_arguments"], "../inputs/" + fixture.PREFIX + "/" + fixture.ENTRY,
                operation, "--config", "../inputs/backend.json"]
        data = request if request is not None else (root / ("read-request.json" if operation == "metadata" else "generate-request.json")).read_bytes()
        result = self.command(name, argv, work, data=data, reject=reject)
        fixture.support.verify_tool(tool)
        return result

    def bind(self, name, root, output, *, reject=None):
        return self.command(name, [self.bridge, "bind", root, output], self.output, reject=reject)

    def changed_file(self, name, root, output, path, data, rejection):
        before = path.read_bytes()
        (self.output / "logs" / (name + ".mutation")).write_bytes(data)
        try:
            path.write_bytes(data)
            self.bind(name, root, output, reject=rejection)
        finally:
            path.write_bytes(before)


def repin_claims(root, bridge):
    """Rebuild referring input identities, never the independent required set."""
    d = fixture.d
    composition = fixture.support.read_json(root / "composition.json")
    profile = fixture.support.read_json(root / "profile.json")
    profile["composition"]["file"] = d.ref(root, "composition.json", "madb.composition/1")
    profile["composition"]["semantic_digest"] = fixture.bridge_command(bridge, "composition-digest", root).decode().strip()
    d.save(root, "profile.json", profile)
    profile_digest = fixture.bridge_command(bridge, "profile-digest", root).decode().strip()
    project = fixture.support.read_json(root / "project.json")
    project["boundaries"][0]["profile"] = d.ref(root, "profile.json", "madb.profile/2")
    d.save(root, "project.json", project)
    lock = fixture.support.read_json(root / "lock.json")
    lock["project_digest"] = d.project_digest(project)
    lock["profiles"][0].update(file=d.ref(root, "profile.json", "madb.profile/2"), semantic_digest=profile_digest)
    lock["inputs"] = [d.ref(root, r["path"]) for r in lock["inputs"]]
    d.save(root, "lock.json", lock)
    request = fixture.support.read_json(root / "read-request.json")
    request.update(profile=d.ref(root, "profile.json", "madb.profile/2"), lock=d.ref(root, "lock.json", "madb.lock/1"))
    request["selector"]["profile_digest"] = profile_digest
    request["inputs"] = [d.ref(root, r["path"], r["schema"]) for r in request["inputs"]]
    d.save(root, "read-request.json", request)


def run(args):
    output = args.output.absolute()
    if output.exists() or output.is_symlink():
        raise ValueError("check output must be new")
    output.mkdir(parents=True)
    bridge = args.bridge.resolve(strict=True)
    if fixture.support.sha(bridge) != args.bridge_sha256:
        raise ValueError("caller-pinned bridge image differs")
    check = Check(output, bridge)
    inputs = output / "inputs"
    config, read = fixture.prepare(inputs, args.tool_observation, bridge)
    typescript = config["tools"]["typescript"]
    roles = composition_recipe.observe_recipe(
        inputs, output / "role-observation", mode="ordinary",
        node=args.observation_node, node_sha256=args.observation_node_sha256.removeprefix("sha256:"),
        typescript=typescript["path"], typescript_sha256=typescript["sha256"].removeprefix("sha256:"),
    )
    composition_recipe.verify_observation(roles, inputs, output / "role-observation")
    actual_roles = {(r.subject, r.stage, r.scope_kind, r.anchor, r.layer) for r in roles.required}
    expected_roles = {(r["subject"]["public_id"], r["stage"], r["scope"]["kind"],
                       r["scope"]["anchor"]["public_id"], r["scope"]["layer"])
                      for r in fixture.provided_roles()}
    assert len(roles.required) == 5 and actual_roles == expected_roles
    fixture.d.save(output, "role-observation.json", asdict(roles))
    check.checks.append({"name": "real-source-observer-five-roles", "result": "passed"})
    work = output / "work"
    work.mkdir()
    first_ir = check.generator("metadata-first", "metadata", inputs, work)
    work.rename(output / "metadata-first")
    work.mkdir()
    second_ir = check.generator("metadata-replay", "metadata", inputs, work)
    assert first_ir == second_ir, "metadata is not deterministic"
    check.checks.append({"name": "real-source-metadata-replay", "result": "passed"})
    work.rename(output / "metadata-replay")
    (inputs / "contract.json").write_bytes(first_ir)
    ir = json.loads(first_ir)
    request = dict(read, schema="madb.generate-request/1", output_root="generated-step",
                   contract=fixture.d.ref(inputs, "contract.json", "madb.contract-ir/1"),
                   input_digest=ir["source_digest"], contract_digest=ir["structure_digest"],
                   policy_digest=ir["policy_digest"], task_digest="sha256:" + "0" * 64)
    fixture.d.save(inputs, "generate-request.json", request)
    request["task_digest"] = check.command("task-digest", [bridge, "task", inputs], output).decode().strip()
    fixture.d.save(inputs, "generate-request.json", request)
    work.mkdir()
    first_output = check.generator("generation-first", "generate", inputs, work)
    first_bound = check.bind("binding-first", inputs, work)
    (output / "bound.json").write_bytes(first_bound)
    work.rename(output / "generated-first")
    work.mkdir()
    second_output = check.generator("generation-replay", "generate", inputs, work)
    second_bound = check.bind("binding-replay", inputs, work)
    assert first_output == second_output and first_bound == second_bound, "generation identities changed on replay"
    manifest = json.loads(first_output)
    assert sorted(f["path"] for f in manifest["files"]) == fixture.PAYLOADS
    assert manifest["profile_digest"] == read["selector"]["profile_digest"]
    assert manifest["input_digest"] == ir["source_digest"]
    assert manifest["task_digest"] == request["task_digest"]
    for item in manifest["files"]:
        assert (work / item["path"]).read_bytes() == (output / "generated-first" / item["path"]).read_bytes()
    fixture.support.verify_refs(work, manifest["files"])
    check.checks.append({"name": "real-generation-replay-and-fixed-15-payload-binding", "result": "passed", "files": len(manifest["files"])})

    qpath = inputs / "generate-request.json"
    for name, change, expected in [
        ("wrong-producer", lambda q: q["component"].update(id="madb.local.c07-composition/other"), "LOCK004"),
        ("wrong-task", lambda q: q.update(task_digest="sha256:" + "1" * 64), "PLAN002"),
        ("wrong-profile-selector", lambda q: q["selector"].update(profile_digest="sha256:" + "1" * 64), "LOCK001"),
        ("wrong-source-lineage", lambda q: q.update(input_digest="sha256:" + "1" * 64), "LOCK005"),
        ("missing-request-source", lambda q: q["inputs"].pop(), "LOCK005"),
        ("wrong-output-domain", lambda q: q.update(output_root="other-step"), "LOCK001"),
    ]:
        value = deepcopy(request)
        change(value)
        check.changed_file(name, inputs, work, qpath, (fixture.d.canonical(value) + "\n").encode(), expected)
    # Real changed bytes are rejected even though the producer previously ran.
    for name, path, expected in [
        ("changed-locked-source", inputs / fixture.NATIVE, "INPUT003"),
        ("changed-profile-bytes", inputs / "profile.json", "FILE004"),
        ("changed-installed-image", inputs / "tools/python", "LOCK004"),
        ("changed-output-bytes", work / "generated/facade/Boundary.ets", "FILE004"),
    ]:
        before = path.read_bytes()
        changed = before + b"\n" if path.name == "profile.json" else before[:-1] + bytes([before[-1] ^ 1])
        check.changed_file(name, inputs, work, path, changed, expected)
    asset = work / "generated/facade/Boundary.ets"
    original = asset.read_bytes()
    asset.unlink()
    try:
        check.bind("missing-output", inputs, work, reject="FILE003")
    finally:
        asset.write_bytes(original)
    extra = work / "unexpected.txt"
    extra.write_text("unclaimed generated byte\n")
    try:
        check.bind("extra-output", inputs, work, reject="EXECUTION002")
    finally:
        extra.unlink()
    extra_directory = work / "unexpected-empty"
    extra_directory.mkdir()
    try:
        check.bind("extra-empty-output-directory", inputs, work, reject="undeclared or empty output directory")
    finally:
        extra_directory.rmdir()
    asset.unlink()
    asset.symlink_to(inputs / fixture.NATIVE)
    try:
        check.bind("output-symlink", inputs, work, reject="nonregular output or symlink")
    finally:
        asset.unlink()
        asset.write_bytes(original)
    alias = output / "work-alias"
    alias.symlink_to(work, target_is_directory=True)
    try:
        check.bind("output-root-alias", inputs, alias, reject="aliased or non-directory root")
    finally:
        alias.unlink()
    check.bind("output-root-traversal", inputs, work / ".." / "work", reject="absolute non-traversing directory required")
    check.bind("overlapping-domains", inputs, inputs, reject="input and output domains overlap")
    # Accurate feature/version guard before launching the actual reader tools.
    legacy = deepcopy(read)
    legacy["profile"]["schema"] = "madb.profile/1"
    denied = output / "version-negative"
    denied.mkdir()
    # The sibling inputs layout is the same; no generation outputs are admitted.
    check.generator("old-version-on-new-entry", "metadata", inputs, denied,
                    request=fixture.d.canonical(legacy).encode(), reject="requires an explicit profile/2 reference")

    # Change provided claims and repin every affected input. A fixed host
    # requirement still rejects; a byte mismatch cannot make this test pass.
    missing = output / "missing-role"
    shutil.copytree(inputs, missing / "inputs")
    role_root = missing / "inputs"
    composition = fixture.support.read_json(role_root / "composition.json")
    composition["responsibilities"].pop()
    fixture.d.save(role_root, "composition.json", composition)
    repin_claims(role_root, bridge)
    role_work = missing / "work"
    role_work.mkdir()
    check.generator("missing-independent-role", "metadata", role_root, role_work,
                    reject="missing, extra, or mismatched responsibility")
    assert check.bind("final-restored-binding", inputs, work) == first_bound
    assert fixture.support.sha(bridge) == args.bridge_sha256
    report = {"scope": "real local ordinary C07 profile/2 metadata, generation and byte binding",
              "checks": check.checks, "commands": check.commands,
              "profile_digest": manifest["profile_digest"], "source_digest": manifest["input_digest"],
              "task_digest": manifest["task_digest"], "generation_id": manifest["generation_id"],
              "bridge": {"path": str(bridge), "sha256": args.bridge_sha256},
              "source_files": {n: fixture.d.ref(fixture.ROOT, n) for n in [
                  "adapters/generators/arkts/driver.py", fixture.ENTRY,
                  "tests/integration/arkts_composition_recipe/check_generate_inputs.py",
                  "tests/integration/arkts_composition_recipe/check_generate.py"]},
              "not_claimed": ["E01 process registration", "E02 session admission", "C04/Source commit", "native/SDK compilation", "target runtime", "enhanced/disabled full generation", "whole-package acceptance or publication"]}
    fixture.d.save(output, "generation-checks.json", report)
    print(json.dumps({"checks": len(check.checks), "output": str(output / "generation-checks.json")}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--bridge", type=Path, required=True)
    parser.add_argument("--bridge-sha256", required=True)
    parser.add_argument("--tool-observation", type=Path, required=True)
    parser.add_argument("--observation-node", type=Path, required=True)
    parser.add_argument("--observation-node-sha256", required=True)
    run(parser.parse_args())


if __name__ == "__main__":
    main()
