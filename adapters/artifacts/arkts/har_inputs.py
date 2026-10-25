"""Private Package helper: bind immutable C04 file refs to real HAR members.

No compiler, discovery, host commit, candidate schema, or device operation lives
here. The caller supplies references from its host-checked Source/Build parents;
this helper never replaces an expected digest with freshly observed bytes.
"""

from pathlib import Path
import hashlib
import re
import stat

from adapters.artifacts.arkts.har import canonicalize, read_members
from adapters.toolchains.common.documents import sha_file


def _input(root: Path, reference: dict) -> Path:
    if not isinstance(reference, dict) or set(reference) - {"path", "sha256", "size", "schema"}:
        raise ValueError("expected an existing C04 FileReference")
    name = reference.get("path")
    if (
        not isinstance(name, str)
        or not name
        or "\\" in name
        or "\0" in name
        or Path(name).is_absolute()
        or any(p in ("", ".", "..") for p in name.split("/"))
    ):
        raise ValueError("invalid frozen package input path")
    if (
        not isinstance(reference.get("sha256"), str)
        or not re.fullmatch(r"sha256:[0-9a-f]{64}", reference["sha256"])
        or type(reference.get("size")) is not int
        or reference["size"] < 0
    ):
        raise ValueError("invalid frozen package input identity")
    current = root
    for part in name.split("/"):
        current = current / part
        if current.is_symlink():
            raise ValueError("package input contains a symlink")
    if not stat.S_ISREG(current.stat().st_mode):
        raise ValueError("package input must be a regular file")
    if current.stat().st_size != reference["size"] or sha_file(current) != reference["sha256"]:
        raise ValueError("frozen package input changed: " + name)
    return current


def bind_har(
    raw: bytes,
    input_root: Path,
    native: dict[str, dict],
    passthrough: dict[str, dict] | None = None,
) -> bytes:
    """Validate the native inventory and named unchanged files; normalize container.

    Mapping keys are archive member names, values are the ORIGINAL frozen C04
    file references. Extra compiler-emitted members remain inventoried by har.py;
    their semantics/declaration provenance are the Package recipe's obligation.
    Every file in the SDK native directory package/libs/, every .so member, and
    every ELF payload anywhere must belong to the explicit native inventory.
    Renamed ELF or versioned .so files cannot evade that closure; ordinary
    non-ELF resource files outside package/libs/ remain permitted.
    Repeated source refs are allowed when the recipe deliberately copies one
    declaration to multiple member names. Cross-map destination collisions are
    rejected; the caller's config decoder must reject duplicate JSON keys.
    """
    input_root = Path(input_root)
    if input_root.is_symlink() or not input_root.is_dir():
        raise ValueError("package input root must be an ordinary directory")
    input_root = input_root.resolve(strict=True)
    if (
        not isinstance(raw, bytes)
        or not isinstance(native, dict)
        or not native
        or passthrough is not None
        and not isinstance(passthrough, dict)
    ):
        raise ValueError("an explicit nonempty native HAR inventory is required")
    passthrough = {} if passthrough is None else passthrough
    if set(native) & set(passthrough):
        raise ValueError("duplicate HAR binding destination")
    if any(
        not isinstance(name, str) or not name.startswith("package/") or not name.endswith(".so")
        for name in native
    ):
        raise ValueError("native HAR bindings must name explicit .so members")
    if any(not isinstance(name, str) or name.endswith(".so") for name in passthrough):
        raise ValueError("native members cannot hide in passthrough bindings")
    expected = native | passthrough
    # Gate frozen-byte drift before examining/normalizing the candidate archive.
    for reference in expected.values():
        _input(input_root, reference)
    files = {member.path: member.data for member in read_members(raw) if member.data is not None}
    native_members = {
        name
        for name, data in files.items()
        if name.endswith(".so") or name.startswith("package/libs/") or data.startswith(b"\x7fELF")
    }
    if native_members != set(native):
        raise ValueError("HAR native library closure differs from frozen Build inputs")
    for name, reference in expected.items():
        data = files.get(name)
        if (
            data is None
            or len(data) != reference["size"]
            or "sha256:" + hashlib.sha256(data).hexdigest() != reference["sha256"]
        ):
            raise ValueError("HAR member differs from frozen input: " + name)
    normalized = canonicalize(raw)
    # Verify original references again; do not bless changed sources after work.
    for reference in expected.values():
        _input(input_root, reference)
    return normalized
