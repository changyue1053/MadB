# MadBObservation

An optional Swift source package that projects an existing ordinary subscription
into an Apple Observation model. It has no third-party, SwiftUI or native-backend
dependency. The base MadB SDK does not depend on this package.

The package requires Swift 6 and the Observation baseline: macOS 14, iOS 17,
tvOS 17 or watchOS 10. These are source availability requirements, not a claim
that every platform or native backend has been built or accepted. The initial
development slice targets macOS Arm64. UIKit lifecycle integration, other native
targets and formal release-package certification remain separate work.

## Ownership and use

Keep a stable model in your presentation owner. Its constructor and property
access have no native registration side effect:

```swift
import MadBObservation

@MainActor
func showState(sourceFactory: () throws -> MadBStateSource<Int>) async throws {
    let model = MadBStateModel<Int>()
    // nil means that this observation session has not received a real snapshot.
    try model.start(using: sourceFactory)
    // UI reads model.value, status, collectionFailure and stopFailure here.
    // When the owner ends this observation:
    try await model.stopAndWait()
}
```

`start(using:)` checks admission before invoking its synchronous MainActor factory
exactly once. The factory is the explicit subscription point. If it throws after
acquiring resources, it must release them. Repeated or reentrant starts reject
before calling the factory. A stop requested by the factory or a synchronous
Observation notification during start is retained: the acquired source is stopped
and drained without collecting. Stop-completion notifications keep admission
closed until their publication finishes. A successful restart clears the previous snapshot;
the package does not invent a default value or replay that the source lacks.

`requestStop()` requests the source's stop and cancels the collector. It does not
confirm drain. `stopAndWait()` is the owner's cleanup join: cancellation of the
calling task does **not** make it return early. It returns after that subscription
and its admitted deliveries drain, or throws the original drain error. The source
is retained after a drain error, new starts are rejected, and another
`stopAndWait()` retries drain without subscribing again. Concurrent callers join
one attempt. Collection errors and drain errors remain separately observable.

Natural completion, collection failure and cancellation all request stop and
perform the same drain. The cleanup task deliberately inherits neither collector
cancellation nor callback TaskLocals. At most one collector and one drain attempt
run for a session; there is no per-value task or additional payload queue. The
model is weakly captured by its task, so it does not form a retain cycle. Deinit
can only request stop; explicit `stopAndWait()` is the completion proof.

The last confirmed snapshot remains available after stopping. It is local
presentation state, not a second authority for business state and not a lossless
event history. Stopping a model never closes a shared business engine. Keep engine
ownership and final engine close in the application's existing owner.

## Source contract

`MadBStateSource<Value>` accepts the existing `collect`, `requestStop` and
`awaitStopped` operations as Sendable closures:

- Values must be owned Sendable snapshots. Wrapping a mutable foreign object does
  not establish that property.
- Collection must serialize and await each receiver call, and eventually end
  after stop/cancellation. The model directly awaits MainActor assignment.
- Stop requests must be thread-safe and idempotent. Drain confirms both the
  underlying subscription and admitted deliveries have stopped.
- A source must not synchronously wait for the MainActor. Its original buffer,
  error, cancellation and resource contracts remain in force.

The model cannot repair an implementation that never ends collection or falsely
reports drain. Such a source is not a conforming adapter.

## Optional SKIE integration

`Integration/SKIE/MadBSkieStateSource.swift` is distributed source, deliberately
outside the default package target. Compile it alongside the **installed public**
C06 `MadBSkieConcurrency.swift` and `MadBSkie.framework`, and link
`MadBObservation`. This keeps the Kotlin and Observation requirements out of each
other's default dependency graph.

```swift
let model = MadBStateModel<MadBValueSnapshot>()
try model.start { try makeMadBSkieStateSource(from: engineOwner) }
// Later, from the presentation owner's async cleanup:
try await model.stopAndWait()
// engineOwner remains usable; its owner closes it separately.
```

The adapter uses only public `subscribeState`, `collect`, `requestStop` and
`awaitStopped`. It preserves the C06 pending-state coalescing and lack of initial
replay. It copies no additional Kotlin object and adds no native state machine.
Terminal failure events become `MadBSkieStateFailure` with the original code.
It does not turn the event lane into UI state or add a second Flow bridge.

The integration is a new extension plus a fixed C06 public candidate during this
development slice. It does not modify that candidate's identity and does not
constitute C10 package certification. The UniFFI callback route needs its own
bounded actor delivery adapter before claiming equivalent support.

## Checks

Run `swift test -Xswiftc -warnings-as-errors` from this directory. Tests use
independently controlled collection/drain barriers and real Observation tracking
to check registration, notification, late delivery, cancellation, concurrent stop,
restart, errors, retry and owner release. A detached cleanup-context test checks
that callback TaskLocals do not escape into the drain attempt.

Repository integration checks separately consume a copied source package and the
selected installed public native candidate. They must retain the candidate and
extension identities and distinguish those checks from full platform acceptance.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
