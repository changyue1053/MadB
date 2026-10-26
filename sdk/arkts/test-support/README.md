# Independent ArkTS test companion

This directory is excluded from ordinary source composition and packaging.
`native/companion.patch` derives the test gateway and instrumented worker from
one exact ordinary baseline. Additional support units are separate allowlisted
files. `facade/companion.patch` derives the test facade from the ordinary facade;
there is no second maintained facade implementation. Each baseline JSON binds
all original and derived file hashes. The composer rejects drift before writing
an output, uses exact context and line offsets, and never applies fuzzy patches.

The initial companion preserves the reviewed C/C++ development test mechanisms:
real worker barriers, actual publication ownership, provider fault ports,
completion guards, bounded registry history and partial-open rollback. Its initial native
156-value frame and the separate facade frame are versioned test contracts, not
public business APIs. `contracts/diagnostic-units-v1.json` specifies counts versus
bytes. All contract attachments must be explicit distributable candidate files;
a nearby build directory is not a package dependency. Later observation changes
must retain or version these contracts and receive a new build identity.

The initial facade derivation matched historical fa07 bytes, SHA-256
`fa07ab119ae87b8929d6a850e99669107eb0564cf2ac6d93107dccf86cd794af`.
The original native test binary was SHA-256
`4c4ecb3ee9bec0b4cd707489df38d6703fde6738f57ab18a88891fa921b86dca`.
These are provenance observations, not build inputs or certification of a new
candidate. Repository changes and generated identity require a new build.

Focused tests are under `tests/integration/c07_native_gateway`. The native host
harness executes real Core/provider/worker code, with an explicit small Node-API
return harness only for registry checks. It does not emulate a JavaScript engine
or establish target Node-API lifecycle behavior.

Companion v2 extends that strict derivation with one held Delivery test seam,
shared native handle-admission controls, real bootstrap event totals and a
closed-set Node-API observer. That version had 31 private native methods and 18
public test helpers. The current companion adds native `testMemory` as ordinal 31
and pure ArkTS `testSubscriptionState`, for exactly 32 private native methods and
20 public test helpers. The observation contract remains 178 values, including a
live process segment outside the stable physical-ACK snapshot. See the versioned
contract attachments. These changes do not appear in ordinary packages.

`build_identity_fixtures.py` derives three CRIT-12 negative native libraries from
a caller-pinned C04 test-companion compile task and its exact frozen inputs. It
changes one ABI/profile/build tuple component and the two native strings derived
from that component. Facade bytes, public expected identity, module name, provider,
SONAME and build recipe remain unchanged. It invokes the parent's frozen
compile/link worker; it does not create a candidate, package a HAR or run a target.

```sh
.venv/bin/python -B sdk/arkts/test-support/build_identity_fixtures.py \
  --compile-task /absolute/workspace/tasks/compile.json \
  --compile-task-sha256 REVIEWED_SHA256 \
  --inputs /absolute/workspace/prepare/compile/inputs \
  --output /absolute/new-fixtures
```

Use `--prepare-only` to inspect the exact three derivations without loading the
worker. Preparation requires the actual 32-method companion declaration, including
`testMemory` at the final ordinal; old 31-method fixtures remain historical inputs
and are not relabeled or reused for a new candidate. Each fresh `wrong-abi`,
`wrong-profile` and `wrong-build` directory retains its source references, three
constant edits, output/log hashes and fixture receipt.
The unchanged public `openRuntime()` is expected to reject with `AbiMismatch`,
`ProfileMismatch` and `ArtifactMismatch`, respectively. The helper checks those
codes and their event increments against the consumed gateway source; a changed
contract fails preparation. The independent consumer must isolate each DSO and
measure the real before/after `testBootstrapTotals()` values. Expected deltas are
one open attempt and one corresponding rejection; identity match and resource
creation counters stay unchanged. These expectations are not execution evidence.

Identity-negative fixture construction accepts an actual ordinary or test C04
parent with the exact 16/32-method declaration and matching module/variant. It
changes only one compiled identity component and its derived strings, preserving
the original facade expectation. Ordinary fixtures carry no bootstrap counter
claim; same-source companion counter evidence remains independently required.
Every receipt and fixtures.json records the parent variant. These libraries are
separate negative fixtures, never additional members of the normal HAR.

The Kotlin helper at `sdk/kotlin/arkts-owned/test-support/build_identity_fixtures.py`
accepts exact actual 15/23-function KSP inventories for ordinary/test respectively,
with matching module and `test_support` flags. It changes the compiled identity
input while keeping the original facade; each `fixtures.json` records the variant.
The ordinary fixture has no counter claim. Companion observations independently
prove zero new owners through the shared admission gate source.


The C09 companion adds one pure ArkTS public `testBeforeRuntimeReceipt` helper
(the native C/C++/Rust inventory stays at 32 methods; public test helpers become
21). Kotlin derives the same cutpoint through `kotlin_test_binding.py`; its KSP
function inventory is unchanged. See [the exact registration, return and failure
contract](contracts/PORTS.md#c09-synchronous-before-receipt-probe). Host checks in
`tests/integration/c09_receipt_hook` execute both actual derived facades with a
controlled native model; they are not target or physical-cleanup evidence.
