"""Real public-author peer process checks with controlled business payloads.

This tests port/IO/cancellation boundaries. It does not run the production
ArkTS generator, compile an SDK, or grant a Source transaction receipt.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
PACKAGE = "madb.local.c07-composition"
PACKAGE_ROOT = f"inputs/packages/{PACKAGE}/0.1.0"
PYTHON_ARGS = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]


def raw(value):
    return (json.dumps(value, separators=(",", ":")) + "\n").encode()


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def identity(path):
    data = path.read_bytes()
    return {"path": str(path), "sha256": digest(data), "size": len(data)}


def now():
    return datetime.now(timezone.utc).isoformat()


def frame(proc, value, sent):
    encoded = raw(value)
    proc.stdin.write(encoded)
    proc.stdin.flush()
    sent.append(value)


def line(proc, received, timeout=10):
    if not select.select([proc.stdout], [], [], timeout)[0]:
        raise AssertionError("protocol frame timed out")
    encoded = proc.stdout.readline()
    if not encoded:
        raise AssertionError("unexpected EOF: " + proc.stderr.read().decode())
    received.append(encoded.decode())
    return json.loads(encoded)


def fixture(name):
    return json.loads((ROOT / "tests/fixtures/contracts" / (name + ".json")).read_bytes())


def case(out, peer, python, mode):
    operation = "read" if mode.startswith("read-") else "generate"
    component = "reader" if operation == "read" else "generator"
    arguments = [] if mode == "legacy-generate" else [operation]
    root = out / mode
    work = root / "work"
    package = work / PACKAGE_ROOT
    (package / "adapters/generators/arkts").mkdir(parents=True)
    (work / "outputs/request-1").mkdir(parents=True)
    manifest = {"schema": "madb.extension/1", "id": PACKAGE, "version": "0.1.0",
                "compatibility": {"protocol": ["1.0"], "ir": ["1.0"]}, "languages": [],
                "components": [{"id": component, "port": "contract-reader" if operation == "read" else "generator",
                                "supported_features": ["core.values@1", "core.extension-composition@1"],
                                "entrypoint": {"kind": "process", "path": "peer", "args": arguments}}],
                "dependencies": [], "routes": [], "assets": [],
                "execution": {"transport": "stdio-jsonl", "network": "none", "env": []}}
    manifest_bytes = raw(manifest)
    (package / "extension.json").write_bytes(manifest_bytes)
    request = fixture("read-request" if operation == "read" else "generate-request")
    request["component"]["id"] = PACKAGE + "/" + component
    request["profile"]["schema"] = "madb.profile/2"
    request["output_root"] = "outputs/request-1"
    if mode == "read-wrong-domain":
        request["output_root"] = "outputs/another"
    request_file = "read-request.json" if operation == "read" else "generate-request.json"
    request_bytes = raw(request)
    (work / "inputs" / request_file).write_bytes(request_bytes)
    config = {"tools": {"python": {**python, "id": "c07-python", "invocation_arguments": PYTHON_ARGS}}}
    (work / "inputs/backend.json").write_bytes(raw(config))
    result = fixture("contract-ir" if operation == "read" else "output-set")
    result_file = "contract.json" if operation == "read" else "outputs.json"
    if mode == "read-wrong-result":
        result = fixture("output-set")
    script = """from pathlib import Path
import json,os,sys,time
args=sys.argv
root=Path(args[args.index('--output-root')+1])
work=root.parent.parent
assert args[args.index('--operation')+1] == OPERATION
(work/'business-started.json').write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp()}))
""".replace("OPERATION", repr(operation))
    if mode in ("read-cancel", "generate-cancel", "read-force-cleanup", "generate-force-cleanup"):
        script += "while not (work/'release-business').exists(): time.sleep(.01)\n"
    if mode.endswith("-failed"):
        script += 'raise RuntimeError("controlled business failure")\n'
    elif mode.endswith("-oversized"):
        script += 'sys.stdout.write("x"*(1024*1024+1))\n'
    else:
        script += "data=" + repr(raw(result)) + "\n"
        script += "(root/" + repr(result_file) + ").write_bytes(data)\n"
        if mode == "read-extra-output":
            script += "(root/'extra.txt').write_text('unclaimed')\n"
        if mode == "read-result-bytes":
            script += "(root/'contract.json').write_bytes(data+b' ')\n"
        script += "sys.stdout.buffer.write(data)\n"
    (package / "adapters/generators/arkts/composition_peer.py").write_text(script)
    started = now()
    proc = subprocess.Popen([str(peer), *arguments], cwd=work, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    sent, received = [], []
    observed_child = None
    terminal = None
    status = "error"
    stderr = b""
    failure = None
    try:
        hello = {"schema": "madb.extension-wire/1", "kind": "hello", "session_id": "s1",
                 "extension_id": PACKAGE, "extension_version": "0.1.0", "component_id": component,
                 "manifest_digest": digest(manifest_bytes), "protocol_versions": ["1.0"],
                 "ir_versions": ["1.0"], "required_features": ["core.extension-composition@1"]}
        if mode == "read-wrong-identity":
            hello["component_id"] = "generator"
        frame(proc, hello, sent)
        ack = line(proc, received)
        if mode == "read-wrong-identity":
            assert ack["kind"] == "reject" and ack["error"]["code"] == "ExtensionIdentityMismatch"
            assert proc.wait(timeout=5) == 0
            terminal = "reject"
        else:
            assert ack["kind"] == "hello_ack" and ack["component_id"] == component
            assert "core.extension-composition@1" in ack["features"]
            invoke = {"schema": "madb.extension-wire/1", "kind": "invoke", "session_id": "s1",
                      "request_id": "r1", "protocol": "1.0", "ir": "1.0", "component": PACKAGE + "/" + component,
                      "operation": operation, "input": {"path": "inputs/" + request_file,
                      "schema": request["schema"], "sha256": digest(request_bytes), "size": len(request_bytes)},
                      "output_root": "outputs/request-1", "timeout_ms": 10000}
            if mode == "read-wrong-schema":
                invoke["input"]["schema"] = "madb.generate-request/1"
            if mode == "read-wrong-digest":
                invoke["input"]["sha256"] = "sha256:" + "0" * 64
            frame(proc, invoke, sent)
            if mode.endswith("-cancel") or mode.endswith("-force-cleanup"):
                marker = work / "business-started.json"
                deadline = time.monotonic() + 5
                while not marker.exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                assert marker.exists(), "controlled business did not start"
                observed_child = json.loads(marker.read_bytes())
                assert observed_child["pgid"] == proc.pid, "derived child escaped the outer group"
                frame(proc, {"schema": "madb.extension-wire/1", "kind": "cancel", "session_id": "s1",
                             "request_id": "r1", "protocol": "1.0", "ir": "1.0"}, sent)
                if mode.endswith("-force-cleanup"):
                    os.killpg(proc.pid, signal.SIGKILL)
                    assert proc.wait(timeout=5) == -signal.SIGKILL
                    assert proc.stdout.read() == b""
                    terminal = "none; outer group killed"
                else:
                    assert not select.select([proc.stdout], [], [], .2)[0], "terminal before child release"
                    (work / "release-business").write_text("release")
            if terminal is None:
                result = line(proc, received)
                terminal = result["status"]
                assert proc.wait(timeout=5) == 0
                if mode.endswith("-cancel"):
                    assert terminal == "cancelled" and "output" not in result
                    try:
                        os.kill(observed_child["pid"], 0)
                    except ProcessLookupError:
                        pass
                    else:
                        raise AssertionError("cancelled while owned business process still exists")
                elif mode in ("read-wrong-schema", "read-wrong-digest", "read-wrong-domain"):
                    assert terminal == "error" and result["error"]["code"] == "InputInvalid"
                elif mode in ("read-wrong-result", "read-extra-output", "read-result-bytes"):
                    assert terminal == "error" and result["error"]["code"] == "OutputInvalid"
                elif mode.endswith("-failed") or mode.endswith("-oversized"):
                    assert terminal == "error" and result["error"]["code"] == "ToolFailed"
                else:
                    assert terminal == "ok"
                    actual = (work / result["output"]["path"]).read_bytes()
                    assert result["output"]["sha256"] == digest(actual) and result["output"]["size"] == len(actual)
                    assert result["output"]["schema"] == ("madb.contract-ir/1" if operation == "read" else "madb.output-set/1")
                    if operation == "read":
                        assert sorted(p.name for p in (work / "outputs/request-1").iterdir()) == ["contract.json"]
                if terminal == "error":
                    assert result["error"]["stage"] == operation
        assert proc.stdout.read() == b"", "second terminal or trailing control output"
        stderr = proc.stderr.read()
        status = "passed"
    except Exception as error:
        failure = str(error)
        raise
    finally:
        recovery_kill = False
        if status != "passed":
            # A faulty early terminal may already have let the leader exit.
            # Retain responsibility for its known controlled process group.
            try:
                os.killpg(proc.pid, signal.SIGKILL)
                recovery_kill = True
            except ProcessLookupError:
                pass
        if proc.poll() is None:
            proc.wait(timeout=5)
        if not stderr:
            stderr = proc.stderr.read()
        (root / "control-sent.json").write_bytes(raw(sent))
        (root / "control-received.jsonl").write_text("".join(received))
        (root / "stderr.log").write_bytes(stderr)
        record = {"case": mode, "result": status, "started_at": started, "finished_at": now(),
                  "command": [str(peer), *arguments], "peer_pid": proc.pid, "exit": proc.returncode,
                  "terminal": terminal, "controlled_child": observed_child, "failure": failure,
                  "recovery_kill": recovery_kill}
        (root / "result.json").write_bytes(raw(record))
    return record


def reject_arguments(out, peer, arguments, name):
    root = out / name
    root.mkdir()
    started = now()
    result = subprocess.run([str(peer), *arguments], cwd=root, capture_output=True, timeout=5)
    (root / "stdout.log").write_bytes(result.stdout)
    (root / "stderr.log").write_bytes(result.stderr)
    assert result.returncode != 0 and result.stdout == b""
    assert b"peer accepts only one fixed read or generate argument" in result.stderr
    record = {"case": name, "result": "passed", "started_at": started, "finished_at": now(),
              "command": [str(peer), *arguments], "exit": result.returncode}
    (root / "result.json").write_bytes(raw(record))
    return record


def run(args):
    peer = args.peer.resolve(strict=True)
    python_path = Path(sys.executable).resolve(strict=True)
    expected_python = (ROOT / ".python-version").read_text().strip()
    assert ".".join(map(str, sys.version_info[:3])) == expected_python, "use pinned Python"
    if args.output.is_symlink():
        raise ValueError("symbolic output is not allowed")
    output = args.output.resolve()
    for protected in (ROOT.resolve(), peer.parent, python_path.parent):
        if output == protected or output in protected.parents or protected in output.parents:
            raise ValueError("output overlaps source or tool directory")
    if output.exists() or output.is_symlink():
        raise ValueError("new output directory required")
    initial = {"peer": identity(peer), "python": identity(python_path), "script": identity(Path(__file__).resolve())}
    output.mkdir(parents=True)
    modes = ["legacy-generate", "generate-ok", "read-ok", "read-wrong-identity", "read-wrong-schema",
             "read-wrong-digest", "read-wrong-domain", "read-wrong-result", "read-extra-output", "read-result-bytes",
             "read-failed", "read-oversized", "generate-failed", "generate-oversized", "read-cancel", "generate-cancel",
             "read-force-cleanup", "generate-force-cleanup"]
    results = [case(output, peer, initial["python"], mode) for mode in modes]
    results.extend([
        reject_arguments(output, peer, ["build"], "unknown-port-argument"),
        reject_arguments(output, peer, ["read", "generate"], "multiple-port-arguments"),
    ])
    assert initial["peer"] == identity(peer) and initial["python"] == identity(python_path)
    report = {"scope": "public author peer with controlled business payloads; not production generation or SDK runtime",
              "tools": initial, "cases": results}
    (output / "results.json").write_bytes(raw(report))
    print(json.dumps({"checks": len(results), "output": str(output)}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--peer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    run(parser.parse_args())
