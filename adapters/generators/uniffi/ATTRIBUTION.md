# UniFFI source-patch attribution

The `patches/*.patch` files contain modifications and context from Mozilla UniFFI **0.32.2**. These source patches and the covered modified files are provided under **MPL-2.0**, with the unchanged license text in [patches/LICENSE-MPL-2.0](patches/LICENSE-MPL-2.0). The repository's Apache-2.0 default does not replace this license.

| Published source crate | MadB patch files | Modified upstream files |
|---|---|---|
| `uniffi_core 0.32.2` | `owned-foreign-result-0.32.2.patch`, `native-ready-result-0.32.2.patch` | `src/ffi/foreignfuture.rs`, `src/ffi/rustfuture/mod.rs`, `src/ffi/rustfuture/future.rs` |
| `uniffi_bindgen 0.32.2` | `swift-cancellation-0.32.2.patch`, `swift-checked-initialization-0.32.2.patch`, `swift-foreign-sendable-0.32.2.patch` | Swift generator templates `Async.swift`, `macros.swift`, `wrapper.swift`, `CallbackInterfaceImpl.swift` under `src/bindings/swift/templates/` |

Both verified crate archives declare `license = "MPL-2.0"` and identify [Mozilla UniFFI](https://github.com/mozilla/uniffi-rs) as the repository. The modified Rust files carry the MPL source-form notice. The inspected modified files have no separately named copyright/year line; no such line is invented here. Existing upstream notices are retained.

Neither published crate archive includes a complete LICENSE or a separate NOTICE file. The complete license was retrieved from [the fixed upstream v0.32.2 LICENSE](https://github.com/mozilla/uniffi-rs/blob/v0.32.2/LICENSE) and verified byte-for-byte against the MPL text in the independently hash-verified Gobley v0.3.7 source archive. Its SHA-256 is `1f256ecad192880510e84ad60474eab7589218784b9a50bc7ceee34c2b91f1d5`.

MadB's patches add foreign/native result ownership cleanup, Swift cancellation, checked ABI initialization and Swift 6 callback transfer. These are local modifications, not an upstream release or endorsement. Each adjacent patch JSON records its exact original archive URL/SHA-256 and patch SHA-256. [The source inventory](../../../third_party/c06-source-attribution.json) records the complete patch-to-source and license mapping. The managed installer applies these patches to a verified, isolated source extraction; upstream source, modified source, helper author inputs and binaries are bound by its receipt.

This source PR includes the patch fragments and license texts. It does not redistribute downloaded generator binaries or entire upstream source trees. A generated SDK or executable release needs the separate applicable license/notice and modified-source availability bundle; this source attribution is not a release certification.
