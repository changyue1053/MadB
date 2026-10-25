# ArkTS ordinary SDK facade

This is the restricted C07 Runtime/Engine/Operation/Subscription SDK template.
It is target runtime code, independent of the MadB tool host. Its lifetime rules
are fixed by [ADR-0013](../../../docs/adr/0013-arkts-runtime-and-completion.md).
It does not itself certify a route or supply a native library.

The generator replaces exactly one `@MADB_NATIVE_MODULE@` and one
`@MADB_EXPECTED_IDENTITY@` with escaped string literals derived from frozen build
inputs. It must parse the rendered exports using the selected actual compiler
and reject structural/policy drift. The state machine is reviewed handwritten
SDK code; it is not inferred from an opaque C callback or a TypeScript type.

`frame-layout.json` fixes the private diagnostic frame. `RuntimeFrame.ets` is
kept alongside it and checked for exact enum consistency. The native side uses
the same frozen layout. No handle or exact uint64 identity travels in that
numeric frame. It is not a public model or a second transport protocol.

The facade creates ordinary Promises, owns one bounded polling timer, keeps at
most 32 registering/settling observers, and consumes actual native ownership
through the private gateway. Runtime close is a separate memoized parent
control. Resource close requests occur before observer admission; request-only
close still receives the bounded internal lifecycle ACK. Immutable wrapper
outcomes preserve callback failure after native slot reuse.

A bridge failure rejects business waiters and blocks new business delivery, but
keeps the public Runtime close path available. The same memoized parent close
requests native shutdown and polls only cleanup progress; it does not retry
unknown pre-guard result ownership. After the actual physical ACK and receipt,
close rejects the original failure instead of reporting success. If the cleanup
poll, receipt or timer itself fails, close rejects that original failure and
retains an unconfirmed residual state; it never invents a Closed ACK. This follows
the existing [stop/drain and owner rules](../../../docs/architecture/05-lifecycle-and-concurrency.md)
and ADR-0013, with no new close API, native queue or owner.

Synchronous native errors and Promise errors use `BoundaryError.code`. Input
getters and listeners remain outside native error normalization. Property reads
do not start native operations. `result()` has one accepted claimant; cancelling
or dropping a Promise does not imply work exit or reversal of side effects.

The Node-based [focused check](../../../tests/integration/c07_arkts_facade/README.md)
uses an explicitly simulated native API and proves only facade bookkeeping.
Formal ArkTS compilation, real native DSOs, Release obfuscation, HAR/HAP
consumption, worker join, environment cleanup and candidate verification are
separate mandatory C07 checks.
