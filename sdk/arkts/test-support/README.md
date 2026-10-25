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
closed-set Node-API observer. It has 31 private native methods and 18 public test
helpers. The current observation contract is 178 values, including a live process
segment that is explicitly outside the stable physical-ACK snapshot. See the
versioned contract attachments. These changes do not appear in ordinary packages.

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
worker. Each fresh `wrong-abi`, `wrong-profile` and `wrong-build` directory retains
its source references, three constant edits, output/log hashes and fixture receipt.
The unchanged public `openRuntime()` is expected to reject with `AbiMismatch`,
`ProfileMismatch` and `ArtifactMismatch`, respectively. The helper checks those
codes and their event increments against the consumed gateway source; a changed
contract fails preparation. The independent consumer must isolate each DSO and
measure the real before/after `testBootstrapTotals()` values. Expected deltas are
one open attempt and one corresponding rejection; identity match and resource
creation counters stay unchanged. These expectations are not execution evidence.
