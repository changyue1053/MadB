# Public generation author fixtures

Each directory is an actual authored `Boundary.ets.in` plus its explicit
`runtime-policy.json`. These inputs are restricted to this profile's supported
SDK façade; they do not advertise arbitrary business-code export.

* `rename-record` renames Snapshot to Parcel throughout declarations and the
  implementation, and Engine.roundtrip to Engine.copyRecord (including the
  concrete façade method and Session implementation). The private native seam
  stays roundtrip. Its explicit public policy selects the renamed source but deliberately retains
  `madb.c07.Engine.roundtrip.v1` as the same capability ID. SourceKey, API name
  and shape still change. Successful
  C04 Source must contain the new actual declarations and ContractIR names.
* `reject-async` makes the exported openRuntime declaration/implementation async.
  The restricted public reader must reject that modifier at its source node.
* `reject-async-listener` gives observeState a `(value: Snapshot) => Promise<void>`
  listener. It is a distinct unsupported async-listener case; async openRuntime
  does not substitute for it.
* `reject-callback-result` gives the public observeState callback a number result.
  The restricted callback reader must reject it at the callback type node.

Run each via the public entry, never by importing a private adapter:

```sh
.venv/bin/python -I -S -B -X pycache_prefix=/dev/null scripts/build_arkts_routes.py \
  --output /private/tmp/madb-arkts-generation-rename-new \
  --route c --stop-after generate \
  --madb-image "$MADB_IMAGE" --madb-sha256 "$MADB_SHA256" \
  --finalizer-image "$FINALIZER_IMAGE" --finalizer-sha256 "$FINALIZER_SHA256" \
  --author-fixture tests/fixtures/c07_arkts_generation/rename-record \
  --cli-root "$CLI_ROOT" --cli-inventory "$CLI_INVENTORY" \
  --cli-inventory-sha256 "$CLI_INVENTORY_SHA256" \
  --python-image "$PYTHON_IMAGE" --java-home "$JAVA_HOME"
```

Use a different new output path and fixture directory for each negative.
The supported output is the host-checked Source batch named in result.json;
negative runs retain their real stderr/failed workspace and no Source candidate.
The diagnostic points into the actual `derived/Boundary.declarations.ets`
compiler input, including line and column. The source declaration is not a
hand-written expected descriptor. Lab owns the expected assertions. Dirty
producer development requires the explicit `--allow-dirty` flag; it cannot
produce a formal candidate. `--author-fixture` is rejected for Build/Package.

The public diagnostic contract for these three rejection fixtures is:

| Fixture | Phase | Required classification and source node |
| --- | --- | --- |
| reject-async | compiler declaration/read during preparation, before committed Generate | `ARKTS_DECL_UNSUPPORTED_MODIFIERS: unsupported export/member modifiers` at exported `openRuntime` (current actual input 507:1) |
| reject-async-listener | compiler declaration/read during preparation, before committed Generate | `ARKTS_DECL_CALLBACK_RESULT: callback must return void` at the Promise return-type node (current actual input 13:47) |
| reject-callback-result | compiler declaration/read during preparation, before committed Generate | `ARKTS_DECL_CALLBACK_RESULT: callback must return void` at the `observeState` callback type (current actual input 13:26) |

The diagnostic is `<logical-source-path>:<line>:<column>: <classification>: <message>`
within CLI stderr. These are backend declaration diagnostics, not changes to the
core wire error catalog. Each command exits 1 with no committed Source. Missing
SDK, tool errors, policy drift and other nonzero exits do not satisfy these
negative cases. Fixture line/column follow the actual source, not a forged map.

The executable pair must match the candidate's pinned host/finalizer bytes.
Use the same `MADB_IMAGE`, `MADB_SHA256`, `FINALIZER_IMAGE`, `FINALIZER_SHA256`
for all four commands. This entry never silently recompiles its own tools.
