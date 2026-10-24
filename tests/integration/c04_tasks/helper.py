"""Trusted test recipe. Product expectations live independently in test_tasks.py."""
from pathlib import Path
import hashlib
import json
import os
import signal
import sys


def reference(path):
    data = Path(path).read_bytes()
    return {"path": path, "size": len(data), "sha256": "sha256:" + hashlib.sha256(data).hexdigest()}


def main():
    mode, marker = sys.argv[1:3]
    signal.alarm(15)
    Path(marker).write_text(json.dumps({"pid": os.getpid(), "ready": False}))
    task = json.load(sys.stdin)
    value = Path("../inputs/value").read_bytes()
    Path("output.bin").write_bytes(value.upper() + b"\n")
    Path("logs").mkdir()
    Path("logs/build.txt").write_bytes(b"fixture recipe completed\n")
    if mode == "cancel-running":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        Path(marker).write_text(json.dumps({"pid": os.getpid(), "ready": True}))
        while True:
            signal.pause()
    evidence = {
        "schema": "madb.build-evidence/1", "task_id": task["id"],
        "task_digest": task["task_digest"], "input_digest": task["input_digest"],
        "profile_digest": task["selector"]["profile_digest"],
        "generation_id": task["generation_id"], "tool": task["tool"],
        "phase": task["phase"], "target": task["selector"]["target"],
        "status": "passed", "exit_code": 0,
        "outputs": [reference("output.bin")], "logs": [reference("logs/build.txt")],
    }
    if mode == "bad-json":
        os.write(1, b"not-json\n")
        return
    if mode.startswith("wrong-"):
        field = mode.removeprefix("wrong-")
        if field == "task":
            evidence["task_id"] = "different-task"
        elif field in ["generation", "profile", "input"]:
            evidence[field + ("_id" if field == "generation" else "_digest")] = "sha256:" + "0" * 64
        elif field == "output-hash":
            evidence["outputs"][0]["sha256"] = "sha256:" + "0" * 64
        elif field == "log-hash":
            evidence["logs"][0]["sha256"] = "sha256:" + "0" * 64
    elif mode == "missing-output":
        Path("output.bin").unlink()
    elif mode == "missing-log":
        Path("logs/build.txt").unlink()
    elif mode == "extra-file":
        Path("undeclared").write_bytes(b"extra")
    elif mode == "extra-directory":
        Path("undeclared").mkdir()
    elif mode in ["symlink-output", "hardlink-output", "fifo-output"]:
        output = Path("output.bin")
        output.unlink()
        if mode == "symlink-output":
            output.symlink_to(Path("../inputs/value").resolve())
        elif mode == "hardlink-output":
            target = Path(marker + ".target")
            target.write_bytes(value.upper() + b"\n")
            os.link(target, output)
        else:
            os.mkfifo(output)
    elif mode == "oversized-declaration":
        evidence["outputs"][0]["size"] = 32 * 1024 * 1024 + 1
    elif mode == "oversized-file":
        with open("output.bin", "wb") as stream:
            stream.truncate(32 * 1024 * 1024 + 1)
    elif mode == "failed-evidence":
        evidence["status"] = "failed"
        evidence["exit_code"] = 2
    elif mode == "log-overlap":
        evidence["logs"] = [evidence["outputs"][0]]
    elif mode == "log-case-alias":
        evidence["logs"].append(dict(evidence["logs"][0], path="Logs/other.txt"))
    elif mode == "input-tamper":
        path = Path("../inputs/value")
        path.chmod(0o600)
        path.write_bytes(b"changed during execution")
    os.write(1, json.dumps(evidence).encode() + b"\n")
    if mode == "nonzero-claimed-passed":
        raise SystemExit(7)


if __name__ == "__main__":
    main()
