# C06 checked managed declarations

This adapter connects the four explicit SampleEngine profiles to the existing C04 ports. It does not accept arbitrary Kotlin, Swift or Rust projects, and it does not assign safety policies merely because a signature can be parsed. Installation and consumer steps are in the [managed-route guide](../../../docs/examples/managed-routes/README.md).

| Responsibility | Source / implementation |
|---|---|
| Rust interface structure | Actual UniFFI `ComponentInterface`, using a source-verified 0.32.2 or 0.29.4 helper and the frozen author UDL |
| Kotlin-to-Swift structure | Clang AST and Swift SymbolGraph from the actual distributable Kotlin/Native + SKIE framework |
| Supported logical surface | Reviewed `semantic-surface.json`: complete types, method owners, parameter/result/error shapes and async modes |
| Runtime semantics | Source-owned runtime protocol, explicit call policies and shared rule validation; no inference of business thread safety or cancellation effects |
| Source identity and output transaction | Frozen C04 inputs, shared Rust finalizer, exact OutputSet and host commit logic |
| SDK bindings and conveniences | Pinned upstream generators, their source-identified patches, thin safe factories and the Swift resource facade |

`lowering.py` converts the parsed type graph; `skie_lowering.py` implements the closed Swift projection. `driver.py` validates the observation before and after work, invokes the actual tools and returns only declared outputs. The same descriptor is recomputed during generation and compared with the checked contract. Updating an implementation never automatically approves a new public declaration.

The supported structure includes bounded accepted value payloads, optional values, logical records and variants, directly managed objects, synchronous reverse callbacks and the declared async methods. The backend owns object representations: a UniFFI integer representation is not evidence of a safe MadB registry handle. Error channels participate in the reachable type projection. Swift's erased `any Error` remains a backend-owned `Swift.Error`; it is not rewritten into an invented exhaustive Kotlin error enum.

Defaults, borrowed UDL arguments, trait-interface objects, unsupported generics and async production callbacks are rejected. The Swift reader compares constructor/getter types, verifies actual sealed superclass relationships, and rejects unsupported attributes, ownership/isolation modifiers, default arguments and surrounding type syntax. An array of a supported scalar is not that scalar. `Phase`'s actual `@frozen` enum is the explicit supported attribute exception.

`Subscription.events` is a native Flow view whose owner is the subscription. The state/event factories and locked runtime protocol determine conflation, replay, capacity and stopping; one getter does not imply a universal stream policy. Raw runtime support APIs, duplicate Objective-C aliases and conformance-only hooks remain captured by the compiler snapshot, but are not silently added to the checked operational surface.

Swift dispatch identities are domain-prefixed hashes of the compiler USR, retained alongside the source overload identity. They are backend dispatch keys, not names of invented C linker symbols. Compiler/parser sources that do not retain declaration positions are reported at file level, never with fabricated line precision.

The SKIE pre-compilation receipt binds its actual framework/API to all author inputs, tool observation and profile. Formal compilation uses the same API and the final interface identity. The portable managed workspace identity is required because Kotlin debug compilation-unit paths otherwise vary with the output directory. Every build is cold and exclusive; no compiled cache establishes acceptance.

Upstream generation runs in a closed source context. UniFFI 0.32 receives an explicit crate plus `--metadata-no-deps` and the frozen SDK directory; it cannot discover an ambient author checkout. All helper HOME, temporary files and compiler descriptors are removed before returning. Output roots, parent paths, links, special files and unexpected artifacts are checked before files are copied into C04's work directory. A failure-only bounded diagnostic may remain in a rejected run, never in a successful candidate.

The [reader tests](../../../tests/integration/c06_reader/test_declarations.py) exercise closed-surface changes, erased errors, defaults, borrowed/trait declarations, actual compiler-shape regressions and file closure. They do not certify a native SDK. Real Release consumers and the final candidate-bound results belong to the independent MadB-Lab repository.
