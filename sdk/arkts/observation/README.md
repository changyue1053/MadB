# ArkTS state observation

This optional L05 source extension adapts the existing C07 `Engine.observeState`
and `Subscription` under [ADR-0013](../../../docs/adr/0013-arkts-runtime-and-completion.md).
It adds no native owner, DTO conversion, timer, payload queue,
worker or reactive framework. The ordinary SDK does not import this extension.
It is an initial state/page lifecycle slice, not C07 or complete L05 acceptance.

## Source consumption

Copy `StateObservation.ets` beside the **generated** C07 `Boundary.ets` in the
consumer module. Import `StateObservation` directly for the no-UI API. Add
`ArkUIStateModel.ets` and import it separately only for ArkUI V2 consumers; there
is deliberately no barrel that loads ArkUI into the ordinary entry.

```text
consumer-module/src/main/ets/madb/
  Boundary.ets                 # Existing generated C07 facade
  RuntimeFrame.ets             # Existing C07 dependency
  StateObservation.ets         # Optional no-UI entry
  ArkUIStateModel.ets          # Optional ArkUI V2 entry
```

Keep the generated facade's native module dependency and identity unchanged.
These project-authored sources use the repository [Apache-2.0 license](../../../LICENSE);
retain its [NOTICE](../../../NOTICE) when redistributing them.
This slice does not yet automate inclusion in a HAR or declare an extension
registry profile. Package delivery and an independent installed candidate
consumer remain required before release.

The ArkUI entry uses `@ObservedV2` and `@Trace`, available from API 12 in the
[OpenHarmony state-management reference](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/ui/state-management/arkts-new-observedV2-and-trace.md).
That reference is not a HarmonyOS product SDK compatibility attestation. Compile
against the selected actual SDK and run the actual target. The no-UI controller
requires only the C07 ordinary API and Promise; it does not require ArkUI.

## Ownership and lifecycle

Constructors and getters do not register anything. One persistent presentation
owner holds one controller/model and its close attempt. Calls and deliveries must
stay in the Engine's owning env; the ArkUI model additionally requires the UI
main thread. These sources do not perform a thread hop or validate arbitrary
foreign env use. The application continues to own Engine and Runtime and their
final closes.

```typescript
const observation: StateObservation = new StateObservation();
observation.attach(engine, (snapshot: Snapshot) => {
  // Use the ordinary owned Snapshot synchronously.
});
observation.requestStop();
await observation.close();
```

| API | Contract |
|---|---|
| `attach(engine, listener)` | Exactly one state registration. A non-idle owner throws `StateObservationError` with code `Busy` before registration; a non-function listener gets `InvalidArgument`. No replay/default is invented. |
| `requestStop()` | First synchronously gates this receiver and releases its listener reference, then calls the existing subscription's stop request. It does not interrupt an already entered listener or confirm drain. Exceptions preserve their identity; the gate remains closed. |
| `close()` | Gates and requests stop synchronously. Calls the existing subscription's async close after registration can return. Concurrent/reentrant callers share one attempt. Ordinary failures reject the Promise. |
| `phase` | `idle`, `attaching`, `attached`, `stopping`, `closing`, or `closeFailed`; plain read-only lifecycle inspection. |
| `closeFailure` | Original failure from the last failed close attempt; reset at a new attempt. Plain read-only inspection, not an ArkUI observed field. |

`close()` success releases this adapter's subscription reference and allows a
new explicit `attach`. Failure retains that exact subscription and keeps new
registration closed. Another `close()` retries its existing close observation;
it never resubscribes. A permanent C07 failure can remain permanent on retry.
No error is reclassified as successful drain, and there is no automatic retry
loop. A stop-request error from `close()` rejects that attempt; a later explicit
close retries the request and observation.

A synchronous callback during `observeState` may request stop or close. The
controller gates immediately and applies the request when the Subscription is
returned. If registration throws without returning a Subscription, the ordinary
Engine retains its registration rollback responsibility. A pending reentrant
close rejects that original registration failure; no unknown handle is invented.
A synchronous listener exception gates further entry and is rethrown unchanged
to C07's listener boundary. The owner still calls `close()`; C07 owns its
CallbackFailure outcome.

Each registration is identified by its own attachment object. Late callbacks
from a stopped or previous attachment cannot update the receiver after a
restart. This is a local callback gate, not a replacement for native drain.
There is at most one retained Subscription and one close attempt per controller.
No per-value task or second capacity budget is introduced.

## ArkUI V2 model and page responsibility

```typescript
const model: ArkUIStateModel = new ArkUIStateModel();
model.attach(engine);
// An ArkUI V2 component reads model.snapshot.
model.requestStop();
await model.close();
```

The model has one `@Trace snapshot: Snapshot | undefined`. It assigns the same
owned snapshot delivered by the ordinary API; it does not introduce a business
state owner. Only snapshot replacement is observed; `phase` and `closeFailure`
are inspection getters and do not publish ArkUI notifications. Consumers should
read the snapshot rather than edit it in place or treat it as authoritative
business state. `@Trace` here is property observation, not HiTrace tracing.

The last confirmed snapshot remains after stop/close. A successful fresh attach
clears it to `undefined` until the first new delivery. A rejected attach preserves
it. Synchronous registration deliveries are held locally until registration
succeeds; then the latest real snapshot is published instead of being cleared.
If registration delivers and subsequently throws, those uncommitted values
never replace the previous snapshot. A stop/close requested during successful
registration does not erase its confirmed snapshot.

A synchronous page disappearance hook must call `requestStop()` immediately and
start/retain `close()` in its presentation owner. It must not rely on returning
a Promise from the lifecycle hook. Handle a possible synchronous stop error and
still retain a close attempt. Install a rejection handler immediately, retain
the original Promise for async joining, and report/retry its failure explicitly.
Keep the owner alive across disappearance until its cleanup finishes. A new
appearance may attach only after successful close; appearance during pending or
failed close gets `Busy`. Repeated build/render calls must not call attach.

This slice has no hidden page hook, destruction finalizer or background scope.
It cannot finish cleanup for a consumer that drops its owner/Promise, fix a
source that falsely confirms drain, or turn a Promise into cancellation. It does
not close a shared Engine or Runtime.

`observeEvents` is intentionally unsupported by this state model. Events retain
their ordinary C07 FIFO/admission contract and must be consumed through their
own explicit Subscription. Do not encode events as repeated snapshot changes or
assume synchronous listener return is a business acknowledgement.

## Checks and remaining acceptance

From the repository root, using its pinned Node version:

```sh
node tests/integration/l05_arkts_observation/check.cjs
```

The host check executes these actual source files after type erasure, with real
Promise scheduling and an independently controlled Engine. The ArkUI decorators
are explicitly erased for host tests, so they test projection/lifecycle behavior
only. See the [test scope](../../../tests/integration/l05_arkts_observation/README.md).

Formal ArkTS compilation is separate; the [API26 compiler probe](../../../tests/integration/l05_arkts_observation/SDK_README.md) builds independent ordinary/UI Release HARs and HAP consumers. Runtime ArkUI notifications, an actual
page appearing/disappearing repeatedly, device/env exit, real native drain,
Release obfuscation and installed HAR/HAP consumption are not proved by these
host tests or by compiler success. V1 compatibility, custom decorator generation,
worker transfer and event adapters are outside this slice.
