"""Controlled P01 peer. One handshake, then exit; never executes user code.

Fault modes are fixtures, not production extension implementations.
"""
import json
import sys
import time


def main():
    mode = sys.argv[1]
    hello = json.loads(sys.stdin.readline())
    if mode == "crash":
        print("controlled helper failure", file=sys.stderr)
        raise SystemExit(23)
    if mode == "timeout":
        time.sleep(10)
        return
    if mode == "noise":
        print("not a protocol frame")
        return
    if mode == "duplicate-key":
        print('{"kind":"hello_ack","kind":"reject"}')
        return
    if mode == "oversize":
        sys.stdout.write("x" * (1024 * 1024 + 1) + "\n")
        return
    if mode == "invalid-utf8":
        sys.stdout.buffer.write(b"\xff\n")
        return
    identity = ["extension_id", "extension_version", "component_id", "manifest_digest"]
    code = None
    if "1.0" not in hello["protocol_versions"] or "1.0" not in hello["ir_versions"]:
        code = "ProtocolIncompatible"
    elif set(hello["required_features"]) - {"core.call.sync@1"}:
        code = "CapabilityUnsupported"
    elif mode == "tool-error":
        code = "ToolFailed"
    if code:
        frame = {"schema": hello["schema"], "session_id": hello["session_id"], "kind": "reject",
                 "error": {"code": code, "message": "controlled probe rejection", "stage": "handshake"}}
    else:
        frame = {"schema": hello["schema"], "session_id": hello["session_id"], "kind": "hello_ack",
                 **{key: hello[key] for key in identity}, "protocol": "1.0", "ir": "1.0",
                 "features": ["core.call.sync@1"]}
        if mode == "wrong-identity":
            frame["extension_id"] = "wrong.helper"
        elif mode == "wrong-session":
            frame["session_id"] = "wrong-session"
        elif mode == "unoffered-version":
            frame["protocol"] = "1.1"
        elif mode == "extra-feature":
            frame["features"].append("core.unclaimed@1")
    print(json.dumps(frame))
    if mode == "duplicate-frame":
        print(json.dumps(frame))


if __name__ == "__main__":
    main()
