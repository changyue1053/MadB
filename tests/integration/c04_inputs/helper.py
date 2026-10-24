"""Trusted fixture tool; expected values come from test_frozen_inputs.py."""
from pathlib import Path
import hashlib
import json
import os
import signal
import sys


def record_pid():
    signal.alarm(15)
    with open("pids", "a") as stream:
        stream.write(f"{os.getpid()}\n")


def wait_forever():
    while True:
        signal.pause()


def result(value):
    data = value.read_bytes()
    observed = {"text": data.decode(), "sha256": hashlib.sha256(data).hexdigest(), "length": len(data)}
    Path("result.json").write_text(json.dumps(observed))
    os.write(1, json.dumps(observed).encode() + b"\n")


def mutate(value, mode):
    inputs = value.parent
    if mode == "mutate-input":
        value.chmod(0o600)
        value.write_bytes(b"tampered input")
    elif mode == "add-file":
        inputs.chmod(0o700)
        (inputs / "undeclared").write_text("unexpected")
    elif mode == "add-directory":
        inputs.chmod(0o700)
        (inputs / "undeclared").mkdir()
    elif mode in ("replace-symlink", "replace-hardlink"):
        inputs.chmod(0o700)
        target = Path("replacement").resolve()
        target.write_bytes(value.read_bytes())
        value.unlink()
        if mode == "replace-symlink":
            value.symlink_to(target)
        else:
            os.link(target, value)
    elif mode == "remove-input":
        inputs.chmod(0o700)
        value.unlink()


def main():
    mode = sys.argv[1]
    value = Path(sys.argv[2])
    record_pid()
    if mode == "cancel":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        Path("partial").write_text("owned scratch")
        Path("ready").touch()
        wait_forever()
    elif mode in ("escaped-pipe", "escaped-closed-pipes"):
        read_fd, write_fd = os.pipe()
        child = os.fork()
        if child == 0:
            os.close(read_fd)
            os.setsid()
            record_pid()
            if mode == "escaped-closed-pipes":
                for fd in (0, 1, 2):
                    os.close(fd)
            os.write(write_fd, b"ready")
            os.close(write_fd)
            wait_forever()
        os.close(write_fd)
        assert os.read(read_fd, 5) == b"ready"
        os.close(read_fd)
        result(value)
        os._exit(0)
    elif mode == "normal":
        result(value)
    else:
        mutate(value, mode)
        # The tool reports exit zero; the independent host check must reject it.
        Path("result.json").write_text(json.dumps({"tool_claim": "complete", "mode": mode}))
        os.write(1, b'{"tool_claim":"complete"}\n')


if __name__ == "__main__":
    main()
