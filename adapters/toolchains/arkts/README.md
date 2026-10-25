# Restricted ArkTS C04 entry

`scripts/build_arkts_routes.py` is a composition root over the existing C04
`FrozenPipeline` and public MadB CLI. Metadata and Generate use the real SDK
TypeScript parser and Clang AST, followed by the core finalizer. Build prepares
and replays an explicit compile/link plan. Package builds a real Release HAR,
checks the compiler-emitted declarations, native closure and notices, and only
normalizes archive metadata. No device or Lab verification runs here.

The current entry supports ordinary and test-companion C and C++ profiles.
Select `--variant test` for the separately identified companion; its strict
composition has 31 private native methods and 25 public exports. Rust and Kotlin helpers
are being connected separately; their existence is not a completed candidate.
The ordinary required five cases and companion required twenty-five come from
the separately pinned Lab allocation. A five-case candidate alone does not pass
C07. Verification remains `missing` until independent Lab observations exist.

First prepare the MadB CLI and finalizer with the repository's standard locked
Cargo build, under a known build environment:

```sh
cargo build --locked -p madb-cli --bins
shasum -a 256 target/debug/madb target/debug/madb-native-finalize
```

Record those exact executable paths as `MADB_IMAGE`/`FINALIZER_IMAGE` and hashes
as `MADB_SHA256`/`FINALIZER_SHA256`. Reuse the same pair for candidate production
and Lab generation. The route entry does not run Cargo, select a compiler wrapper,
or assume an old target directory was freshly built. Independent pins prove tool
byte identity; source provenance requires the separate recorded build.

Then, from a clean producer checkout, with the reviewed CLI inventory and
independent Lab requirements available:

```sh
.venv/bin/python -I -S -B -X pycache_prefix=/dev/null scripts/build_arkts_routes.py \
  --output /private/tmp/madb-arkts-c-candidate-new \
  --route c \
  --madb-image "$MADB_IMAGE" --madb-sha256 "$MADB_SHA256" \
  --finalizer-image "$FINALIZER_IMAGE" --finalizer-sha256 "$FINALIZER_SHA256" \
  --cli-root "$CLI_ROOT" \
  --cli-inventory "$CLI_INVENTORY" \
  --cli-inventory-sha256 "$CLI_INVENTORY_SHA256" \
  --python-image "$PYTHON_IMAGE" --java-home "$JAVA_HOME" \
  --case-allocation "$CASE_ALLOCATION" \
  --case-allocation-sha256 "$CASE_ALLOCATION_SHA256" \
  --requirements "$ROUTE_REQUIREMENTS"
```

Every path is explicit. The output must be new. The CLI inventory binds the
complete installed distribution to locally computed archive bytes; it is not
an official signature. The program also checks actual Node, TypeScript and
Clang versions. `inventory-format.json` specifies exact canonical JSON and the
separate CLI/JBR row formats for independent consumers. Hvigor's executable input
is `hvigor/hvigor/bin/hvigor.js`; the plugin and the rest of the installed SDK are
covered by the full CLI inventory. Python and JBR have separate runtime trees.

`--allow-dirty` labels a development result provisional. Formal preparation
checks both a clean revision and every copied repository source/helper/schema
against that revision. Generated composition is replayed from original inputs;
prebuilt SDK libraries are never accepted as author declarations. Failed command
scratch, bounded stdout/stderr and statuses remain in the new workspace.
Successful Build command logs retain their original admitted SHA/size and are
copied into explicit Package inputs so Candidate evidence is self-contained.

`candidate/generated/contract/` contains the original role policy, Frame layout,
inventory definition and selected independent case-allocation inputs. State
`capacity_items=3` counts one latest pending version, one entered callback and
one staging version responsibility per selected subscription. It does not mean
three publicly queued states; the shared retained UTF-8 budget remains 4096.
Resource `shared` slots represent backend references; their public owners close
them. The ArkTS-owned erased error channel is not a closable SDK resource.

For independent generation checks use the real author fixtures described in
`tests/fixtures/c07_arkts_generation/README.md`. The entry freezes those actual
source/policy files and stops after Generate. It never manufactures a replacement
metadata-only API. The runtime state machine remains an explicitly authored SDK
template; the generator reads declarations, freezes policy, and renders only the
module and expected-identity string literals.

Focused checks:

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/c07_arkts_adapter
.venv/bin/python -B -m unittest discover -s tests/integration/c07_native_gateway
```

These are registered in `scripts/check.py`. They are host boundary tests, not
Harmony target certification. The public entry exercises actual compilers and
C04 ports and produces the separate artifacts needed by Lab.

Generation replay binds the complete executable source closure, not only the
entry script hash: `madb.lock.inputs` includes `scripts/build_arkts_routes.py`,
the project/pipeline/requirements composition code, all invoked declaration,
lowering, worker and Package helpers, common C04 helpers, original native author
sources, template/policy and tool inventories. The candidate carries this lock;
the public Generate result explicitly names its own input_manifest/read_request
and checked ContractIR. Lab can compare immutable helper/tool rows across its
fresh generation and the candidate while independently accounting for the
intended author-template/policy change and its derived files. A different helper
closure cannot be silently treated as proof for an older candidate.

Public IDs are authored per call in the frozen role policy. The lowerer rejects
missing, malformed or duplicate IDs before generation, and never labels a
name-concatenated default as explicit. The rename fixture deliberately preserves
the old stable capability ID while its SourceKey, actual API name and shape
change; retaining that ID is not a compatibility verdict.

The public Python entry requires `-I -S -B -X pycache_prefix=/dev/null` before
any adapter import. Run it from a trusted host/launcher; this is not a defense
against a replaced Python executable or compromised OS. Every child tool and
MadB owner receives a fixed base environment plus declared task settings.
Caller `NODE_OPTIONS`, `JAVA_TOOL_OPTIONS`, dynamic-loader and compiler-wrapper
variables are not inherited. Package's HOME/cache directories are task-owned.
The two optional common hooks preserve existing C05/C06 launch behavior by
default; only this route selects the restricted environment.

The companion's exact original ordinary source, native/facade patches and derived
source pins are published in `generated/contract/composition.json`; all originals
also remain in the Source/lock inventory. `generated/contract/test/` contains
PORTS, versioned layouts and units. The 178-element native diagnostic frame, the
16-element bootstrap totals and 8-element delivery-gate frame are distinct;
changing an actual layout without its explicit bounded array role is rejected.
The test-only number-array projection is a logical ArkTS value copy, not a C
contiguous buffer. Each scalar is an exact checked JS-safe integer; command
parameters have explicit ordinal/count domains. The Runtime-owned one-shot
settlement hook remains retained after registration; it is not a subscription
listener or a call-frame borrow. Ordinary reader acceptance is unchanged.
