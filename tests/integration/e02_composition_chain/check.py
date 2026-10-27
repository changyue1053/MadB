"""Real public reader/generator sessions, atomic Source and bounded failures.

This checks source production only. It does not build an SDK, HAR or target app.
"""
from datetime import datetime, timezone
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
import prepare as fixture
from prepare import base, d

FAULTS = (
    "cancel-between", "cancel-accept", "cancel-commit", "reader-cancel-before", "reader-cancel-result",
    "generator-cancel-before", "generator-cancel-result", "reader-output-drift",
    "generator-output-drift", "upstream-drift",
)
EXIT_OK = "Exited(ProcessExit { code: Some(0), signal: None })"
CANCELLED_OK = "Stopped { reason: Cancellation, forced: false, exit: Some(ProcessExit { code: Some(0), signal: None }) }"


def now():
    return datetime.now(timezone.utc).isoformat()


def file_id(path):
    data = path.read_bytes()
    return {"path": str(path), "size": len(data), "sha256": "sha256:" + hashlib.sha256(data).hexdigest()}


def inventory(root):
    return {p.relative_to(root).as_posix(): file_id(p) for p in sorted(root.rglob("*")) if p.is_file()}


def command(argv, output, *, expected_error=None):
    start = now()
    try:
        result = subprocess.run(list(map(str, argv)), capture_output=True, timeout=480,
                                env=base.support.clean_environment())
    except subprocess.TimeoutExpired as error:
        (output.parent / (output.name + ".timeout.json")).write_text(json.dumps({
            "started_at": start, "finished_at": now(), "command": list(map(str, argv)),
            "error": str(error), "sdk_or_cleanup_success": False,
        }, indent=2) + "\n")
        raise
    output.with_suffix(".stdout").write_bytes(result.stdout)
    output.with_suffix(".stderr").write_bytes(result.stderr)
    output.with_suffix(".execution.json").write_text(json.dumps({
        "started_at": start, "finished_at": now(), "command": list(map(str, argv)),
        "exit_code": result.returncode,
        "stdout": file_id(output.with_suffix(".stdout")), "stderr": file_id(output.with_suffix(".stderr")),
    }, indent=2) + "\n")
    if expected_error is not None:
        if result.returncode == 0 or expected_error not in result.stderr.decode(errors="replace"):
            raise AssertionError(f"expected exact rejection {expected_error}: {result.returncode} {result.stderr!r}")
        return None
    if result.returncode:
        raise AssertionError(f"probe failed: {result.returncode} {result.stderr!r}")
    return json.loads(result.stdout)


def process_complete(stage, *, cancelled=False):
    process = stage["process"]
    assert process is not None, stage
    cleanup = process["cleanup"]
    assert cleanup["locally_complete"] and cleanup["leader_reaped"] and cleanup["stdout_eof"] and cleanup["stderr_eof"], stage
    assert not process["pending_cleanup"] and not cleanup["signal_failed"], stage
    expected = (EXIT_OK, CANCELLED_OK) if cancelled else (EXIT_OK,)
    assert process["termination"] in expected, process["termination"]
    assert not process["stdout_truncated"] and not process["stderr_truncated"], stage
    assert process["stdout_observed"] == len(process["stdout"].encode()), stage
    assert process["stderr_observed"] == len(process["stderr"].encode()), stage


def stage_identities(stages, run, fault):
    assert len(stages) in (1, 2), stages
    for stage, operation, component in zip(stages, ("read", "generate"), ("reader", "generator")):
        assert stage["component"] == component, stage["component"]
        assert stage["session_id"] == "chain-" + operation, stage
        assert stage["request_id"] == "request-" + operation, stage
        assert stage["output_root"] == "outputs/" + operation + "-1", stage
        if stage["process"] is None:
            continue
        frames = [json.loads(line) for line in stage["process"]["stdout"].splitlines()]
        assert len(frames) == 2 and [f["kind"] for f in frames] == ["hello_ack", "result"], frames
        ack, result = frames
        inputs = run / operation / "work/inputs"
        manifest_bytes = (inputs / base.PREFIX / "extension.json").read_bytes()
        manifest = json.loads(manifest_bytes)
        selected = next(c for c in manifest["components"] if c["id"] == component)
        assert selected["port"] == ("contract-reader" if operation == "read" else "generator"), selected
        assert selected["entrypoint"] == {"kind": "process", "path": fixture.ENTRY, "args": [operation]}, selected
        assert ack["extension_id"] == base.PACKAGE and ack["extension_version"] == "0.1.0", ack
        assert ack["component_id"] == component and ack["session_id"] == stage["session_id"], ack
        assert ack["manifest_digest"] == "sha256:" + hashlib.sha256(manifest_bytes).hexdigest(), ack
        assert ack["features"] == selected["supported_features"], ack
        assert set(json.loads((inputs / "profile.json").read_bytes())["required_features"]) <= set(ack["features"]), ack
        for frame in frames:
            assert frame["schema"] == "madb.extension-wire/1" and frame["protocol"] == frame["ir"] == "1.0", frame
        assert result["request_id"] == stage["request_id"] and result["session_id"] == stage["session_id"], result
        if fault == component + "-business-failure":
            assert result["status"] == "error" and "output" not in result, result
            assert result["error"]["code"] == "ToolFailed" and result["error"]["stage"] == operation, result
            assert stage["peer_diagnostic"] == result["error"], stage["peer_diagnostic"]
            continue
        assert result["status"] == "ok", result
        response = "contract.json" if operation == "read" else "outputs.json"
        assert result["output"]["path"] == stage["output_root"] + "/" + response, result
        assert result["output"]["schema"] == ("madb.contract-ir/1" if operation == "read" else "madb.output-set/1"), result


def check_run(result, run, fault):
    stages = result["stages"]
    stage_identities(stages, run, fault)
    cancelled = "cancel" in fault and fault != "cancel-commit"
    failed = fault.endswith(("output-drift", "business-failure"))
    assert result["sequence_status"] == ("Cancelled" if cancelled else "Failed" if failed else "Validated"), result["sequence_status"]
    if fault == "ok":
        assert result["committed"] and not result["index_unchanged"], result
        assert result["after_revision"] == result["before_revision"] + 1 and result["error"] is None, result
        assert len(stages) == 2, result
    else:
        assert not result["committed"] and result["index_unchanged"], result
        assert result["after_revision"] == result["before_revision"], result
        assert not (run / "public-source").exists(), result
        expected_stages = 1 if fault.startswith("reader-") else 2
        assert len(stages) == expected_stages, result
    for index, stage in enumerate(stages):
        affected = fault.startswith(("reader-" if index == 0 else "generator-"))
        if fault == "cancel-between" and index == 1:
            assert stage["admission"] == "RejectedByCancellation" and stage["process"] is None, stage
            assert stage["graph_status"] == "Cancelled" and stage["skipped_before_execution"] is True, stage
            assert not (run / "generate").exists(), "cancelled sequence launched generator workspace"
            continue
        if affected and fault.endswith("cancel-before"):
            assert stage["admission"] == "RejectedByCancellation" and stage["process"] is None, stage
            assert stage["graph_status"] == "Cancelled", stage
            continue
        process_complete(stage, cancelled=affected and fault.endswith("cancel-result"))
        if affected and fault.endswith("cancel-result"):
            assert stage["admission"] == "RejectedByCancellation", stage
            assert stage["graph_status"] == "Cancelled", stage
        elif affected and fault.endswith("output-drift"):
            assert stage["admission"] == "Unavailable", stage
            assert stage["graph_status"] == "Failed", stage
            assert stage["process"]["diagnostic"]["code"] == "OutputInvalid", stage
        elif affected and fault.endswith("business-failure"):
            assert stage["admission"] == "Unavailable", stage
            assert stage["graph_status"] == "Failed", stage
            assert stage["process"]["diagnostic"]["code"] == "ToolFailed", stage["process"]["diagnostic"]
        else:
            assert stage["admission"] == "EligibleForValidation", stage
            assert stage["graph_status"] == "Validated", stage
    if fault == "cancel-commit":
        assert "STORE002" in result["error"] and "cancelled before commit preparation" in result["error"], result
    if fault == "upstream-drift":
        assert result["error"] and "INPUT004" in result["error"] and "contract.json" in result["error"], result
    if len(stages) == 2:
        transfer = result["transfer"]
        assert transfer["exact_bytes"] and transfer["size"] > 0, result
        upstream, downstream = transfer["upstream"], transfer["downstream"]
        assert upstream["sha256"] == downstream["sha256"] and upstream["size"] == downstream["size"] == transfer["size"], transfer
        frozen = (run / "generate-frozen/contract.json").read_bytes()
        assert "sha256:" + hashlib.sha256(frozen).hexdigest() == downstream["sha256"], transfer
        if fault != "upstream-drift":
            assert frozen == (run / "read/work/outputs/read-1/contract.json").read_bytes(), result
    check_store(result, run, committed=fault == "ok")


def check_store(result, run, *, committed):
    """Inspect persisted index and baseline bytes, not just probe booleans."""
    store = run / "store"
    current = json.loads((store / "current.json").read_bytes())
    assert set(current) == {"schema", "revision", "source", "build", "candidate"}, current
    assert current["schema"] == "madb.storage-index/1" and current["build"] is current["candidate"] is None, current
    previous = list((store / "batches").glob("*/files/previous.txt"))
    assert len(previous) == 1 and previous[0].read_bytes() == b"previous complete source", previous
    assert (run / "baseline/previous.txt").read_bytes() == b"previous complete source"
    metadata = json.loads((previous[0].parents[1] / "metadata.json").read_bytes())
    baseline_digest = "sha256:" + hashlib.sha256(b"baseline").hexdigest()
    assert metadata["schema"] == "madb.storage-batch/1", metadata
    assert metadata["spec"] == {"layer": "source", "parent": None, **{
        field: baseline_digest for field in ("input_digest", "task_digest", "profile_digest", "generation_id")}}, metadata
    assert metadata["files"] == [{"path": "previous.txt", "size": 24,
                                  "sha256": "sha256:" + hashlib.sha256(b"previous complete source").hexdigest()}], metadata
    baseline_id = "sha256:" + previous[0].parents[1].name
    assert metadata["id"] == baseline_id, metadata
    assert current["source"] == result["source_batch"] and current["revision"] == result["after_revision"], current
    if not committed:
        assert current["source"] == baseline_id and current["revision"] == 1, current
    else:
        assert current["source"] != baseline_id and current["revision"] == 2, current
        published = store / "batches" / current["source"].removeprefix("sha256:") / "files"
        exported = run / "public-source"
        a = {p.relative_to(published).as_posix(): p.read_bytes() for p in published.rglob("*") if p.is_file()}
        b = {p.relative_to(exported).as_posix(): p.read_bytes() for p in exported.rglob("*") if p.is_file()}
        assert a == b, "published batch and exported Source bytes differ"


def business_failure(args, normal_root, tools, output, operation):
    root = output / "inputs"
    shutil.copytree(normal_root, root)
    config = base.support.read_json(root / "backend.json")
    if operation == "reader":
        config["public_source"] = "derived/nonexistent.ets"
    else:
        config["provider_factory"] = "unsupported_factory"
    d.save(root, "backend.json", config)
    fixture.repin(root, args.bridge, base.support.read_json(root / "read-request.json"))
    before = inventory(root)
    result = command([args.probe, root, tools, output / "run", "ok"], output / "failure")
    check_run(result, output / "run", operation + "-business-failure")
    assert inventory(root) == before


def reject_selection(args, normal_root, tools, output, profile_id):
    root = output / "inputs"
    shutil.copytree(normal_root, root)
    profile = base.support.read_json(root / "profile.json")
    profile["id"] = profile_id
    d.save(root, "profile.json", profile)
    composition = base.support.read_json(root / "composition.json")
    composition["selection"]["profile_id"] = profile_id
    d.save(root, "composition.json", composition)
    fixture.repin(root, args.bridge, base.support.read_json(root / "read-request.json"))
    command([args.probe, root, tools, output / "run", "ok"], output / "rejection",
            expected_error="unsupported fixed composition profile")
    assert not (output / "run").exists(), "unsupported selection must reject before execution"


def reject_admission(args, normal_root, tools, output, gate):
    root, selected_tools = fixture.prepare_rejection(args, normal_root, tools, output, gate)
    before = inventory(root)
    code, path = fixture.ADMISSION_GATES[gate]
    command([args.probe, root, selected_tools, output / "run", "ok"], output / "rejection",
            expected_error=f'code: "{code}", path: "{path}"')
    assert not (output / "run").exists(), "shared admission gate must reject before run or peer workspace allocation"
    assert inventory(root) == before
    return {"case": gate, "result": "passed", "code": code, "path": path,
            "scope": "rejected mutation of actual registered C07 inputs; no X backend configuration or implementation claim"}


def run(args):
    expected_python = (fixture.ROOT / ".python-version").read_text().strip()
    assert ".".join(map(str, sys.version_info[:3])) == expected_python, "fixed Python required"
    output = args.output.absolute()
    protected = [fixture.ROOT.resolve(), *(p.resolve().parent for p in (args.bridge, args.probe, args.peer)), args.observation.resolve().parent]
    if output.exists() or output.is_symlink() or any(output.resolve().is_relative_to(p) or p.is_relative_to(output.resolve()) for p in protected):
        raise ValueError("new output must not overlap source, observations or tool inputs")
    output.mkdir(parents=True)
    identities = {name: file_id(getattr(args, name).resolve()) for name in ("bridge", "probe", "peer", "observation")}
    identities["python"] = file_id(Path(sys.executable).resolve())
    identities["sources"] = inventory(Path(__file__).parent)
    d.save(output, "inputs-before.json", identities)
    records = []
    mode_roots = {}
    for mode in fixture.MODES:
        mode_output = output / mode
        mode_output.mkdir()
        root = mode_output / "inputs"
        tools = fixture.prepare(args, root, mode)
        mode_roots[mode] = (root, tools)
        before = inventory(root)
        d.save(mode_output, "source-before.json", before)
        run_root = mode_output / "run-ok"
        result = command([args.probe, root, tools, run_root, "ok"], mode_output / "ok")
        check_run(result, run_root, "ok")
        observed = command([sys.executable, "-I", "-S", "-B", Path(__file__).with_name("consume_source.py"), run_root / "public-source", mode], mode_output / "consume")
        assert observed["consumed_payloads"] == (17 if mode == "enhanced" else 15), observed
        tampered = mode_output / "tampered-source"
        shutil.copytree(run_root / "public-source", tampered)
        (tampered / "generated/facade/RuntimeFrame.ets").write_bytes(b"tampered")
        command([sys.executable, "-I", "-S", "-B", Path(__file__).with_name("consume_source.py"), tampered, mode], mode_output / "consumer-rejection", expected_error="public payload byte identity differs")
        assert inventory(root) == before, "original frozen input changed"
        assert not (root / "contract.json").exists() and not (root / "generate-request.json").exists()
        records.append({"case": mode + "-reader-generator-source-and-consumer", "result": "passed", "payloads": observed["consumed_payloads"]})
    root, tools = mode_roots["ordinary"]
    before = inventory(root)
    for fault in FAULTS:
        run_root = output / ("run-" + fault)
        result = command([args.probe, root, tools, run_root, fault], output / fault)
        check_run(result, run_root, fault)
        assert inventory(root) == before, "fault changed original input"
        records.append({"case": fault, "result": "passed"})
    for operation in ("reader", "generator"):
        destination = output / (operation + "-business-failure")
        destination.mkdir()
        business_failure(args, root, tools, destination, operation)
        records.append({"case": operation + "-business-failure", "result": "passed"})
    for kind, profile_id in (("unknown", "c07-c-arkts-chain-unknown-aarch64-ohos23"), ("unsupported-alternative-selection", "c07-c-arkts-chain-alternative-aarch64-ohos23")):
        destination = output / kind
        destination.mkdir()
        reject_selection(args, root, tools, destination, profile_id)
        records.append({"case": kind, "result": "passed", "scope": "unsupported fixed profile rejection, not alternative backend implementation"})
    for gate in fixture.ADMISSION_GATES:
        destination = output / gate
        destination.mkdir()
        records.append(reject_admission(args, root, tools, destination, gate))
    after = {name: file_id(getattr(args, name).resolve()) for name in ("bridge", "probe", "peer", "observation")}
    after["python"] = file_id(Path(sys.executable).resolve())
    after["sources"] = inventory(Path(__file__).parent)
    d.save(output, "inputs-after.json", after)
    assert after == identities, "test source or tool identity changed during execution"
    d.save(output, "checks.json", {"scope": "real two-component profile/2 Source execution", "checks": records,
        "not_claimed": ["SDK build", "HAR", "target runtime", "replacement backend implementation", "E01/E02/E03 whole-package acceptance"]})
    print(json.dumps({"checks": len(records), "output": str(output)}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("output", "bridge", "probe", "peer", "observation"):
        parser.add_argument("--" + name, type=Path, required=True)
    run(parser.parse_args())
