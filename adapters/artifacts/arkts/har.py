"""Private deterministic HAR container step; member contents remain unchanged."""
from __future__ import annotations

from dataclasses import dataclass
import gzip
import hashlib
import io
from pathlib import PurePosixPath
import tarfile
import zlib
import unicodedata

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_EXPANDED_BYTES = 256 * 1024 * 1024
MAX_MEMBER_BYTES = 64 * 1024 * 1024
MAX_MEMBERS = 4096


@dataclass(frozen=True)
class Member:
    path: str
    data: bytes | None
    mode: int


def _path(name: str) -> str:
    if not name or '\\' in name or any(unicodedata.category(c) == 'Cc' for c in name):
        raise ValueError('invalid HAR member path')
    parts = name.split('/')
    if any(part in ('', '.', '..') for part in parts) or parts[0] != 'package':
        raise ValueError('HAR members must be below package/')
    if len(name.encode('utf-8')) > 4096:
        raise ValueError('HAR member path exceeds budget')
    return name


def _expand(data: bytes) -> bytes:
    if len(data) > MAX_ARCHIVE_BYTES:
        raise ValueError('HAR archive exceeds budget')
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(data)) as archive:
            expanded = archive.read(MAX_EXPANDED_BYTES + 1)
    except (OSError, EOFError, zlib.error) as error:
        raise ValueError('invalid gzip HAR') from error
    if len(expanded) > MAX_EXPANDED_BYTES:
        raise ValueError('expanded HAR exceeds budget')
    return expanded


def read_members(data: bytes) -> tuple[Member, ...]:
    """Read a bounded regular-file/directory HAR without extracting it."""
    expanded = _expand(data)
    members: dict[str, Member] = {}
    portable_names: set[str] = set()
    try:
        with tarfile.open(fileobj=io.BytesIO(expanded), mode='r:') as archive:
            for item in archive:
                if len(members) >= MAX_MEMBERS:
                    raise ValueError('HAR member count exceeds budget')
                name = _path(item.name)
                if name in members:
                    raise ValueError('duplicate HAR member: ' + name)
                portable = unicodedata.normalize('NFC', name).casefold()
                if portable in portable_names:
                    raise ValueError('case or Unicode alias in HAR member: ' + name)
                portable_names.add(portable)
                if not (item.isfile() or item.isdir()) or item.linkname or item.sparse is not None:
                    raise ValueError('HAR supports regular files and directories only')
                if item.mode & ~0o777:
                    raise ValueError('special HAR permission bits are unsupported')
                if item.size < 0 or item.size > MAX_MEMBER_BYTES or (item.isdir() and item.size):
                    raise ValueError('HAR member size is invalid or exceeds budget')
                # Only container metadata emitted by the selected toolchain may
                # vary. Unknown PAX semantics cannot be silently discarded.
                if set(item.pax_headers) - {'path', 'mtime', 'atime', 'ctime', 'uid', 'gid', 'uname', 'gname'}:
                    raise ValueError('unsupported HAR PAX metadata')
                stream = archive.extractfile(item) if item.isfile() else None
                contents = stream.read() if stream is not None else None
                if contents is not None and len(contents) != item.size:
                    raise ValueError('truncated HAR member')
                members[name] = Member(name, contents, item.mode)
            if any(expanded[archive.offset:]):
                raise ValueError('HAR has trailing non-tar content')
    except (tarfile.TarError, RecursionError) as error:
        raise ValueError('invalid HAR tar container') from error
    # Hvigor 6.26.8 omits the package/ root entry. An explicit root, when
    # present, must still be a directory; other parent entries are required.
    if not members or ('package' in members and members['package'].data is not None):
        raise ValueError('HAR package root cannot be a file')
    for name in members:
        for parent in PurePosixPath(name).parents:
            if str(parent) in ('.', 'package'):
                break
            if str(parent) not in members or members[str(parent)].data is not None:
                raise ValueError('HAR parent directory is absent or a file')
    manifest = members.get('package/oh-package.json5')
    if manifest is None or manifest.data is None:
        raise ValueError('HAR manifest is absent')
    return tuple(members[name] for name in sorted(members))


def canonicalize(data: bytes) -> bytes:
    """Keep file bytes/modes; sort entries and normalize time/user metadata."""
    members = read_members(data)
    output = io.BytesIO()
    with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0, compresslevel=9) as compressed:
        with tarfile.open(fileobj=compressed, mode='w|', format=tarfile.PAX_FORMAT) as archive:
            for member in members:
                info = tarfile.TarInfo(member.path)
                info.mode = member.mode
                info.type = tarfile.DIRTYPE if member.data is None else tarfile.REGTYPE
                info.size = 0 if member.data is None else len(member.data)
                archive.addfile(info, None if member.data is None else io.BytesIO(member.data))
    result = output.getvalue()
    if len(result) > MAX_ARCHIVE_BYTES or read_members(result) != members:
        raise ValueError('HAR normalization changed members or exceeded budget')
    return result


def inventory(data: bytes) -> list[dict]:
    return [dict(path=m.path, size=len(m.data), sha256=hashlib.sha256(m.data).hexdigest(), mode=m.mode)
            for m in read_members(data) if m.data is not None]
