# Gobley source-patch attribution

The four `patches/*.patch` files contain modifications and context from the upstream projects below. These source patches and the covered modified files are provided under **MPL-2.0**, with the unchanged license text in [patches/LICENSE-MPL-2.0](patches/LICENSE-MPL-2.0). The repository's Apache-2.0 default does not replace this license.

| Upstream source | Actual attribution and license evidence | MadB patch files |
|---|---|---|
| Gobley `v0.3.7` | `crates/gobley-uniffi-bindgen/Cargo.toml`: authors `Gobley Contributors`, license `MPL-2.0`; archive root `LICENSE` | `gobley-0.3.7-once-completion.patch`, `gobley-0.3.7-native-abi.patch` |
| Mozilla UniFFI `uniffi_core 0.29.4` | Published crate `Cargo.toml`: license `MPL-2.0`; modified Rust files retain the MPL source-form notice | `uniffi_core-0.29.4-owned-result.patch`, `uniffi_core-0.29.4-native-ready.patch` |

The Gobley modifications affect the generator `crates/gobley-uniffi-bindgen/src/gen_kotlin_multiplatform/mod.rs` and eight templates under `crates/gobley-uniffi-bindgen/src/templates/`: `ffi/Async.kt`, `headers/RustBufferTemplate.h`, `macros.kt`, `native/Async.kt`, `native/CallbackInterfaceImpl.kt`, `native/HandleMap.kt`, `native/NamespaceLibraryTemplate.kt` and `native/RustBufferTemplate.kt`. The generator source retains its original MPL source-form notice. The UniFFI runtime modifications affect `src/ffi/foreignfuture.rs`, `src/ffi/rustfuture/mod.rs` and `src/ffi/rustfuture/future.rs`.

The inspected modified upstream files have no separately named copyright/year line. No such line is invented here. Existing notices in the upstream files remain intact. Gobley's archive README credits its fork of [UniFFI Kotlin Multiplatform bindings](https://gitlab.com/trixnity/uniffi-kotlin-multiplatform-bindings) and its use of [Mozilla UniFFI](https://github.com/mozilla/uniffi-rs).

MadB's modifications add bounded ownership/completion handling, Native ABI/version checks, exact unsigned C declarations for integer/handle/buffer fields and explicit Kotlin bit-carrier conversions; they are local controlled patches, not upstream releases or an upstream endorsement. [toolchain.json](toolchain.json) records the exact source URLs, archive hashes, patch hashes and generator dependency-lock hash. [The source inventory](../../../third_party/c06-source-attribution.json) records the reviewed provenance and license files.

The UniFFI crates.io archive includes its per-file MPL notices and package license declaration, but no complete LICENSE or separate NOTICE file. Its complete license was checked against the official fixed `v0.29.4` repository LICENSE; those bytes match the Gobley archive's MPL text. This source PR includes patches and license texts, not downloaded generator binaries or entire upstream source trees. Reconstruct the modified sources using the hash-verified archives and the patches in the committed recipe. Generated SDK and executable release distributions require the separate release notice/source-availability review.
