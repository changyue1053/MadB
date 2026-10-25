# C07 actual language providers

From the repository root, with Clang C/C++ and the pinned Rust toolchain:

```sh
.venv/bin/python -B tests/integration/c07_native_providers/check.py
```

The check builds C as C17, C++ as C++17 and Rust as an independent staticlib
from `sdk/*/arkts-provider`. Cargo runs offline with the checked-in lockfile;
format and Clippy are checked before the Release archive is linked. Its native
link dependency list comes from this actual Rust compiler. There are no
prebuilt provider inputs or absolute author-workspace paths.

`provider_tests.cpp` exercises all three factories: C/Rust layout agreement,
copy isolation, UTF-8/NUL/empty/invalid input, overflow, busy context destruction,
failure before and after allocation, exception/panic containment, actual work
failure and paired cleanup. Four simultaneous callers perform 100 independent
allocations each. The same implementations also execute typed Core operation
and stream paths; each language's final ledger has 409 allocations and releases,
with zero live models. Expected values are checked, not derived from Core stats.

Release and ASan/UBSan configurations rebuild all C/C++ sources. Rust remains
an uninstrumented Release staticlib; no Rust sanitizer or LeakSanitizer coverage
is claimed. Source hashes, actual commands, native library list and logs are
written to the printed temporary directory. Missing tooling, compile failures,
timeouts or nonzero checks fail the gate. These checks do not prove target
napi-ohos registration, ArkTS runtime behavior or complete C07 acceptance.
