# Private ArkTS native protocol

C07 implements ADR-0011's shared C++17 ownership protocol here. This is an
internal SDK component, not a stable C ABI, complete Node-API gateway or certified
profile. It imports no MadB tool-host code and creates no hidden executor.
`CMakeLists.txt` builds the position-independent static library; its composition
owner supplies the worker, provider and delivery mechanisms.

The source is independently reviewed. Two separately authored C17 providers and
20/25-case host suites are kept under
[protocol checks](../../../tests/integration/c07_native_protocol/README.md).
The regular repository gate executes Release and ASan/UBSan. Formal OHOS compile
and actual device/runtime evidence are separate; host checks do not certify them.

## Resource and ownership contract

The ledger has 8 operation slots, 8 total state/event subscriptions, 8 outstanding event recipient tickets including entered callbacks, 4096 canonical UTF-8 payload bytes including staging, and 32 shared result/close waiter slots. State has one replaceable pending value and one entered value per subscriber, plus one bounded incoming transaction per selected subscriber. Text copies are independent and fully charged before allocation. The 4096 figure is text bytes, not total heap usage; fixed control arrays, provider object headers/capacity and bounded synchronous conversion scratch are separate.

`Payload` carries the sole paired-release responsibility and its permit. Provider release executes outside the mutex while its bytes/ticket remain charged. The permit retires only after release returns. Source→mailbox→entered→cleanup transitions move the same ticket; they never drop and reacquire it. Fanout reserves the full recipient set atomically; partial clone failure rolls back every new recipient and leaves old state unchanged. State replacement conservatively reserves full new copies while the old ones remain charged. Pending cleanup prevents unbounded overlapping replacements.

`Publication` explicitly holds source ownership before enqueue. Event source reservations carry monotonically increasing order; a later committed publication cannot overtake an earlier source lease. Abandoning that earlier lease releases the barrier. Zero recipients return a value-only success receipt without Core ownership, metadata allocation, or an admission record. The input byte span remains borrowed, valid and immutable throughout `allocate`; this prototype never retains an uncharged input scratch copy.

State `take_delivery` holds a delivery lease, leaving its payload in the replaceable pending slot. `invoke` atomically takes the latest value and claims callback entry. Event `take_delivery` moves its existing ticket. `stop` gates new entry and drops queued payloads, then waits for held source/delivery/entered owners before releasing callback context. Provider calls and user callbacks/destructors never run under the protocol mutex.

## Work, result and close ports

1. The external owner acquires the single `Executor` lease and drives one real worker. `take_work` allows only one Running Work. The protocol does not create a thread or hidden pool. `Executor.finish()` is an assertion by that owner that the actual worker has exited/joined; host tests of this lease are not target worker-exit proof.
2. `Work.input()` is an exclusive provider-owned borrow. The provider performs checked Int32 increment; the protocol does not know the object's layout or perform the business algorithm. `succeed` moves that object into Stored and wins only from Pending; `fail` claims DomainFailure. Cancellation cannot rewrite an existing success/failure. No input borrow survives success/failure/finish.
3. A terminal result is separate from work exit. Running work continues to hold its lease after cancellation or completion until `Work.finish`/destruction. Queue removal provides its own real removal ACK. Abandoned Running Work fails closed with InternalFailure, then releases its own input/work responsibility.
4. `ResultClaim` atomically reserves claimant+waiter. Registration failure must rollback both. An admitted reservation survives close and must commit or rollback. A committed claimant must eventually be `take_result`-consumed/discarded by the external delivery owner, including env exit; deleting a JS Promise is not that action.
5. `ResultDelivery.invoke` only lends `const void*` to a private callback through return. Its consumer copies/converts before return; paired native release and callback-context destruction then retire the waiter. No consumer may retain that raw pointer. A gateway that only receives ready notifications must hold this token until its separate tryTake/consume step, not invoke it early.
6. Close first requests cancellation/stop; full waiter admission may reject its observer but does not undo close. Unclaimed stored results are actually destroyed. Already-admitted result transfers keep the operation alive until consumed/discarded. Engine operational ACK additionally requires all active children, source/admission/work/payload owners and its Executor to drain.
7. Close observers do not wait for themselves. Their fixed records remain charged when ready and during callback/cleanup; only actual `invoke`/`discard` completion retires the record. Outcome is latched before resource-slot reuse. Already-closed observation invokes/releases its private callback immediately outside the lock and returns a value-only receipt. That is not a decision about JavaScript Promise scheduling or executor affinity.

The same 32-owner admission must be used when a gateway is added. A second independent 32-table or retiring native ownership at markDirty would violate the contract. The gateway's consume guard must finish the existing token and its actual reference cleanup. This prototype does not infer that an OHOS `napi_deferred` can be abandoned.

## Private handle, thread and lifetime preconditions

Numeric resource ids are per-Engine, monotonically increasing and never reused; exhausted id/order/observer spaces reject admission before wrap. Slot reuse does not let an old id affect a new resource. There is no unbounded native history map. A public SDK object must bind route/env/generation/owner and store its own immutable Closed tombstone/outcome; it must never reconstruct an old error by looking up a recycled private slot. Wrong-env/public-handle validation belongs to that later boundary layer.

Engine methods synchronize shared state. A move-only token may transfer between legal threads, but the same token must never be concurrently accessed or re-entered while one of its methods is active. A callback may reenter Engine methods such as stop/status; it may not recursively consume its own token. Provider and callback context, runtime, paired-release entrypoint and DSO must outlive every lease; legal provider/runtime entry threads are the assembling owner's obligation. Provider clone/destroy and callback destroy are noexcept. A provider must not mutate the snapshot byte length after reservation; the checked increment test changes only Int32 value and preserves text/capacity.

Engine destruction requests close but does not fake completion or join a thread. External Work, Publication, Delivery, ResultClaim/ResultDelivery and CloseWaiter owners must really retire. This slice contains no NAPI calls, env references, Promise executor, TSFN, async cleanup handle or domain worker implementation, and does not claim their target safety.

## Regression coverage

The behavior cases cover strict UTF-8/int32/NUL validation; cancellation before/after terminal and real concurrent claims; domain overflow; unclaimed-result close; already-admitted claimant completion/rollback; op/sub quotas; one worker; state latest replacement including pre-entry replacement; entered callback stop; acquired delivery gate; source-before-enqueue and out-of-order clone completion; atomic fanout and partial allocation failure; event item full including entered; independent byte full and conservative state staging; shared waiter full/ready/entered/rollback; close control under exhaustion; callback throw; callback/provider release reentry; old-id isolation and latched close error across slot reuse; and zero-recipient no-op ownership.
