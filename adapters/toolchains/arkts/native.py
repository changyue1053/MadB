"""Compose ArkTS native source inputs; the C04 worker alone invokes compilers.

Ordinary and companion variants share one checked gateway/Core/worker baseline.
Companion patches are strict, complete-file derivations, never fuzzy source edits.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from pathlib import Path
import re

from .preflight import parse_json_metadata


@dataclass(frozen=True)
class FileIdentity:
    path: Path
    sha256: str
    size: int


@dataclass(frozen=True)
class NativeUnit:
    path: Path
    language: str


@dataclass(frozen=True)
class NativeComposition:
    units: tuple[NativeUnit, ...]
    include_dirs: tuple[Path, ...]
    native_declaration: Path
    facade_template: Path
    source_inputs: tuple[FileIdentity, ...]
    derived_files: tuple[FileIdentity, ...]
    provider_factory: str
    registrar: str


def _identity(path: Path, data: bytes) -> FileIdentity:
    return FileIdentity(path, hashlib.sha256(data).hexdigest(), len(data))


def source_paths(repository: Path, *, route: str, variant: str) -> tuple[Path, ...]:
    """Return the explicit source allowlist for freezing before helper execution."""
    if route not in {"c", "cpp", "rust"} or variant not in {"ordinary", "test"}:
        raise ValueError("unsupported ArkTS native route/variant")
    gateway = repository / "sdk/arkts/native-gateway"
    runtime = repository / "sdk/arkts/native-runtime"
    facade = repository / "sdk/arkts/facade"
    paths = [gateway / "include" / name for name in ("codec.hpp", "handle.hpp", "runtime.hpp")]
    paths += [gateway / "src" / name for name in ("api.cpp", "runtime.cpp")]
    paths += [gateway / "Index.d.ts", facade / "Boundary.ets.in", facade / "RuntimeFrame.ets",
              facade / "frame-layout.json"]
    paths += [runtime / "include" / name for name in
              ("protocol.hpp", "worker.hpp", "provider_abi.h", "provider_port.hpp", "validate_snapshot.h")]
    paths += [runtime / "src/protocol.cpp", runtime / "src/worker.cpp"]
    provider = {"c": "sdk/c/arkts-provider/src/provider.c",
                "cpp": "sdk/cpp/arkts-provider/src/provider.cpp",
                "rust": "sdk/rust/arkts-provider/src/lib.rs"}[route]
    paths += [repository / provider, repository / "adapters/toolchains/arkts/native.py",
              repository / "adapters/toolchains/arkts/preflight.py"]
    if variant == "test":
        support = repository / "sdk/arkts/test-support"
        paths += [support / "native" / name for name in
                  ("companion.patch", "baseline.json", "support.hpp", "support_state.hpp", "support.cpp",
                   "observer.hpp", "observer.cpp", "observer_inventory.py")]
        paths += [support / "facade" / name for name in
                  ("companion.patch", "baseline.json", "TestFrame.ets", "FacadeFrame.ets")]
        paths += [support / "contracts" / name for name in
                  ("support.d.ts", "test-frame-layout.json", "facade-frame-layout.json",
                   "diagnostic-units-v1.json", "PORTS.md", "bootstrap-totals-layout.json",
                   "ports-layout.json", "observer-layout.json")]
    return tuple(paths)


def _apply_exact_patch(base: dict[str, str], patch: str) -> dict[str, str]:
    """Apply only the versioned companion's context hunks at exact line offsets.

    No path creation, renames, binary/mode patches, fuzz or offset search. New
    companion files are separate allowlisted inputs, not patch side effects.
    """
    output = dict(base)
    lines = patch.splitlines(keepends=True)
    seen: set[str] = set()
    index = 0
    while index < len(lines):
        if not lines[index].startswith("--- a/") or index + 1 >= len(lines):
            raise ValueError("invalid companion patch header")
        name = lines[index][6:].rstrip("\n")
        if lines[index + 1] != "+++ b/" + name + "\n" or name not in base or name in seen:
            raise ValueError("unknown, renamed or repeated companion patch file")
        seen.add(name)
        index += 2
        original = base[name].splitlines(keepends=True)
        result: list[str] = []
        cursor = 0
        hunks = 0
        while index < len(lines) and not lines[index].startswith("--- a/"):
            match = re.fullmatch(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n", lines[index])
            if match is None:
                raise ValueError("invalid companion hunk")
            old_start, old_count, new_start, new_count = (
                int(match[1]), int(match[2] or 1), int(match[3]), int(match[4] or 1))
            start = old_start - 1 if old_count else old_start
            if start < cursor or start > len(original):
                raise ValueError("overlapping or out-of-range companion hunk")
            result.extend(original[cursor:start])
            cursor = start
            if len(result) != (new_start - 1 if new_count else new_start):
                raise ValueError("companion new-line position mismatch")
            index += 1
            removed = added = 0
            while index < len(lines) and not lines[index].startswith(("@@ ", "--- a/")):
                line = lines[index]
                if not line or line[0] not in " +-":
                    raise ValueError("unsupported companion hunk content")
                marker, value = line[0], line[1:]
                if marker in " -":
                    if cursor >= len(original) or original[cursor] != value:
                        raise ValueError("companion base context changed")
                    cursor += 1
                    removed += 1
                if marker in " +":
                    result.append(value)
                    added += 1
                index += 1
            if (removed, added) != (old_count, new_count):
                raise ValueError("companion hunk count mismatch")
            hunks += 1
        if not hunks:
            raise ValueError("empty companion file patch")
        result.extend(original[cursor:])
        output[name] = "".join(result)
    return output


def _derive(base: dict[str, str], patch: str, baseline: object) -> dict[str, str]:
    if not isinstance(baseline, dict) or set(baseline) != {"base_sha256", "derived_sha256"}:
        raise ValueError("invalid companion baseline")
    def hashes(files: dict[str, str]) -> dict[str, str]:
        return {name: hashlib.sha256(text.encode()).hexdigest() for name, text in files.items()}
    if hashes(base) != baseline["base_sha256"]:
        raise ValueError("companion baseline changed")
    output = _apply_exact_patch(base, patch)
    if hashes(output) != baseline["derived_sha256"]:
        raise ValueError("companion derived content mismatch")
    return output


def compose_native(repository: Path, output: Path, *, route: str, variant: str) -> NativeComposition:
    """Materialize one source variant into a fresh workspace; never compile.

    The caller freezes source_paths before importing/running this helper. The
    generated identity header must declare GatewayProviderCreate (the selected
    provider factory), GatewayModuleName and existing identity constants. Rust's
    genuine frontend adapter replaces only the two marked callback/registrar
    blocks in the composed api.cpp and supplies its own sole registration entry.
    """
    paths = source_paths(repository, route=route, variant=variant)
    if output.exists() or output.is_symlink():
        raise ValueError("native composition output already exists")
    data = {path: path.read_bytes() for path in paths}
    inputs = tuple(_identity(path, data[path]) for path in paths)
    gateway = repository / "sdk/arkts/native-gateway"
    runtime = repository / "sdk/arkts/native-runtime"
    facade = repository / "sdk/arkts/facade"
    material = {"include/" + name: data[gateway / "include" / name].decode()
                for name in ("codec.hpp", "handle.hpp", "runtime.hpp")}
    material.update({"src/" + name: data[gateway / "src" / name].decode()
                     for name in ("api.cpp", "runtime.cpp")})
    material["worker/worker.cpp"] = data[runtime / "src/worker.cpp"].decode()
    declaration = data[gateway / "Index.d.ts"].decode()
    facade_text = data[facade / "Boundary.ets.in"].decode()
    if variant == "test":
        support = repository / "sdk/arkts/test-support"
        material = _derive(material, data[support / "native/companion.patch"].decode(),
                           parse_json_metadata(data[support / "native/baseline.json"]))
        facade_text = _derive({"Boundary.ets.in": facade_text},
                              data[support / "facade/companion.patch"].decode(),
                              parse_json_metadata(data[support / "facade/baseline.json"]))["Boundary.ets.in"]
        for name, destination in (("support.hpp", "include/support.hpp"),
                                  ("support_state.hpp", "include/support_state.hpp"),
                                  ("support.cpp", "src/support.cpp"),
                                  ("observer.hpp", "include/observer.hpp"),
                                  ("observer.cpp", "src/observer.cpp")):
            material[destination] = data[support / "native" / name].decode()
        support_declaration = data[support / "contracts/support.d.ts"].decode()
        match = re.fullmatch(r"/\*\*[^\n]*\*/\nexport interface NativeSupport \{\n(.*)\}\n",
                             support_declaration, re.S)
        ending = "};\nexport default gateway;\n"
        if match is None or declaration.count(ending) != 1:
            raise ValueError("native declaration composition boundary changed")
        declaration = declaration.replace(ending, match[1] + ending)
    material["Index.d.ts"] = declaration
    material["Boundary.ets.in"] = facade_text
    # Check the complete inputs again before any output writes. C04 also checks
    # the original frozen references on each prepare/replay copy.
    if any(path.read_bytes() != content for path, content in data.items()):
        raise ValueError("native composition source changed during derivation")
    output.mkdir(parents=True, exist_ok=False)
    derived = []
    for logical, text in sorted(material.items()):
        path = output / logical
        path.parent.mkdir(parents=True, exist_ok=True)
        encoded = text.encode()
        with path.open("xb") as stream:
            stream.write(encoded)
        derived.append(_identity(path, encoded))
    units = [NativeUnit(runtime / "src/protocol.cpp", "c++"),
             NativeUnit(output / "worker/worker.cpp", "c++"),
             NativeUnit(output / "src/runtime.cpp", "c++"),
             NativeUnit(output / "src/api.cpp", "c++")]
    if variant == "test":
        units.extend((NativeUnit(output / "src/support.cpp", "c++"),
                      NativeUnit(output / "src/observer.cpp", "c++")))
    provider = {"c": ("sdk/c/arkts-provider/src/provider.c", "c"),
                "cpp": ("sdk/cpp/arkts-provider/src/provider.cpp", "c++"),
                "rust": ("sdk/rust/arkts-provider/src/lib.rs", "rust")}[route]
    units.append(NativeUnit(repository / provider[0], provider[1]))
    return NativeComposition(tuple(units), (output / "include", runtime / "include"),
                             output / "Index.d.ts", output / "Boundary.ets.in", inputs,
                             tuple(derived), "mp_" + route + "_create",
                             "rust-macro" if route == "rust" else "napi-constructor")
