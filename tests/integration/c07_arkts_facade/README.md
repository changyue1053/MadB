# C07 ordinary facade checks

Run the project-pinned Node version from the repository root:

```sh
node tests/integration/c07_arkts_facade/check.cjs
```

The 24 independent cases execute the actual SDK template after type erasure,
with controlled native API and timer substitutes. They cover request-only
close/reconnect, stop-only slot reuse, 32-full observer rejection with real
request ordering, delayed observation, callback-failure tombstones, strict
result claim and rollback, settling reentry, one terminal signal, timer failure,
error-class consistency and user getter identity/reentry. Delayed Engine join
ACK cases cover pending and immediate Core receipts, the same 32 JS owners,
settling reentry and observable conversion/close errors. The frame enum must
also match the frozen layout exactly. Failed-close regressions cover failure before
and during the existing parent close, same-Promise reuse, pending until physical
ACK, blocked business/listeners, original error identity, and unconfirmed
poll/receipt/timer failures. Receipts are written to a fresh temporary
directory rather than into the source tree.

These tests do not execute ArkTS, Node-API or a provider. In particular,
`physical=true` is an explicit test input, not evidence of physical cleanup.
The independent Release/obfuscated HAR/HAP consumer must establish real
allocation/free, native join, TSFN/ref/hook cleanup and actual target behavior.
The original regression oracle and failures are preserved in the C07 review
evidence; no callback expectation was weakened during repository integration.
