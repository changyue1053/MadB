"""Small author-side implementation of the existing bounded JSON wire rules.

No product internals are imported. Host-side public protocol decoders remain
the authority for schema, semantic admission and output acceptance.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess

MAX_DOCUMENT = 1024 * 1024


def canonical(value):
    def normalize(item):
        if isinstance(item, dict):
            return {k: normalize(item[k]) for k in sorted(item, key=lambda k: k.encode("utf-16-be"))}
        if isinstance(item, list):
            return [normalize(v) for v in item]
        if isinstance(item, float) or (type(item) is int and abs(item) > 9007199254740991):
            raise ValueError("wire number is not an exact safe integer")
        return item
    return json.dumps(normalize(value), ensure_ascii=False, separators=(",", ":"), allow_nan=False)


def semantic(domain, value):
    return digest((domain + "\n" + canonical(value)).encode())


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def read_json(path, limit=MAX_DOCUMENT):
    data = Path(path).read_bytes()
    if len(data) > limit:
        raise ValueError("document exceeds byte limit")
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    value = json.loads(data, object_pairs_hook=pairs,
                       parse_constant=lambda _: (_ for _ in ()).throw(ValueError("non-finite JSON")))
    canonical(value)
    return value


def confined(root, name):
    relative = PurePosixPath(name)
    if not isinstance(name, str) or not name or not name.isascii() or "\\" in name or "\0" in name:
        raise ValueError("invalid logical path")
    if relative.is_absolute() or str(relative) != name or any(p in ("", ".", "..") for p in relative.parts):
        raise ValueError("noncanonical logical path")
    path = Path(root).joinpath(*relative.parts)
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError("symbolic path is forbidden")
        if part == Path(root):
            break
    if not path.is_file():
        raise ValueError("referenced input is not a regular file")
    return path


def reference(root, name, schema=None):
    path = confined(root, name)
    result = {"path": name, "sha256": digest(path.read_bytes()), "size": path.stat().st_size}
    if schema:
        result["schema"] = schema
    return result


def verify(root, references):
    for expected in references:
        actual = reference(root, expected["path"])
        if any(expected[k] != actual[k] for k in ("sha256", "size")):
            raise ValueError("input byte identity differs: " + expected["path"])


def source_digest(references):
    files = [{k: f[k] for k in ("path", "sha256", "size")} for f in references]
    files.sort(key=lambda f: f["path"])
    if len({f["path"] for f in files}) != len(files):
        raise ValueError("duplicate source path")
    return semantic("madb.source/1", files)


def check_tool(tool):
    path = Path(tool["path"])
    if not path.is_absolute() or not path.is_file():
        raise ValueError("tool requires an installed absolute regular image")
    if digest(path.read_bytes()) != tool["sha256"] or path.stat().st_size != tool["size"]:
        raise ValueError("tool image identity differs")
    return path


def command(tool, arguments, work, *, stdin=None, maximum=8 * MAX_DOCUMENT):
    image = check_tool(tool)
    # No new session/process group: the outer Host retains cancellation and
    # descendant cleanup. Environment is explicit; HOME is never overwritten.
    result = subprocess.run([str(image), *map(str, arguments)], cwd=work, input=stdin,
                            capture_output=True, timeout=120,
                            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "CI": "true"})
    check_tool(tool)
    if len(result.stdout) > maximum or len(result.stderr) > MAX_DOCUMENT:
        raise ValueError("tool output exceeds declared limit")
    if result.returncode:
        raise ValueError("tool failed: " + result.stderr.decode(errors="replace"))
    return result


def write_new(root, name, data):
    relative = PurePosixPath(name)
    if relative.is_absolute() or str(relative) != name or any(p in ("", ".", "..") for p in relative.parts):
        raise ValueError("invalid new output path")
    path = Path(root) / name
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        stream.write(data)
    return path
