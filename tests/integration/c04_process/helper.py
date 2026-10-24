"""Real process fixture. Never imported by production; every worker has a watchdog."""
from pathlib import Path
import hashlib
import json
import os
import signal
import sys
import time


def record_pid():
    signal.alarm(15)
    with open("pids", "a") as stream:
        stream.write(f"{os.getpid()}\n")


def wait_forever():
    while True:
        signal.pause()


def emit(data):
    os.write(1, json.dumps(data).encode() + b"\n")


def write_all(fd, data):
    while data:
        data = data[os.write(fd, data):]


def main():
    mode = sys.argv[1]
    record_pid()
    if mode == "normal":
        emit({"arguments": sys.argv[2:], "allowed": os.environ.get("SAFE_VALUE"),
              "ambient": os.environ.get("SECRET"), "cwd": Path.cwd().name,
              "input": sys.stdin.read()})
    elif mode == "exit-failure":
        raise SystemExit(23)
    elif mode == "self-signal":
        os.kill(os.getpid(), signal.SIGTERM)
    elif mode == "stdout-flood":
        while True:
            os.write(1, b"x" * 4096)
    elif mode == "stderr-flood":
        write_all(2, b"e" * (2 * 1024 * 1024))
        emit({"done": True})
    elif mode == "backpressure":
        write_all(2, b"e" * (128 * 1024))
        data = sys.stdin.buffer.read()
        emit({"length": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    elif mode in ("sleep", "ignore-term", "no-read", "close-stdin"):
        if mode in ("ignore-term", "no-read"):
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
        if mode == "close-stdin":
            os.close(0)
        Path("ready").touch()
        wait_forever()
    elif mode == "wait-release":
        Path("ready").touch()
        while not Path("release").exists():
            time.sleep(0.005)
        emit({"done": True})
    elif mode in ("tree", "parent-exits", "escaped-pipe"):
        # A pipe barrier establishes the child group/handler before the parent
        # publishes readiness. Scheduling sleeps are not the cancellation oracle.
        read_fd, write_fd = os.pipe()
        if mode == "tree":
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
        pid = os.fork()
        if pid == 0:
            os.close(read_fd)
            if mode == "escaped-pipe":
                os.setsid()
            record_pid()
            os.write(write_fd, b"ready")
            os.close(write_fd)
            wait_forever()
        os.close(write_fd)
        assert os.read(read_fd, 5) == b"ready"
        os.close(read_fd)
        Path("ready").touch()
        if mode != "tree":
            os._exit(0)
        wait_forever()
    else:
        raise AssertionError(f"unknown fixture mode: {mode}")


if __name__ == "__main__":
    main()
