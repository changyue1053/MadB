#!/usr/bin/env python3
"""Closed C07 gateway call-site coverage, not an AST or a platform-lifetime proof.

Scan the actual composed source. Optional compiler -E output proves those call sites
were expanded through the observer; include one preprocessed file per gateway TU.
Rust backend macro/import coverage is deliberately a separate evidence input.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

API = re.compile(r"\bnapi_[A-Za-z0-9_]+\b")
FUNCTION = re.compile(r"\b(napi_[A-Za-z0-9_]+)\s*\((?!\s*\*)")
MACRO = re.compile(r"^#define (napi_[A-Za-z0-9_]+)\(\.\.\.\)", re.M)


def code_only(text):
    """Blank comments and literals, preserving offsets/lines; reject exotic raw literals."""
    if re.search(r'\b(?:u8|u|U|L)?R"', text):
        raise ValueError("raw C++ literal needs explicit inventory review")
    pattern = r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\[\s\S]|[^"\\])*"|\'(?:\\[\s\S]|[^\'\\])*\''
    return re.sub(pattern, lambda m: re.sub(r"[^\n]", " ", m[0]), text)


def sites(path, text, wrappers, sdk_functions):
    code = code_only(text)
    if re.search(r"^\s*#\s*(?:define|undef)\s+(?:napi_\w+|MADB_NAPI_OBSERVER_NO_WRAPPERS)\b", code, re.M):
        raise ValueError(f"{path}: source may not disable/redefine the observer")
    result = []
    for token in API.finditer(code):
        name = token[0]
        tail = code[token.end():].lstrip()
        called = tail.startswith("(")
        if not called and name not in sdk_functions:
            continue  # napi_env/napi_ok/etc are types/constants, not API references.
        if name not in wrappers:
            raise ValueError(f"{path}: uncovered Node-API symbol {name}")
        if not called:
            raise ValueError(f"{path}: bare Node-API reference {name} bypasses function-like wrapper")
        result.append({"api": name, "line": code.count("\n", 0, token.start()) + 1})
    return result


def check_preprocessed(path, watched, wrappers):
    active = False
    seen = set()
    expanded = 0
    for line in path.read_text().splitlines():
        marker = re.match(r'^#\s+\d+\s+"([^"]+)"', line)
        if marker:
            current = str(Path(marker[1]).resolve())
            active = current in watched
            if active:
                seen.add(current)
            continue
        if not active:
            continue
        code = code_only(line)
        if FUNCTION.search(code):
            raise ValueError(f"{path}: raw Node-API call survived preprocessing: {line.strip()}")
        for token in API.finditer(code):
            if token[0] not in wrappers:
                continue
            before = code[:token.start()]
            if not before.endswith("::gateway::napi_observer::envCall(::"):
                raise ValueError(f"{path}: raw Node-API reference survived preprocessing: {token[0]}")
        expanded += code.count("::gateway::napi_observer::")
    if not seen or not expanded:
        raise ValueError(f"{path}: no instrumented gateway source found")
    return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "gateway_files": sorted(seen), "observer_expressions": expanded}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--observer-header", type=Path, default=Path(__file__).with_name("observer.hpp"))
    parser.add_argument("--napi-headers", type=Path, required=True)
    parser.add_argument("--preprocessed", type=Path, action="append", default=[])
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    wrappers = set(MACRO.findall(args.observer_header.read_text()))
    if len(wrappers) != 29:
        raise ValueError("observer's reviewed 29-symbol set changed")
    sdk_functions = set()
    headers = []
    for path in sorted(args.napi_headers.glob("*.h")):
        sdk_functions.update(FUNCTION.findall(code_only(path.read_text())))
        headers.append({"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    if not sdk_functions:
        raise ValueError("no actual Node-API header declarations")
    files = []
    watched = set()
    found = set()
    for directory in ("include", "src", "worker"):
        for path in sorted((args.source_root / directory).glob("*")):
            if path.suffix not in (".h", ".hpp", ".cpp", ".c", ".inc") or path.name in ("observer.hpp", "observer.cpp"):
                continue
            entries = sites(path, path.read_text(), wrappers, sdk_functions)
            watched.add(str(path.resolve()))
            found.update(entry["api"] for entry in entries)
            files.append({"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                          "calls": entries})
    if not files or not found:
        raise ValueError("no composed gateway call sites")
    if args.preprocessed:
        installed = args.source_root / "include/observer.hpp"
        if not installed.is_file() or installed.read_bytes() != args.observer_header.read_bytes():
            raise ValueError("composed observer header differs from checked wrapper source")
    preprocessed = [check_preprocessed(path, watched, wrappers) for path in args.preprocessed]
    result = {"schema": "madb.c07.napi-call-inventory.v1", "scope": "composed C/C++ gateway only; excludes Rust backend",
              "call_count": sum(len(file["calls"]) for file in files), "apis": sorted(found),
              "observer_header_sha256": hashlib.sha256(args.observer_header.read_bytes()).hexdigest(),
              "sdk_headers": headers, "sources": files, "preprocessed": preprocessed,
              "preprocessing_checked": bool(preprocessed)}
    encoded = json.dumps(result, indent=2) + "\n"
    if args.out:
        args.out.write_text(encoded)
    print(f"{result['call_count']} gateway call sites, {len(found)} API symbols; "
          f"{len(preprocessed)} preprocessed units checked")


if __name__ == "__main__":
    main()
