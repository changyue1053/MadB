"""Private, read-only binding of one reviewed CLI ZIP to its installed tree.

The caller freezes this inventory as an ordinary input before later verification.
Local checksum agreement is not an official signature or C07 certification.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import unicodedata
import zipfile

from .preflight import inspect_command_line_tools, inspect_package, inspect_sdk


PREFIX = "command-line-tools"
MAX_ENTRIES = 200_000
MAX_MEMBER_SIZE = 1 << 30
MAX_TOTAL_SIZE = 12 << 30
EXCLUSIONS = ["top-level __MACOSX directories and non-executable AppleDouble sidecars",
              "regular non-executable files named exactly .DS_Store"]
LAUNCHERS = {
    "node": "tool/node/bin/node",
    "clang": "sdk/default/openharmony/native/llvm/bin/clang",
    "hdc": "sdk/default/openharmony/toolchains/hdc",
    "hvigor": "hvigor/bin/hvigorw.js",
    "ohpm": "ohpm/bin/ohpm",
}


def _hash_stream(stream):
    digest, size = hashlib.sha256(), 0
    while chunk := stream.read(1 << 20):
        digest.update(chunk)
        size += len(chunk)
    return digest.hexdigest(), size


def _digest(entries):
    return hashlib.sha256(json.dumps(entries, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _path(name):
    if (not name or name.startswith("/") or "\\" in name or ":" in name
            or any(ord(char) < 32 or ord(char) == 127 for char in name)
            or any(part in ("", ".", "..") for part in name.split("/"))):
        raise ValueError("unsafe member path: " + repr(name))
    return name


def _unique_name(name, seen):
    key = unicodedata.normalize("NFC", name).casefold()
    if key in seen:
        raise ValueError("duplicate or case/Unicode-colliding member: " + name)
    seen.add(key)


def _target(path, target, entries):
    if (not target or target.startswith("/") or "\\" in target or ":" in target
            or any(ord(char) < 32 or ord(char) == 127 for char in target)):
        raise ValueError("unsafe symlink target: " + path)
    parts = path.split("/")[:-1]
    for part in target.split("/"):
        # Even before '..', the OS must traverse the preceding component as a
        # directory. Lexically collapsing file/../name would hide ENOTDIR.
        current = "/".join(parts) or "."
        if entries.get(current, {}).get("kind") != "directory":
            raise ValueError("symlink target traverses missing/non-directory component: " + path + ": " + current)
        if part == "..":
            if not parts:
                raise ValueError("symlink escapes root: " + path)
            parts.pop()
        elif part not in ("", "."):
            parts.append(part)
    return "/".join(parts)


def _resolved(path, entries):
    seen = set()
    while path in entries and entries[path]["kind"] == "symlink":
        if path in seen or len(seen) >= 64:
            raise ValueError("cyclic or excessive symlink chain: " + path)
        seen.add(path)
        path = _target(path, entries[path]["target"], entries)
    if path not in entries or entries[path]["kind"] != "file":
        raise ValueError("symlink/launcher must end at an inventoried regular file: " + path)
    return path


def _closed_entries(rows):
    entries, seen = {}, set()
    if not isinstance(rows, list) or not rows or len(rows) > MAX_ENTRIES:
        raise ValueError("invalid inventory entry count")
    for row in rows:
        name = row["path"]
        if name != ".":
            _path(name)
        _unique_name(name, seen)
        kind = row["kind"]
        required = {"path", "kind", "mode"} if kind == "directory" else (
            {"path", "kind", "size", "sha256", "mode"} if kind == "file" else {"path", "kind", "target"})
        if kind not in ("directory", "file", "symlink") or set(row) != required:
            raise ValueError("invalid inventory entry: " + name)
        if kind != "symlink" and (type(row["mode"]) is not int or not 0 <= row["mode"] <= 0o777):
            raise ValueError("invalid inventory mode: " + name)
        if kind == "file" and (type(row["size"]) is not int or not 0 <= row["size"] <= MAX_MEMBER_SIZE
                               or re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) is None):
            raise ValueError("invalid inventory file identity: " + name)
        entries[name] = row
    if entries.get(".", {}).get("kind") != "directory":
        raise ValueError("archive must contain its explicit root directory")
    for name, row in entries.items():
        if name != ".":
            parent = str(PurePosixPath(name).parent)
            if entries.get(parent, {}).get("kind") != "directory":
                raise ValueError("missing or non-directory archive parent: " + name)
        if row["kind"] == "symlink":
            _resolved(name, entries)
    return entries


def acquire_inventory(archive: Path, *, expected_sha256: str, expected_size: int) -> dict:
    """Hash a caller-reviewed archive and stream every member; never extract it."""
    archive = Path(archive).absolute()
    if (not isinstance(expected_sha256, str) or re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
            or type(expected_size) is not int or expected_size <= 0):
        raise ValueError("explicit SHA256 and positive archive size are required")
    if archive.is_symlink() or not archive.is_file():
        raise ValueError("archive must be a regular file, not a symlink")
    with archive.open("rb") as stream:
        before = os.fstat(stream.fileno())
        if before.st_size != expected_size:
            raise ValueError("archive size differs from reviewed acquisition")
        digest, size = _hash_stream(stream)
        if digest != expected_sha256 or size != expected_size:
            raise ValueError("archive SHA256 differs from reviewed acquisition")
        stream.seek(0)
        rows, excluded, seen = [], [], set()
        with zipfile.ZipFile(stream) as zipped:
            members = zipped.infolist()
            if len(members) > MAX_ENTRIES or sum(item.file_size for item in members) > MAX_TOTAL_SIZE:
                raise ValueError("archive exceeds bounded inventory limits")
            for member in members:
                name = _path(member.orig_filename.removesuffix("/"))
                _unique_name(name, seen)
                mode = member.external_attr >> 16
                kind = stat.S_IFMT(mode)
                if (member.flag_bits & 1 or member.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
                        or member.file_size > MAX_MEMBER_SIZE or member.create_system != 3
                        or mode & 0o7000 or kind not in (stat.S_IFREG, stat.S_IFDIR, stat.S_IFLNK)
                        or member.is_dir() != (kind == stat.S_IFDIR)):
                    raise ValueError("unsupported ZIP member type, encoding or bounds: " + name)
                if kind == stat.S_IFDIR and member.file_size:
                    raise ValueError("directory member has data: " + name)
                if name == "__MACOSX" or name.startswith("__MACOSX/"):
                    if kind != stat.S_IFDIR:
                        leaf = PurePosixPath(name).name
                        counterpart = str(PurePosixPath(name.removeprefix("__MACOSX/")).with_name(leaf.removeprefix("._")))
                        if (kind != stat.S_IFREG or mode & 0o111 or not leaf.startswith("._")
                                or counterpart not in zipped.namelist()):
                            raise ValueError("unsupported Mac metadata exclusion: " + name)
                        with zipped.open(member) as source:
                            if source.read(8) != bytes.fromhex("0005160700020000"):
                                raise ValueError("invalid AppleDouble metadata: " + name)
                            _hash_stream(source)  # Also force decompression/CRC validation.
                    excluded.append(name)
                    continue
                if name != PREFIX and not name.startswith(PREFIX + "/"):
                    raise ValueError("unexpected archive root: " + name)
                relative = name.removeprefix(PREFIX + "/") if name != PREFIX else "."
                if PurePosixPath(relative).name == ".DS_Store":
                    if kind != stat.S_IFREG or mode & 0o111:
                        raise ValueError("unsupported .DS_Store exclusion: " + name)
                    with zipped.open(member) as source:
                        _hash_stream(source)
                    excluded.append(name)
                    continue
                row = {"path": relative, "kind": {stat.S_IFDIR: "directory", stat.S_IFREG: "file", stat.S_IFLNK: "symlink"}[kind]}
                if kind == stat.S_IFLNK:
                    if member.file_size > 4096:
                        raise ValueError("symlink target exceeds limit: " + name)
                    row["target"] = zipped.read(member).decode("utf-8", errors="strict")
                else:
                    row["mode"] = stat.S_IMODE(mode)
                    if kind == stat.S_IFREG:
                        with zipped.open(member) as source:
                            row["sha256"], row["size"] = _hash_stream(source)
                        if row["size"] != member.file_size:
                            raise ValueError("ZIP member size differs: " + name)
                rows.append(row)
        after = os.fstat(stream.fileno())
        if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns):
            raise ValueError("archive changed while inventorying")
    rows.sort(key=lambda row: row["path"])
    _closed_entries(rows)
    return {"kind": "arkts-local-acquisition-inventory", "archive": {
        "path": str(archive), "sha256": digest, "size": size,
        "checksum_source": "caller-reviewed expected checksum; locally computed from the user-supplied archive",
        "official_checksum_or_signature_verified": False},
        "archive_root": PREFIX, "exclusions": EXCLUSIONS.copy(), "excluded_members": sorted(excluded),
        "entries": rows, "entries_sha256": _digest(rows)}


def _installed_entries(root):
    rows, ignored = [], []
    total_size = 0
    pending = [(root, ".")]
    while pending:
        path, name = pending.pop()
        info = path.lstat()
        if name != ".":
            _path(name)
        if path.name == ".DS_Store":
            if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o111:
                raise ValueError("unsupported installed .DS_Store exclusion: " + name)
            ignored.append(name)
            continue
        row = {"path": name}
        if stat.S_ISLNK(info.st_mode):
            row.update(kind="symlink", target=os.readlink(path))
        elif stat.S_ISDIR(info.st_mode):
            row.update(kind="directory", mode=stat.S_IMODE(info.st_mode))
            with os.scandir(path) as children:
                for child in children:
                    relative = child.name if name == "." else name + "/" + child.name
                    pending.append((Path(child.path), relative))
        elif stat.S_ISREG(info.st_mode):
            total_size += info.st_size
            if info.st_size > MAX_MEMBER_SIZE or total_size > MAX_TOTAL_SIZE:
                raise ValueError("installed tree exceeds byte limits: " + name)
            with path.open("rb") as stream:
                digest, size = _hash_stream(stream)
            row.update(kind="file", mode=stat.S_IMODE(info.st_mode), sha256=digest, size=size)
        else:
            raise ValueError("unsupported installed entry type: " + name)
        rows.append(row)
        if len(rows) + len(pending) > MAX_ENTRIES:
            raise ValueError("installed tree exceeds entry limit")
    return sorted(rows, key=lambda row: row["path"]), sorted(ignored)


def _metadata(root):
    errors = []
    cli = inspect_command_line_tools(root, errors)
    sdk = inspect_sdk(root / "sdk/default", errors)
    packages = {}
    for name, manifest, launcher, expected in (
        ("@ohos/hvigor", "hvigor/hvigor/package.json", LAUNCHERS["hvigor"], cli.get("hvigor_version")),
        ("@ohos/hvigor-ohos-plugin", "hvigor/hvigor-ohos-plugin/package.json", "hvigor/hvigor-ohos-plugin/package.json", cli.get("hvigor_version")),
        ("@ohos/ohpm", "ohpm/package.json", LAUNCHERS["ohpm"], cli.get("ohpm_version")),
    ):
        observed = inspect_package(root / manifest, root / launcher, name, errors)
        packages[name] = observed
        if observed.get("version") != expected:
            errors.append({"code": "tool_component_mismatch", "component": name})
    for name, observed in {"sdk": sdk["product"], **sdk["components"]}.items():
        if any(observed.get(key) != value for key, value in cli.get("bundled_sdk", {}).items()):
            errors.append({"code": "tool_sdk_mismatch", "component": name})
    if errors:
        raise ValueError("installed metadata rejected: " + json.dumps(errors, sort_keys=True))
    return {"command_line_tools": cli, "sdk": sdk, "packages": packages}


def verify_inventory(inventory: dict, root: Path) -> dict:
    """Reject any tree/metadata change against a caller-frozen acquisition inventory.

    This executes no launcher. Load saved JSON with preflight.read_json_metadata
    and verify its frozen FileRef before calling; the inventory is not a signature.
    """
    if (inventory.get("kind") != "arkts-local-acquisition-inventory" or inventory.get("archive_root") != PREFIX
            or inventory.get("exclusions") != EXCLUSIONS
            or inventory.get("entries_sha256") != _digest(inventory["entries"])):
        raise ValueError("invalid or changed acquisition inventory")
    expected = _closed_entries(inventory["entries"])
    root = Path(root).absolute()
    if root.is_symlink() or not root.is_dir():
        raise ValueError("installation root must be a directory, not a symlink")
    root = root.resolve(strict=True)
    rows, ignored = _installed_entries(root)
    actual = {row["path"]: row for row in rows}
    differences = ["missing: " + name for name in sorted(expected.keys() - actual.keys())]
    differences += ["extra: " + name for name in sorted(actual.keys() - expected.keys())]
    differences += ["changed: " + name for name in sorted(expected.keys() & actual.keys()) if expected[name] != actual[name]]
    if differences:
        raise ValueError("installation tree differs from archive:\n" + "\n".join(differences))
    for name, entry in expected.items():
        if entry["kind"] == "symlink":
            resolved = _resolved(name, expected)
            if (root / name).resolve(strict=True) != root / resolved:
                raise ValueError("installed symlink differs from its closed identity: " + name)
    launchers = {}
    for name, relative in LAUNCHERS.items():
        resolved = _resolved(relative, expected)
        image = (root / relative).resolve(strict=True)
        if not image.is_relative_to(root) or image != root / resolved:
            raise ValueError("launcher resolves outside its closed identity: " + relative)
        if name != "hvigor" and not expected[resolved]["mode"] & 0o111:
            raise ValueError("launcher image is not executable: " + relative)
        launchers[name] = {"path": str(root / relative), "image_path": str(image),
                           "image_sha256": expected[resolved]["sha256"],
                           "link_target": expected[relative].get("target")}
    return {"root": str(root), "archive": inventory["archive"], "entries_sha256": inventory["entries_sha256"],
            "entry_count": len(rows), "ignored_installed_metadata": ignored,
            "launchers": launchers, "metadata": _metadata(root),
            "cpf": {"state": "incomplete", "reason": "separate reviewed compiler/LLVM/sysroot/runtime inputs required"},
            "certifies_c07": False}
