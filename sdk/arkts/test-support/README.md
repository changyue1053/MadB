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
