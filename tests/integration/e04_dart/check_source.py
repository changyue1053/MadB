"""Real public two-session source checks; no package/runtime acceptance claim."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from datetime import datetime, timezone

import inputs


def pin(path):
    path = Path(path)
    data = path.read_bytes()
    return {"path": str(path), "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def inventory(root):
    return [dict(pin(p), path=p.relative_to(root).as_posix()) for p in sorted(root.rglob("*")) if p.is_file()]


def check(value, mode):
    success = mode == "ok"
    assert value["committed"] is success, value
    assert value["index_unchanged"] is not success, value
    assert value["after_revision"] == value["before_revision"] + int(success), value
    for stage in value["stages"]:
        process = stage["process"]
        if process is None:
            assert stage["skipped_before_execution"] and stage["admission"] == "RejectedByCancellation", stage
            continue
        assert process["cleanup"]["locally_complete"] and process["cleanup"]["leader_reaped"], stage
        assert process["cleanup"]["stdout_eof"] and process["cleanup"]["stderr_eof"], stage
        assert not process["cleanup"]["signal_failed"] and not process["pending_cleanup"], stage
        assert not process["stdout_truncated"] and not process["stderr_truncated"], stage
        frames = [json.loads(line) for line in process["stdout"].splitlines()]
        if stage["admission"] == "EligibleForValidation":
            assert process["termination"] == "Exited(ProcessExit { code: Some(0), signal: None })", stage
            assert [f["kind"] for f in frames] == ["hello_ack", "result"], stage
            assert frames[0]["component_id"] == stage["component"], stage
            assert frames[1]["status"] == "ok", stage
        elif stage["admission"] == "RejectedByCancellation":
            assert process["termination"] in (
                "Exited(ProcessExit { code: Some(0), signal: None })",
                "Stopped { reason: Cancellation, forced: false, exit: Some(ProcessExit { code: Some(0), signal: None }) }",
            ), stage
    if success:
        assert value["sequence_status"] == "Validated" and len(value["stages"]) == 2, value
        assert [s["component"] for s in value["stages"]] == ["reader", "generator"], value
        assert all(s["admission"] == "EligibleForValidation" for s in value["stages"]), value
        assert value["transfer"]["exact_bytes"] and value["transfer"]["upstream"] == value["transfer"]["downstream"], value
    elif mode in ("cancel-accept", "cancel-between") or "cancel-" in mode and mode != "cancel-commit":
        assert value["sequence_status"] == "Cancelled", value
    elif mode.endswith("output-drift"):
        assert value["sequence_status"] == "Failed", value
        last = value["stages"][-1]
        assert last["admission"] == "Unavailable" and last["process"]["diagnostic"]["code"] == "OutputInvalid", value
    elif mode == "upstream-drift":
        assert value["sequence_status"] == "Validated" and value["error"], value
    elif mode == "cancel-commit":
        assert value["sequence_status"] == "Validated" and "STORE002" in value["error"], value


def main():
    parser = argparse.ArgumentParser()
    for name in ("output", "candidate", "clang", "sdk", "bridge", "peer", "ffigen", "libclang"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    root = args.output.absolute()
    protected = [inputs.ROOT.resolve(), *(getattr(args, n).resolve() for n in ("candidate", "clang", "sdk", "bridge", "peer", "ffigen", "libclang"))]
    if root.exists() or root.is_symlink() or root.parent.resolve() != root.parent:
        raise ValueError("fresh absolute output with canonical parent required")
    if any(root.is_relative_to(p) or p.is_relative_to(root) for p in protected):
        raise ValueError("output overlaps protected input")
    root.mkdir()
    started = datetime.now(timezone.utc).isoformat()
    inputs.prepare(args, root / "inputs")
    (root / "tools").mkdir()
    shutil.copyfile(args.peer, root / "tools/dart-peer")
    (root / "tools/dart-peer").chmod(0o500)
    tool_pins = [pin(getattr(args, name).resolve()) for name in ("bridge", "peer", "ffigen", "clang", "libclang")]
    before = inventory(root / "inputs")
    (root / "before.json").write_text(json.dumps({"inputs": before, "tools": tool_pins}, indent=2))
    records = []
    modes = ["ok", "ok", "reader-cancel-result", "generator-cancel-result", "cancel-between", "cancel-accept", "cancel-commit", "reader-output-drift", "generator-output-drift", "upstream-drift"]
    for index, mode in enumerate(modes):
        directory = root / f"{index:02d}-{mode}"
        argv = [str(args.bridge), "chain", str(root / "inputs"), str(root / "tools"), str(directory), mode]
        result = subprocess.run(argv, capture_output=True, timeout=240)
        (root / f"{index:02d}.stdout").write_bytes(result.stdout)
        (root / f"{index:02d}.stderr").write_bytes(result.stderr)
        assert result.returncode == 0, result.stderr.decode(errors="replace")
        value = json.loads(result.stdout)
        check(value, mode)
        records.append({"mode": mode, "argv": argv, "exit_code": result.returncode,
                        "stdout": pin(root / f"{index:02d}.stdout"), "stderr": pin(root / f"{index:02d}.stderr")})
        (root / "checks.json").write_text(json.dumps(records, indent=2))
    assert inventory(root / "00-ok/public-source") == inventory(root / "01-ok/public-source")
    assert inventory(root / "inputs") == before
    assert [pin(p["path"]) for p in tool_pins] == tool_pins
    (root / "after.json").write_text(json.dumps({"inputs": inventory(root / "inputs"), "tools": tool_pins}, indent=2))
    (root / "result.json").write_text(json.dumps({"scope": "Source only: two public AuthorSessions, frozen IR, atomic C04 Source", "started_at": started, "finished_at": datetime.now(timezone.utc).isoformat(), "status": "passed", "checks": len(records), "not_claimed": ["Build", "Package", "SDK consumer", "whole E04"]}, indent=2))
    print(f"passed {len(records)} source checks")


if __name__ == "__main__":
    main()
