# Rust ArkTS provider

This is a standalone, dependency-free Cargo workspace. It cannot import MadB
tool-host crates. Build it with the repository's pinned Rust toolchain:

```sh
cargo build --manifest-path sdk/rust/arkts-provider/Cargo.toml --release --offline --locked
```

The provider owns genuine `Box<Model>` and `String` storage. Each exported C
boundary contains unwind panics, with `panic = "unwind"` in both profiles;
allocation-aware paths return `AllocationFailure`. Abort panics, allocator
abort/OOM and invalid foreign pointers are not caught or claimed recoverable.
`try_reserve_exact` and an explicit fallible allocation precede Box ownership.
A failure after allocation drops the real Box/String before retiring its ledger.

Host checks rebuild this archive offline from these sources; no author-created
static archive is accepted as test input. Clang ASan/UBSan instruments C/C++
and the C++ caller, not this stable Rust static library. This slice deliberately
does not contain the later napi-ohos module metadata/entrypoint integration.

This is C07's private assembly slice, not a released SDK or a certified
ArkTS route. The private C table is declared in
`../../arkts/native-runtime/include/provider_abi.h`. Models and contexts must
come from this exact provider, and the owner must drain all calls before context
and DSO destruction. `read_view` lends bytes only while that model remains alive
and unmodified; only the paired `release` consumes it.

The supported model contains an Int32 value and at most 1024 valid UTF-8 bytes,
including embedded NUL. `increment` checks overflow and leaves the text intact.
Core manages protocol ownership, while this language owns the domain algorithm
and model storage. Per-context counters and injected failures are private test
support, not a frozen public API.

Run `.venv/bin/python -B tests/integration/c07_native_providers/check.py` from the
repository root. It builds the three actual providers from repository source
and checks the same behavior through their separate factory functions. Host
results do not prove Node-API metadata, env cleanup or the target HAR.
