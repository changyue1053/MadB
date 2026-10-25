# UniFFI 0.32.2 owned foreign-result cleanup

The stock runtime places raw `ForeignFutureResult` in its oneshot channel. A dropped receiver does not lift the result; `RustBuffer` and the manually dropped error buffer therefore leak. The controlled patch owns each raw result until the awaiting Rust future takes it. Abandoned results are lifted and dropped exactly once. The normal path preserves its original lift location. The external ABI is unchanged.

This profile permits only UniFFI-generated, thread-independent value/error/object lifting. Custom converters that require an actor or execution environment are not admitted. A foreign completion still must be invoked once after cancellation to release its sender; suppressing the callback is not a valid fix. This patch does not replace the generator's separate single-completion fix.

`owned-foreign-result-0.32.2.json` pins the original crates.io archive and patch. Apply the patch to an isolated, verified extraction, then record an inventory digest of the **whole** patched crate. Never modify the ambient Cargo registry. Production and P03 builds use that exact runtime source through an explicit Cargo path override. The archive's MPL-2.0 license and notice must accompany the source patch/provenance.

`../tests/owned_result.rs` is injected only into a separate test copy. It watches the actual allocation address for strings, nested strings, owned objects and declared errors, under cancel-before-completion, completion-before-cancel and ordinary completion. The original 0.32.2 source fails with zero releases; the patched source passes all 12 combinations with one release. Test injection is not part of the production patched-source inventory.

## Swift wait cancellation and checked initialization

`swift-cancellation-0.32.2.patch` connects throwing asynchronous declarations to UniFFI's existing native cancel functions. A per-future owner linearizes cancellation/completion, pins the native handle while cancellation runs, and defers its single free until the pin exits. Native callbacks never run under its lock. Pre-existing cancellation, synchronous wake/reentry, completion-first, late cancellation and concurrent cancel/free are covered by `../tests/swift_cancellation.swift` appended to the actual generated code. Nonthrowing declarations keep upstream behavior and cannot claim this cancellable profile.

`swift-checked-initialization-0.32.2.patch` exposes `madbCheckAbi()` and typed `MadbAbiMismatch` from the existing UniFFI initialization result. The ordinary MadB Swift `openEngine(expectedProfile:)` entry uses it before construction. It does not duplicate the ABI checker or remove the upstream guard.

## Swift 6 reverse asynchronous callbacks

`swift-foreign-sendable-0.32.2.patch` first lifts each owned callback argument into a generated Swift value (or a retained conversion error), then transfers `@Sendable` closures into the callback task. The asynchronous result is constrained to `Sendable`; raw FFI buffers are not declared Sendable. This admitted fixture requires Swift 6 and generated thread-independent conversions. `../tests/P03Swift.swift` uses real callback code and a Rust object's Drop counter to prove that a cancelled caller's late object is released once. `../tests/P03Jvm.kt` checks the actual generated completion helper when success/error notification throws, normal/typed-error callbacks and cancellation followed by a late allocated record; it separately joins the backend trampoline.

## Native future ready-result ownership

`native-ready-result-0.32.2.patch` fixes the opposite direction: upstream `poll()` lowers a completed Rust value into a raw FFI allocation, but `free()` drops that raw result without consuming its allocation. The patch retains an owned Rust value in a deferred lowering closure until `complete()` transfers it. Abandoning a ready result therefore runs ordinary Rust destruction; already encoded poll-time panic errors are explicitly freed. Lowering executes in the completion phase, under the existing panic/error conversion, and is restricted to generated thread-independent converters. The external FFI shape is unchanged.

`../tests/native_ready_result.rs` watches actual allocation addresses for ready strings, owned objects and panic error buffers. It runs in its own isolated upstream test copy (separate from the foreign-result allocation probe). Original source fails with zero releases; the patch releases exactly once for abandonment and normal completion. All upstream core tests also run against the patch.

## Reproducing the managed-toolchain regressions

`../run_regressions.py --tools <installed-tools.json> --output <fresh-directory>` accepts explicit `--kotlin-home`, `--java-home` and `--jvm-classpath`. It copies isolated test sources, proves original zero-release failures and patched exact-once release, runs the upstream unit/doc tests, builds the real test-support provider, generates both bindings and runs Swift 6/JVM P03 plus the Swift handle-owner races. `--runtime-only` skips the consumer builds. The JSON report binds commands, log digests, fixtures and tool receipt; it is development/toolchain evidence, not independent Lab candidate certification.

The installer receipt also records `metadata_inputrefs` for the complete copied SDK source closure. The staged dependency lock is verified against its recorded derived hash and the canonical original hash/size; preparation compares the original input references against frozen author bytes. Source, archive, build-source, image and obsolete-patch tampering are rejected. A changed helper or local module requires a new installation.
