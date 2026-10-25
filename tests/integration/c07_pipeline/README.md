# C07 frozen pipeline regression checks

Scope: ADR-0010's Python preparation seam and committed evidence enumeration.
The adapter retains its default compile/link/run linear plan. Only a profile
that requires `core.external-runtime-verification@1` may use compile/link, and
it must also declare support. These are the only complete linear plans offered
by this helper; the Rust owner remains the authority for capability registration,
unknown required features, general graph execution, commits and certification.

Preparation validates the whole phase plan before worker import or execution.
It requires a successful host generation context and binds the profile's read
reference and semantic digest, contract, committed source batch, generation and
output-set identity.
Before import it also checks the host image, the original input references fixed
for successful generation, and each generated file's committed SHA/size. Copying
still checks those identities again. This rejects ordinary changed inputs; it
does not promise isolation from an active concurrent attacker.
Package evidence comes exclusively from that Build batch's
metadata file inventory, retaining the declared SHA/size references. Empty,
duplicate, malformed or noncontiguous evidence inventories are rejected; stray
files in the build directory are not evidence.

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/c07_pipeline -v
.venv/bin/python -B -m unittest discover -s tests/integration/c05_pipeline -v
```

Tests use real import/execution markers for the preparation helper and synthetic
host commit records. They establish adapter pre-execution and inventory behavior,
not actual CLI acceptance, compilation, target execution or Lab certification.
The Rust host's separate tests cover unknown features and full parent lineage.
