"""Independent freestanding C fixture and native consumer for CRIT-10.

No production dependency/cache identity routine is used to calculate expected
outputs. The generated C is compiled and executed against a Python integer
oracle; checking only a cache key would not prove incremental behavior.
"""
from functools import lru_cache
from pathlib import Path
import platform
import shutil
import subprocess
import sys


SOURCE = b'''#include "entry.h"
#if __has_include("optional.h")
#include "optional.h"
#else
#define OPTIONAL_VALUE 0
#endif
#ifndef SCALE
#define SCALE 1
#endif
int read_value(void) { return BASE_VALUE * SCALE + OPTIONAL_VALUE; }
'''
ENTRY = b'#include "detail/value.h"\n'
BASE = 7


def find_clang():
    if sys.platform == "darwin":
        result = subprocess.run(["xcrun", "--find", "clang"], capture_output=True, text=True, timeout=10)
        if result.returncode:
            raise AssertionError("The locked Apple host needs xcrun clang: " + result.stderr)
        executable = Path(result.stdout.strip())
    else:
        found = shutil.which("clang")
        if not found:
            raise AssertionError("A real Clang compiler is required for CRIT-10; this is not a skipped pass")
        executable = Path(found)
    executable = executable.resolve(strict=True)
    if not executable.is_file():
        raise AssertionError(f"Clang is not a regular executable: {executable}")
    return executable


def host_target():
    architecture = platform.machine().lower()
    if sys.platform == "darwin":
        targets = {"arm64": "arm64-apple-macos11.0", "x86_64": "x86_64-apple-macos10.15"}
    else:
        targets = {"aarch64": "aarch64-unknown-linux-gnu", "x86_64": "x86_64-unknown-linux-gnu"}
    if architecture not in targets:
        raise AssertionError(f"The CRIT-10 host target is not supported: {sys.platform}/{architecture}")
    return targets[architecture]


@lru_cache(maxsize=1)
def consumer_platform_arguments():
    # Resolving the genuine image bypasses xcrun's automatic SDK selection.
    # The independently linked host consumer therefore declares its own sysroot.
    if sys.platform != "darwin":
        return []
    result = subprocess.run(["xcrun", "--sdk", "macosx", "--show-sdk-path"],
                            capture_output=True, text=True, timeout=10)
    if result.returncode:
        raise AssertionError("The real host consumer needs the macOS SDK: " + result.stderr)
    sdk = Path(result.stdout.strip())
    if not sdk.is_dir():
        raise AssertionError(f"The selected macOS SDK is missing: {sdk}")
    return ["-isysroot", str(sdk)]


def materialize_sources(root):
    (root / "source").mkdir(parents=True, exist_ok=True)
    (root / "include/detail").mkdir(parents=True, exist_ok=True)
    (root / "source/main.c").write_bytes(SOURCE)
    (root / "include/entry.h").write_bytes(ENTRY)
    set_base(root, BASE)


def set_base(root, value):
    (root / "include/detail/value.h").write_text(f"#define BASE_VALUE {value}\n")


def set_optional(root, value):
    (root / "include/optional.h").write_text(f"#define OPTIONAL_VALUE {value}\n")


def expected_value(base=BASE, scale=1, optional=0):
    return base * scale + optional


def consume_generated(test, clang, generated, expected, directory):
    """Compile the actual cold/hot output and run a separate expectation check."""
    directory.mkdir(parents=True, exist_ok=False)
    source = directory / "generated.c"
    source.write_bytes(generated)
    consumer = directory / "consumer.c"
    consumer.write_text(
        "int read_value(void);\n"
        f"int main(void) {{ return read_value() == {expected} ? 0 : 71; }}\n"
    )
    executable = directory / "consumer"
    compiled = subprocess.run([str(clang), *consumer_platform_arguments(), "-std=c11",
                               str(source), str(consumer), "-o", str(executable)],
                              capture_output=True, text=True, timeout=20)
    test.assertEqual(compiled.returncode, 0, (compiled.stdout, compiled.stderr, generated))
    consumed = subprocess.run([str(executable)], capture_output=True, timeout=5)
    test.assertEqual(consumed.returncode, 0, (consumed.stdout, consumed.stderr, generated, expected))
