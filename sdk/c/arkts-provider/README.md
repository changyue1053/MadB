# C17 ArkTS provider

The source is compiled as C17, using real `malloc`/`free` and C atomics. It does
not use a C++ compiler to simulate the C route. Expected allocation and domain
failures return explicit status values. Fault injection returns the same typed
status contract without inventing a C exception facility.

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
