# Kotlin authored declaration fixtures

Each fixture contains exactly four authored inputs for the existing ordinary
`--author-fixture` entry: `BoundaryTypes.kt`, `OwnedExports.kt`, `Boundary.ets.in`,
and `runtime-policy.json`. The Kotlin files replace their corresponding source
inputs under `sdk/kotlin/arkts-owned/src/common/`; they are real KSP inputs.
No generated declarations are edited.

`rename-record` declares `Parcel` and uses it directly in exported signatures
and notification metadata. The internal `Snapshot` type alias keeps the fixed
business owner source valid; exported signatures do not use the alias because
stock akinterop KSP does not support it. The public surface uses `Parcel` and
`copyRecord` while retaining the `roundtrip` public role identifier.

The three negative fixtures alter only actual Kotlin callback declarations:
`suspend onReady` (line 17), `suspend onValue` (line 18), and return-valued
`onValue` (line 18). The expected compiler diagnostic prefix is `MADB_RECORD`:
`suspend/generic/extension export unsupported` or
`synchronous return-valued callback is unsupported by the nonblocking notification profile`.
Diagnostics carry the source file and line supplied by KSP; no column is invented.

These fixtures require independent public C04 generation validation. The producer
has additionally executed cold KSP for all four shapes; this is not target evidence.
