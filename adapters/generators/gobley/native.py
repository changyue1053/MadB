"""Compile Gobley native outputs with explicit compiler/runtime closure paths."""
from __future__ import annotations

import os
from pathlib import Path
import subprocess


def compile_native(generated: Path, compiler_root: Path, dependencies: Path,
                   rust_library: Path, output: Path, *, consumer: Path | None = None,
                   entry: str = "madb.boundary.main", embedded_static: bool = False) -> Path:
    output.mkdir(parents=True, exist_ok=True)
    common = sorted((generated / "commonMain").rglob("*.kt"))
    native = sorted((generated / "nativeMain").rglob("*.kt"))
    headers = list((generated / "nativeInterop/cinterop/headers").rglob("*.h"))
    if not common or not native or len(headers) != 1:
        raise ValueError("Gobley output lacks the complete common/native/header closure")
    header = headers[0]
    definition = output / "boundary.def"
    definition.write_text(f"headers = {header.name}\npackage = madb_boundary.cinterop\n")
    environment = dict(os.environ, JAVA_HOME=str(compiler_root / "jdk/Contents/Home"),
                       KONAN_DATA_DIR=str(compiler_root / "konan"))
    interop = output / "madb_boundary_interop"
    interop_command = [str(compiler_root / "kotlin-native/bin/cinterop"), "-def", str(definition),
                       "-compiler-option", f"-I{header.parent}", "-o", str(interop), "-target", "macos_arm64"]
    if embedded_static:
        interop_command.extend(["-staticLibrary", rust_library.name, "-libraryPath", str(rust_library.parent)])
    subprocess.run(interop_command, check=True, env=environment)
    command = [str(compiler_root / "kotlin-native/bin/konanc"), *map(str, common + native)]
    if consumer:
        command.extend([str(consumer), "-entry", entry])
    else:
        command.extend(["-produce", "library", "-module-name", "madb_boundary"])
    if consumer and not embedded_static:
        command.extend(["-linker-option", f"-L{rust_library.parent}", "-linker-option", "-lmadb_boundary_gobley",
                        "-linker-option", "-rpath", "-linker-option", str(rust_library.parent)])
    target = output / ("consumer" if consumer else "madb_boundary")
    command.extend(["-Xmulti-platform", "-Xexpect-actual-classes",
                    "-Xcommon-sources=" + ",".join(map(str, common)),
                    "-target", "macos_arm64", "-opt", "-o", str(target),
                    "-linker-option", "-liconv", "-Xoverride-konan-properties=minVersion.macos=14.0",
                    "-library", str(interop.with_suffix(".klib"))])
    for name in ["kotlinx-coroutines-core-macosarm64-1.11.0.klib", "atomicfu-macosarm64-0.32.1.klib",
                 "atomicfu-macosarm64-0.32.1-cinterop-interop.klib"]:
        command.extend(["-library", str(dependencies / name)])
    subprocess.run(command, check=True, env=environment)
    return target.with_suffix(".kexe" if consumer else ".klib")
