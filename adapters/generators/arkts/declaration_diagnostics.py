"""Read-only explanations for existing ArkTS declaration diagnostics.

These Python values are private helpers, not an explain CLI or wire format.
The caller supplies a captured source and the exact stderr of its checked reader
invocation; a digest match does not independently authenticate that invocation.
"""

from hashlib import sha256
from pathlib import Path, PurePosixPath
import json
import re
from typing import NamedTuple


class UnknownDiagnosticCode(ValueError):
    """The supplied stable code has no explanation in this helper."""


class UnsupportedReaderDiagnostic(ValueError):
    """Reader output is not one of the exact supported coded diagnostics."""


class SourceBindingError(ValueError):
    """The supplied source reference, bytes or location do not agree."""


class Explanation(NamedTuple):
    code: str
    message: str
    meaning: str
    trigger: str
    repair: str
    ordinary: str


def _bundled_explanations():
    # One fixed data source is embedded by the Rust CLI and included in the
    # actual generator helper input closure. Loading happens at module import;
    # lookup itself remains pure and never probes a source/compiler/workspace.
    catalog = json.loads((Path(__file__).resolve().parent.parent /
                          "diagnostic-explanations.json").read_bytes())
    entries = catalog["entries"]
    by_code = {entry["code"]: entry for entry in entries}
    if catalog["version"] != 1 or len(entries) != len(by_code):
        raise ValueError("invalid bundled diagnostic explanation catalog")
    result = {}
    for code in ("ARKTS_DECL_UNSUPPORTED_MODIFIERS", "ARKTS_DECL_CALLBACK_RESULT"):
        entry = by_code[code]
        result[code] = Explanation(
            code, entry["reader_message"], entry["meaning"], entry["triggers"][0],
            entry["repairs"][0]["instruction"], entry["ordinary"],
        )
    return result


_EXPLANATIONS = _bundled_explanations()


class SourcePoint(NamedTuple):
    path: str
    sha256: str
    size: int
    line: int
    column: int
    utf16_offset: int  # Reader text offset, after TextDecoder removes one BOM.
    byte_offset: int  # Original captured bytes, including any initial BOM.


class BoundDiagnostic(NamedTuple):
    explanation: Explanation
    source: SourcePoint


def explain(code):
    """Look up an existing code without filesystem, tool or runtime access."""
    if not isinstance(code, str) or code not in _EXPLANATIONS:
        raise UnknownDiagnosticCode("unknown declaration diagnostic code")
    return _EXPLANATIONS[code]


def bind_location(source_ref, source, location):
    """Bind an existing compiler point to captured UTF-8 bytes, without I/O.

    Columns and utf16_offset use the reader's decoded-text convention (one
    initial BOM removed). byte_offset addresses the original captured bytes.
    This only binds a point: stderr has no end range, symbol ID or template map.
    """
    if not isinstance(source_ref, dict) or not isinstance(location, dict):
        raise SourceBindingError("source reference and location must be mappings")
    path = source_ref.get("path")
    if (
        not isinstance(path, str)
        or not path
        or PurePosixPath(path).is_absolute()
        or any(part in ("", ".", "..") for part in path.split("/"))
        or "\\" in path
        or any(ord(c) < 32 or c in "\u2028\u2029" for c in path)
    ):
        raise SourceBindingError("source path must be a logical relative path")
    if (
        not isinstance(source, bytes)
        or len(source) > 1024 * 1024
        or type(source_ref.get("size")) is not int
        or source_ref["size"] != len(source)
        or source_ref.get("sha256") != "sha256:" + sha256(source).hexdigest()
    ):
        raise SourceBindingError("captured source differs from its reference")
    if location.get("path") != path:
        raise SourceBindingError("compiler location names a different source")
    line, column = location.get("line"), location.get("column")
    if type(line) is not int or type(column) is not int or line < 1 or column < 1:
        raise SourceBindingError("compiler location must be a positive line/column")
    try:
        # read_facade uses TextDecoder('utf-8', {fatal:true}), whose default
        # ignoreBOM=false consumes exactly one leading UTF-8 BOM.
        bom_bytes = 3 if source.startswith(b"\xef\xbb\xbf") else 0
        text = source[bom_bytes:].decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise SourceBindingError("source must be UTF-8") from error
    # Match the compiler's line breaks; str.splitlines also splits non-JS breaks.
    starts = [0, *(match.end() for match in re.finditer(r"\r\n|[\r\n\u2028\u2029]", text))]
    if line > len(starts):
        raise SourceBindingError("compiler line is outside the source")
    start = starts[line - 1]
    end_match = re.search(r"[\r\n\u2028\u2029]", text[start:])
    end = start + end_match.start() if end_match else len(text)
    remaining = column - 1
    offset = start
    while offset < end and remaining > 0:
        remaining -= 2 if ord(text[offset]) > 0xFFFF else 1
        offset += 1
    if remaining != 0:
        raise SourceBindingError("compiler column is outside a UTF-16 boundary")
    prefix = text[:offset]
    return SourcePoint(
        path, source_ref["sha256"], len(source), line, column,
        len(prefix.encode("utf-16-le")) // 2, bom_bytes + len(prefix.encode("utf-8")),
    )


def explain_reader_failure(stderr, source_ref, source):
    """Explain exact coded stderr; refuse unknown codes and uncoded failures."""
    if not isinstance(stderr, str) or len(stderr) > 12000:
        raise UnsupportedReaderDiagnostic("reader diagnostic is absent or over budget")
    # The production reader emits precisely one line and an optional final LF.
    line = stderr.removesuffix("\n")
    if not isinstance(source_ref, dict):
        raise SourceBindingError("source reference must be a mapping")
    path = source_ref.get("path")
    if not isinstance(path, str) or not line.startswith(path + ":"):
        raise SourceBindingError("reader diagnostic names a different source")
    match = re.fullmatch(r"([1-9][0-9]{0,8}):([1-9][0-9]{0,8}): ([A-Z][A-Z0-9_]*): ([^\r\n]+)", line[len(path) + 1:])
    if match is None:
        raise UnsupportedReaderDiagnostic("reader failure has no supported coded shape")
    info = explain(match[3])
    if match[4] != info.message:
        raise UnsupportedReaderDiagnostic("coded reader message differs from supported text")
    point = bind_location(source_ref, source, {
        "path": path, "line": int(match[1]), "column": int(match[2]),
    })
    return BoundDiagnostic(info, point)


def note_for_command_failure(error, source_ref, source):
    """Optional human note; never replace an unrecognized command failure."""
    if not isinstance(error, RuntimeError) or not str(error).startswith("node failed: "):
        return None
    try:
        bound = explain_reader_failure(str(error)[len("node failed: "):], source_ref, source)
    except (UnknownDiagnosticCode, UnsupportedReaderDiagnostic, SourceBindingError):
        return None
    info, point = bound
    return (
        f"L02 declaration hint [{info.code}] at {point.path}:{point.line}:{point.column} "
        f"(compiler-derived UTF-16 point; {point.sha256}): {info.meaning} "
        f"Trigger: {info.trigger} Repair: {info.repair} Ordinary entry: {info.ordinary}"
    )
