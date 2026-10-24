"""The observed Python standard runtime, excluding only disabled import locations."""
from __future__ import annotations

import json
import os
from pathlib import Path
import stat
import subprocess

from .install_tools import digest

SCOPE = "python-runtime-v1"
ISOLATION = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"]
EXCLUDED = {"__pycache__", "site-packages"}


def inventory(root):
    root = Path(root).resolve(strict=True)
    result = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if EXCLUDED.intersection(relative.parts):
            continue
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            if not path.resolve(strict=True).is_relative_to(root):
                raise ValueError("Python runtime symlink escapes its root: " + str(relative))
            result.append(dict(path=relative.as_posix(), symlink=os.readlink(path)))
        elif stat.S_ISREG(mode):
            result.append(dict(path=relative.as_posix(), sha256=digest(path), size=path.stat().st_size,
                               executable=bool(mode & 0o111)))
        elif not stat.S_ISDIR(mode):
            raise ValueError("Python runtime contains a non-regular entry: " + str(relative))
    return result


def describe(image):
    source = "import json,sys;print(json.dumps({'root':sys.base_prefix,'search_paths':sys.path,'version':sys.version.split()[0]}))"
    value = json.loads(subprocess.check_output([str(image), *ISOLATION, "-c", source], text=True, timeout=30))
    root = Path(value["root"]).resolve(strict=True)
    for item in value["search_paths"]:
        # Even absent pythonXY.zip locations must stay in the frozen runtime tree;
        # adding such a zip later changes that tree and invalidates the observation.
        path = Path(item).resolve()
        if not path.is_relative_to(root) or EXCLUDED.intersection(path.relative_to(root).parts):
            raise ValueError("isolated Python imports from outside its runtime tree")
    value["root"] = str(root)
    return value


def native_dependencies(otool, root, image):
    """Follow explicit Mach-O linkage. OS shared-cache libraries remain host inputs."""
    root = Path(root)
    pending = [Path(image)] + [p for p in root.rglob("*")
        if p.is_file() and not EXCLUDED.intersection(p.relative_to(root).parts)
        and (p.suffix in (".so", ".dylib") or p == root / "Python")]
    seen, external, system = set(), {}, set()
    while pending:
        batch = sorted({str(p.resolve(strict=True)) for p in pending} - seen)
        pending = []
        if not batch:
            break
        if len(seen) + len(batch) > 512:
            raise ValueError("Python native dependency closure exceeds supported image count")
        output = subprocess.check_output([str(otool), "-L", *batch], text=True, timeout=30)
        if len(output) > 4 * 1024 * 1024:
            raise ValueError("Python linkage metadata exceeds its bound")
        seen.update(batch)
        for line in output.splitlines():
            if not line.startswith("\t"):
                continue
            name = line.strip().split(" (compatibility version", 1)[0]
            if name.startswith(("/usr/lib/", "/System/Library/")):
                system.add(name)
                continue
            if not Path(name).is_absolute():
                raise ValueError("unsupported relative Python native dependency: " + name)
            resolved = Path(name).resolve(strict=True)
            if not resolved.is_file():
                raise ValueError("Python native dependency is not a file")
            if not resolved.is_relative_to(root):
                external[name] = dict(path=name, image_path=str(resolved), sha256=digest(resolved), size=resolved.stat().st_size)
            pending.append(resolved)
    return dict(libraries=[external[k] for k in sorted(external)], system_libraries=sorted(system))


def verify(observation):
    python = observation["tools"]["python"]
    runtime = observation["python_runtime"]
    if python.get("invocation_arguments") != ISOLATION:
        raise ValueError("Python helpers require isolated imports without site or bytecode caches")
    for entry in runtime["libraries"]:
        path = Path(entry["path"])
        if (not path.is_file() or path.resolve(strict=True) != Path(entry["image_path"])
                or path.stat().st_size != entry["size"] or digest(path) != entry["sha256"]):
            raise ValueError("Python native dependency changed: " + entry["path"])
