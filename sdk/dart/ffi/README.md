# Dart synchronous runtime asset

`madb_runtime.dart` is the base production source asset in this directory. The
extension copies it to `lib/src/madb_runtime.dart`, as a part of the generated
`lib/madb_dart_ffi.dart`. It implements ADR-0026's limited macOS arm64 / Dart
3.13.7 contract. It is not a standalone Dart package or a second binding
generator. Generated facades, the ffigen adapter, and package pins belong to the
extension source/build pipeline.

`madb_value_runtime.dart` separately contains the optional bounded UTF-8 input
and output helpers. A value composition selects that asset as
`lib/src/madb_value_runtime.dart` and adds its part to the same generated library.
Resource-only compositions select only the base runtime. Neither composition
suppresses unused-element diagnostics or inserts artificial business calls.
The runtime test composition selects both assets to retain the UTF-8 positive
and negative cases; this does not claim a formal value package already exists.

The parent library imports `dart:convert`, `dart:ffi`, `dart:io`,
`dart:typed_data`, `package:crypto/crypto.dart`, and `package:ffi/ffi.dart as ffi`.
Its generated part supplies `_generatedCalls(DynamicLibrary)`. `_NativeCalls`
contains bootstrap and image-path inspection; resource modules additionally
implement `_EngineNativeCalls`. No native pointers escape these private records.
Generated adapters own their temporary buffers and free them in `finally`.

`_NativeLibrary.open` takes the generated install digest (bare SHA-256), the
original `sha256:` interface/profile digests, and the selected native symbol
count. `packageDirectory` denotes the package's dedicated native asset directory.
Its exact `install.json` keys are `bridge`, `native`, `identity`, and `files`.
The first three name distinct normalized relative files. `files` contains exactly
those three `{path, sha256, size}` records. Paths use lowercase ASCII components;
symlinks, unlisted content, case aliases and traversal are rejected. The identity
file preserves the public `madb.native-identity/1` object and original bytes.
Build pins the index bytes after the bridge exists; source generation cannot
invent that build-dependent digest.

Opening validates bytes, creates a fresh private copy, loads only its bridge,
checks every selected native function's actual image path, rechecks copied
bytes, then checks the public bootstrap identity. Opening creates no Engine.
`_EngineOwner.create` consumes the original checked-create result. Owners retain
the library and its `DynamicLibrary`, making the object graph unsendable on the
fixed Dart SDK. This is isolate ownership, not fixed OS-thread affinity.

Close enters `closing` before requesting native closure. Only successful
request, await and release advance to `closed`. Any phase exception retains the
owner and token in `closing`; a retry starts with request again. No automatic
Finalizer, registry, unload, worker or callback support is provided. A closed
owner's duplicate close is a native no-op. Invalid timeout input never invokes
native code. Generated methods check owner admission before invoking business
calls and use the runtime's checked numeric and strict UTF-8 helpers.

Private copied images remain on disk, including after failed loading. The host
may give the process a dedicated `TMPDIR` and remove that exact directory only
after process exit. Engine closure never deletes loaded images. Repeated opens
are checked independently; no mixed-version/concurrent-open guarantee is made.

## Focused validation

`test/check_runtime.py` accepts `--dart`, `--pub-cache`, `--candidate`, and
`--output`. The output must be a new directory under `.test-work/`. It verifies
the fixed Dart binary and the public C05/L02 candidate, copies dependencies to a
private offline cache, and captures commands and source/input hashes before and
after. The test bridge and handwritten ABI adapter are test-only composition;
they do not certify ffigen or the registered extension pipeline.

The test package runs analyze, JIT and AOT. Native tests exercise same-image
loading, real checksum values, normal close, retired handles, sequential open,
isolate-send rejection, and malformed package/image/identity cases. Separate
injected call-record tests prove Dart close failure/retry admission and typed
error/input/output checks. Injected Timeout/Busy records are not evidence that
the native engine itself experienced those failures. All original failures and
private process caches remain in the run directory for inspection.

The fixed dependency inputs are `crypto 3.0.8`, `ffi 2.2.0`, `typed_data 1.4.0`
and `collection 1.19.1`; the generated package's lock is retained in each result.
