# Fixed companion ports

Ordinary second-build source and identity are frozen separately. This test variant will have one registrar and a distinct DSO/build/profile identity, and contains the same ordinary API plus the twelve methods in support.d.ts. Methods use actual internal SDK handles obtained by the SDK's test-only facade bridge; consumers never inspect private object fields. Build receipts separately report actual compilation; this file does not claim a target result.

Work phase1 holds the actual worker after provider.run, before terminal claim; phase2 holds after actual succeed/fail and before Work.finish. arm requires no previously queued/running work and one unused gate, release is nonblocking/idempotent while closing. After its selected work resumes the gate disarms; later work cannot overwrite that gate observation. The engine thread never waits for JS. Hook entry on actual env exit releases test holds so the real cleanup join can run; normal close does not fake release.

Publication hold owns a real Core Publication reserve+allocate source lease. commit is exactly number0(abandon) or1(commit), no coercion. Native bytes/items remain in Core until real token action. There is one held publication per env, and no copied second payload or native waiter.

failClone nth is1..32 at the real typed provider clone port; mode1=the C provider's allocation failure,2=contained internal before allocation,4=test adapter post-clone InternalFailure with the actual successful provider allocation still owned by the returned result. Modes1/2 use the real C provider inject entrypoint; mode4 does not claim that provider itself returned an error-owned allocation. Core/roundtrip must paired-release that nonnull owned result. Actual clone/release counters remain the owner evidence.

failCompletion stage1=reserve real ResultClaim then rollback before commit (ResourceExhausted, retry allowed); stage2=throw before take guard (TakeBeforeGuardResidual); stage3=take the real Core token then discard through its paired cleanup and throw DeliveryFailureConsumed. JS construction/resolver-after-accepted faults belong to the independent test facade variant, not these native stages. At most one next completion fault is armed.

Partial-open arm is one occurrence after actual TSFN creation but before cleanup-hook registration. It triggers ordinary rollback/abort. lastAdmissionAudit records the real counters. A synchronous retry is refused while the old owner is not physical; if the runtime finalizes synchronously, the observed physical state controls the expectation. No artificial holder/finalizer pause is used.

Observe/inspect return fixed156 scalars: native54, test trace48, actual hook-entry native54 at offset102. AsyncCleanupEntered>0 marks the final segment present; otherwise it is explicitly unobserved. ExportKey creates a distinct observation-namespace key from a checked actual Runtime; inspect reads only the bounded static slot/history, never old env/context state. Normal receipt consumption retains diagnostics in that fixed history window. Core.stats is coherent under its Core mutex; support trace is copied under its separate mutex. The concatenation is NOT one atomic snapshot. Only real gate checkpoints and final physical ACK may be used as a joint stable state; moving observations support only justified partial orders/inequalities. Operation observation will identify actual Core winner and closed state, with no business snapshot return. No absent/stale/layout-mismatch field defaults to zero. Context-free history is fixed16 receipts, never an unbounded owner map.

The operation observation has exactly 10 fields as listed in test-frame-layout.json. Winner values are Pending=0, Success=1, DomainFailure=2, Cancelled=3, InternalFailure=4. Each native getter uses its real Core state; the pair of winner and stats is not a single multi-getter transaction. Observe at the supported work barrier for joint assertions.

lastAdmissionAudit returns the same 156-field layout. Before a triggered partial-open failure it throws NoAdmissionObservation (or WrongEnv if no matching env was armed). The injected failure is ResourceExhausted after actual create_threadsafe_function success. An immediate retry is RuntimeAlreadyOpen while the previous slot has not physically retired; after actual finalizer/owner release it may succeed. This is conditional on observed physical state, not a fixed timer delay.

releasePublication(commit=1) calls the real Publication.commit: if the engine is closing or any reserved recipient has stopped, the transaction returns Closing (Core status 3, surfaced as the same native error code), releases all reserved source payloads, and enqueues none. Successful commit increments SourceCommitted; rejected commit increments SourceAbandoned. commit=0 abandons the actual token, remains legal during Closing, and returns normally. Repeated release when no held token remains is a no-op. A physically retired engine handle remains invalid, never silently targets a new generation.

GateWorkInput records the actual work model value after the real provider increment, before the selected terminal gate; it is therefore input.value+1 on success, or the unchanged maximum value on DomainOverflow. Once the gate is consumed, later work leaves this observation unchanged.

ReadMode1 requires an actual same-env live Owner refresh. A cached residual with no live Owner and all historical entries that never reached physical ACK are mode0, even when queried from the original env. Only the actual stored physical predicate permits mode2/Phase3. An early failed reservation does not become closed merely because its slot was reused.

## Companion v2 additions

The ordinary SDK remains unchanged. Native methods append `testDeliveryGate`,
`testHandleAdmission`, and `testBootstrapTotals` at ordinals 28, 29 and 30. Their
public helpers accept actual SDK wrappers; no private handle is exposed. See
`ports-layout.json` for the eight-value delivery frame, exact commands and seven
admission cases. The gate holds the original Core Delivery, preserving its
pre-entry replacement and callback lifetime. Release schedules ordinary delivery;
actual env cleanup discards the owned lease. The handle fixture runs the ordinary
shared parser/admission helpers, with a real valid-env positive control. It does
not claim to execute the full submit operation.

`testBootstrapTotals()` is a static, read-only, 16-value DSO event observation.
`bootstrap-totals-layout.json` distinguishes attempted identity checks, matching
identity, actual reservations/contexts/engines/workers and successful Node-API
resource calls. Values are cumulative and never reset; no missing Runtime is
converted to synthetic zeros. A successful TSFN release call is not finalizer ACK.

Observation frames now have 178 values. The original 156-value prefix is intact;
`observer-layout.json` adds 12 runtime-local counters followed by 10 process-wide
observations. Physical stability covers the old prefix and local 12 only. Process
counters remain live, including diagnostic serialization calls, and are not an
atomic snapshot. History stores only the local segment. Every value is checked
against the JS safe-integer range; saturated counters fail instead of wrapping.

The closed-set observer records actual gateway attempts before forwarding and
successful resource events after real Node-API returns. It never grants
production permission, suppresses a call, or retains a Runtime. Its checked
source/preprocessor inventory excludes Rust backend calls; those need independent
macro/import evidence. Null-env cleanup is exercised only when its measured
entry count is positive, never inferred from an observed zero.
