# Rust ArkTS frontend

This standalone SDK crate provides the real napi-ohos registrar and borrowed
`Unknown`/raw-return entry points. The single source declares the ordinary 16
methods and conditionally enables 16 test methods with `test-support`. The real
business provider remains in `../arkts-provider`; no business model is copied
into this crate. Shared C++ protocol/env source remains in the ArkTS SDK.

`MADB_BUILD_PHASE=metadata` evaluates the actual macros using a fresh Cargo target
without a Native SDK, target objects or linker directives. The tool compares the
actual emitted names/arity with the selected typed gateway declaration. Raw
`unknown` metadata is not the public typed facade authority.

`MADB_BUILD_PHASE=native` requires the actual C04 archive in
`MADB_NATIVE_ARCHIVE`. The compile task creates C++ objects/archive and checks
Rust types. The link task uses ohrs to perform Rust code generation and DSO
linking; it is not described as a pure linker action or reuse of a previously
compiled Rust object. Both tasks bind actual sources, tools, dependency closure
and diagnostics. There is no target run in this producer phase.

napi-ohos 1.2.0 explicitly enables only napi3 (resolved napi1/2/3); the independent
sys crate enables napi8 for the C++ runtime. Enabling frontend napi4 would create
an implicit global CustomGC TSFN outside the runtime ledger and is rejected.
The generic upstream class loop may exist, but this crate exports no classes,
Buffer, UnknownRef, JsDeferred or Tokio work. Borrowed values never escape their
NAPI call frame; retained callbacks use the shared runtime's actual napi_ref.

The raw C seam catches C++ exceptions. Raw return conversion preserves nullptr
with an existing pending native exception. Release uses unwind to contain
ordinary Rust panics at the exported boundary; abort/OOM is not claimed catchable.

The companion has 32 actual native exports. Its generated C++ raw entry and
C bridge retain the same Entry observer scope, including validation failures
before dispatch. Nested scopes establish callback authority; they are not
counted as two native API calls. The Rust macro prelude has separate fixed-version
source/expansion/ELF evidence and is not included in the gateway dynamic counters.

The existing C04 preparation freezes the exact Cargo source/lock, original
registry archives, all dependency notices, and the installed Rust standard
library copyright/license originals. Macro output remains a raw authority;
actual ArkTS declarations and policy own public types. No downloaded binary or
old target cache substitutes for compilation.

The ohrs 1.5.0 installer verifies original sources against its published lock and
builds offline. Its Windows-only locked dependency contains both `README.md` and
`readme.md`: on macOS use a dedicated case-sensitive filesystem and fresh Cargo
`registry/src` there (copying the archive cache/index is sufficient). Preserve the
case-sensitive source/vendor tree for verification. Never fold names or weaken
archive verification. Producer execution only uses the verified tool image;
installation and dependency acquisition remain preparation actions.
