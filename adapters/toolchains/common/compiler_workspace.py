"""Owned, input-addressed compiler workspaces and bounded cancellable process groups."""
from __future__ import annotations
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import time
import selectors
import signal
import subprocess
import threading
import sys

SCHEMA = "madb.compiler-workspace/1"
ALLOWED = {"owner.json", "project", "home", "gradle-home", "build.log"}


def canonical(value) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def identity(inputs, observation, profile, interface) -> str:
    portable = {"schema": SCHEMA, "author_inputs": inputs, "profile": profile, "interface": interface,
        "tools": [{"key": key, **{name: tool[name] for name in ("id", "version", "sha256", "size")}}
            for key, tool in sorted(observation["tools"].items())],
        "trees": [{"key": key, **{name: tree[name] for name in ("sha256", "entries", "size")}}
            for key, tree in sorted(observation["trees"].items())], "xcode": observation["xcode"]}
    return hashlib.sha256(canonical(portable)).hexdigest()


def private_directory(path: Path) -> None:
    try:
        path.mkdir(mode=0o700)
    except FileExistsError:
        pass
    current = path.lstat()
    if not stat.S_ISDIR(current.st_mode) or current.st_uid != os.getuid() or stat.S_IMODE(current.st_mode) != 0o700:
        raise ValueError("Compiler workspace requires an owned 0700 directory: " + str(path))


def _clean_owned(root: Path) -> None:
    if (root / "compiler-process.json").exists() or (root / "compiler-process.json").is_symlink():
        raise ValueError("Interrupted compiler process marker present; refusing cleanup until orphan-process recovery")
    unexpected = {path.name for path in root.iterdir()} - ALLOWED
    if unexpected:
        raise ValueError("Unexpected compiler workspace files; refusing cleanup: " + repr(sorted(unexpected)))
    for name in ALLOWED - {"owner.json"}:
        path = root / name
        if path.is_symlink():
            raise ValueError("Compiler workspace child is a symlink: " + str(path))
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()


@contextmanager
def acquire(key: str, timeout: float = 90, *, base: Path):
    if len(key) != 64 or any(ch not in "0123456789abcdef" for ch in key):
        raise ValueError("Invalid compiler workspace identity")
    base = Path(base)
    if not base.is_absolute():
        raise ValueError("Compiler workspace base must be explicit and absolute")
    private_directory(base.parent)
    private_directory(base)
    lock_path = base / (key + ".lock")
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o600:
            raise ValueError("Compiler workspace lock ownership/mode mismatch")
        deadline = time.monotonic() + timeout
        while True:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise TimeoutError("Timed out waiting for the same compiler input identity")
                time.sleep(0.05)
        root = base / key
        private_directory(root)
        marker = root / "owner.json"
        expected = {"schema": SCHEMA, "input_digest": "sha256:" + key, "uid": os.getuid()}
        if marker.exists() or marker.is_symlink():
            info = marker.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or json.loads(marker.read_text()) != expected:
                raise ValueError("Compiler workspace ownership marker differs; refusing cleanup")
        else:
            if any(root.iterdir()):
                raise ValueError("Nonempty unowned compiler workspace")
            marker_fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(marker_fd, "wb") as stream:
                stream.write(canonical(expected) + b"\n")
        _clean_owned(root)
        yield root
    finally:
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)


def _quiescent_group(leader: int) -> bool:
    """Observe this group while its waitable leader reserves the numeric PGID.

    Approved compilers must not escape with setsid/setpgid. This is not daemon
    containment. Darwin libproc is an OS runtime dependency, bound by the observed
    system build; no unregistered process-inspection executable is launched.
    """
    if sys.platform == "darwin":
        import ctypes
        library = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        query = library.proc_listpids
        # libproc.h: int proc_listpids(uint32_t, uint32_t, void *, int).
        query.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_int]
        query.restype = ctypes.c_int
        values = (ctypes.c_int * 65536)()
        size = ctypes.sizeof(values)
        ctypes.set_errno(0)
        count = query(2, leader, values, size)  # PROC_PGRP_ONLY = 2
        error = ctypes.get_errno()
        if count < 0 or (count == 0 and error) or count >= size or count % ctypes.sizeof(ctypes.c_int):
            raise RuntimeError("Cannot completely observe compiler process group: " + str(error))
        return all(pid in (0, leader) for pid in values[:count // ctypes.sizeof(ctypes.c_int)])
    if sys.platform == "linux":
        with os.scandir("/proc") as entries:
            count = 0
            for entry in entries:
                if not entry.name.isascii() or not entry.name.isdigit():
                    continue
                count += 1
                if count > 65536:
                    raise RuntimeError("Process observation exceeds its supported bound")
                try:
                    with open(Path(entry.path) / "stat", "rb") as source:
                        value = source.read(4097)
                except (FileNotFoundError, ProcessLookupError):
                    continue
                closing = value.rfind(b")")
                fields = value[closing + 1:].split()
                if len(value) > 4096 or closing < 0 or len(fields) < 3 or len(fields[0]) != 1:
                    raise RuntimeError("Invalid bounded process observation")
                pid = int(value.split(b" (", 1)[0])
                group = int(fields[2])
                if pid != int(entry.name):
                    raise RuntimeError("Process observation identity differs")
                if group == leader and pid != leader and fields[0] not in (b"Z", b"X", b"x"):
                    return False
        return True
    raise RuntimeError("Compiler group observation requires macOS or Linux")


class _OwnedCompiler:
    """Sole waiter; no SIGCHLD auto-reap or competing waitpid(-1) is supported."""
    def __init__(self, process):
        self.process = process
        self.owned = True
        self.kill_sent = False

    def exited(self):
        if not self.owned:
            raise RuntimeError("Compiler child ownership is unavailable")
        try:
            return os.waitid(os.P_PID, self.process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        except ChildProcessError as error:
            self.owned = False
            raise RuntimeError("Compiler child ownership was lost to another waiter") from error

    def signal(self, number):
        exited = self.exited()  # Diagnose lost ownership before using the reserved PGID.
        if number == signal.SIGKILL:
            self.kill_sent = True
        try:
            os.killpg(self.process.pid, number)
        except ProcessLookupError:
            if exited is None:
                raise
        except PermissionError:
            # XNU may reject signals when only its excluded zombie leader remains.
            if sys.platform != "darwin" or exited is None or not _quiescent_group(self.process.pid):
                raise

    def reap_if_exited(self):
        if self.owned and self.exited() is not None:
            result = self.process.wait(timeout=0.1)
            self.owned = False
            return result
        return None


def _cleanup(owner):
    """Reserve five seconds inside the C06 owner's explicit six-second grace."""
    deadline = time.monotonic() + 5
    try:
        if not owner.kill_sent:
            if owner.exited() is None:
                owner.signal(signal.SIGTERM)
                graceful = time.monotonic() + 2
                while owner.exited() is None and time.monotonic() < graceful:
                    time.sleep(0.01)
            owner.signal(signal.SIGKILL)
        while time.monotonic() < deadline:
            if owner.exited() is not None and _quiescent_group(owner.process.pid):
                owner.reap_if_exited()
                return
            time.sleep(0.01)
        raise RuntimeError("Compiler group cleanup unconfirmed; retain process marker for recovery")
    finally:
        # All final group-signal attempts precede reaping. On failure retain the
        # marker even if the leader can be reaped; other members remain unproven.
        owner.reap_if_exited()


def execute(command, project, environment, log):
    previous = {}
    def interrupted(signum, frame):
        raise RuntimeError("Compiler process cancelled by signal " + str(signum))
    if threading.current_thread() is threading.main_thread():
        for number in (signal.SIGTERM, signal.SIGINT):
            previous[number] = signal.signal(number, interrupted)
    process = owner = None
    marker = project.parent / "compiler-process.json"
    marker_created = False
    spawn_started = False
    try:
        # A SIGKILL or unconfirmed cleanup quarantines this owned cold workspace.
        fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "w") as stream:
            json.dump({"schema": "madb.compiler-process/1", "launcher_pid": os.getpid()}, stream)
        marker_created = True
        with log.open("wb") as stream:
            spawn_started = True
            process = subprocess.Popen(command, cwd=project, env=environment, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
            owner = _OwnedCompiler(process)
            fd = os.open(marker, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
            with os.fdopen(fd, "w") as record:
                json.dump({"schema": "madb.compiler-process/1", "launcher_pid": os.getpid(),
                           "leader_pid": process.pid, "process_group": process.pid}, record)
            deadline = time.monotonic() + 900
            retained = 0
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                while selector.get_map():
                    if time.monotonic() >= deadline:
                        raise TimeoutError("Compiler execution or pipe cleanup exceeded its deadline")
                    if owner.exited() is not None and not owner.kill_sent:
                        owner.signal(signal.SIGKILL)
                        deadline = min(deadline, time.monotonic() + 3)
                    for key, _ in selector.select(0.05):
                        data = os.read(key.fileobj.fileno(), 65536)
                        if not data:
                            selector.unregister(key.fileobj)
                            continue
                        retained += len(data)
                        if retained > 4 * 1024 * 1024:
                            raise RuntimeError("Compiler log exceeded 4 MiB")
                        stream.write(data)
            status = owner.exited()
            while status is None:
                if time.monotonic() >= deadline:
                    raise TimeoutError("Compiler exceeded 900 seconds")
                time.sleep(0.01)
                status = owner.exited()
            return status.si_status if status.si_code == os.CLD_EXITED else -status.si_status
    finally:
        try:
            if owner is not None:
                _cleanup(owner)
                if marker_created:
                    marker.unlink()
            elif marker_created and not spawn_started:
                marker.unlink()
        finally:
            if process is not None and process.stdout is not None:
                process.stdout.close()
            for number, handler in previous.items():
                signal.signal(number, handler)
