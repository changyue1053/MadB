"""Trusted real process; test expectations are separately calculated in fixture.py."""
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
    inputs = sorted(item["path"] for item in task["inputs"] if item["path"] != "helper.py")
    data = b"".join(Path("../inputs", path).read_bytes() for path in inputs) + b"|" + task["id"].encode()
    for name in task["outputs"]:
        Path(name).parent.mkdir(parents=True, exist_ok=True)
        Path(name).write_bytes(data)
    log = "logs/" + task["id"] + ".txt"
    Path(log).parent.mkdir(parents=True)
    Path(log).write_bytes(b"completed:" + task["id"].encode())
    if mode == "cancel-running":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        Path(marker).write_text(json.dumps({"pid": os.getpid(), "ready": True}))
        while True:
            signal.pause()
    if mode == "bad-json":
        os.write(1, b"not-json\n")
        return
    evidence = {
        "schema": "madb.build-evidence/1", "task_id": task["id"],
        "task_digest": task["task_digest"], "input_digest": task["input_digest"],
        "profile_digest": task["selector"]["profile_digest"],
        "generation_id": task["generation_id"], "tool": task["tool"],
        "phase": task["phase"], "target": task["selector"]["target"],
        "status": "passed", "exit_code": 0,
        "outputs": [reference(name) for name in task["outputs"]], "logs": [reference(log)],
    }
    if mode == "wrong-output-hash":
        evidence["outputs"][0]["sha256"] = "sha256:" + "0" * 64
    elif mode == "wrong-task":
        evidence["task_id"] = "unrelated-node"
    os.write(1, json.dumps(evidence).encode() + b"\n")
    if mode == "nonzero":
        raise SystemExit(7)


if __name__ == "__main__":
    main()
