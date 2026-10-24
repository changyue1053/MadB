# First-route native reader and generator

This stdlib-only helper implements C05's restricted `madb.first-route/1` recipe.
It is a locked external peer of C04, not a fixture response producer. Invoke it in
the host's private `work` directory with a sibling frozen `inputs` directory:

```text
python driver.py metadata --config ../inputs/backend.json < read-request.json
python driver.py generate --config ../inputs/backend.json < generate-request.json
```

The response is written byte-for-byte to stdout and `contract.json` or
`outputs.json`. Temporary compiler files are removed before success. The host
validates the protocol, exact work tree, immutable input bytes and final commit.
This helper never changes a current index, supplies build evidence, or certifies
an SDK. Build-stage export checks and independent Lab consumption remain required.

## Inputs and responsibilities

`driver.metadata_seed(request, config, input_root, work)` supports initial
preparation of explicit project policies. Formal metadata always reads the real
source again. It sends `{contract, project, profile}` to the locked
`madb-native-finalize` binary, which owns policy resolution, domain validation and
IR hashes. The Python code does not reproduce the policy resolver or load a
prewritten ContractIR. Generation repeats reading and finalization and requires
exact equality with the requested ContractIR before producing public files.

The internal, versioned configuration has these fields:

| Field | Meaning |
| --- | --- |
| `schema`, `semantics` | Exactly `madb.rust-c-backend/1`, `madb.first-route/1` |
| `provider` | `rust` or `c` |
| `source`, `project`, `profile` | Logical frozen input paths |
| `exports` | Each exact native name, semantic `role`, and explicit `public_id` |
| `tools` | Installed absolute `path`, byte `sha256` and `size`; optional argv prefix; Clang also has profile-matching `id` and `version` |
| `clang_arguments` | Explicit target, SDK/resource flags and warning policy |
| `rust_target` | One of the two Apple or two Linux 64-bit targets admitted by the implementation |
| `environment` | Only the adapter's explicit locale, SDK, deployment and libclang whitelist |
| `sdk_inputs` | Exact absolute SDK/header references observed in actual Clang `-M` discovery |
| `cbindgen_config` | Rust provider's frozen `sdk/rust/engine/cbindgen.toml` |
| `wrapper` | C provider's frozen `manifest`, `source`, and `build_script` paths for the public Rust crate |
| `test_support_header` | Optional, separately identified test-only header path |
| `test_support` | Optional boolean; must agree with `test_support_header` when present |

Source, cbindgen config, wrapper files and any test header must be in the request
input inventory. cbindgen's real depfile must be a subset of that inventory. The
single primary C header may include only the locked SDK headers: indirect user
headers require another explicitly supported reader. Clang `-M` verifies this
actual read set. Every tool and SDK reference is byte-checked before and after an
operation. The install image and declared SDK inputs are not a sandbox or a
promise of a complete dynamic loader environment. There is no persistent cache.
Compiler flags use a closed recipe: one explicit target, C11, optional installed
SDK/resource directories, the Apple 14.0 deployment flag where applicable, and
`-Wall`/`-Wextra`/`-Werror`. Architecture overrides, frontend passthrough options,
and tool-image argument prefixes are refused rather than allowed to change the
declared target after validation.
The internal `backend.json` is included in that frozen source inventory, rather
than registered through `profile.backend_config`. The latter remains `null`:
C02's generic backend-config validator still refuses unregistered configurations
with CAP005. This first-route helper validates its own exact versioned recipe.
The public profile's `sdk` refers to `toolchain.json`: portable tool image
identities, SDK content identities under logical names, and observed compiler
semantics. It contains no producer filesystem paths. Public tool `arguments` are
empty; actual executable arguments and absolute locators are preserved only in
the locked internal backend input, not represented as portable commands.

## Actual declaration reading and lowering

For Rust, the locked cbindgen creates the C header from the real implementation;
Clang reads its AST, macros and record layouts. Its logical symbols are marked
`artifact-derived`. A unique source-function locator supplies the original Rust
line/column only; it does not infer semantics. For C, Clang reads the author header
and the symbols are `compiler-derived`. Runtime addresses, Clang node IDs, and
temporary output paths do not form source identities. Named typedefs for anonymous
C structs are resolved through the actual Clang declaration relationship.

The configuration maps roles, not copied signatures. ABI v1 requires the exact
status-return/output-argument signatures already defined by the first-route
contract. Rust provides bootstrap, create, checksum, request-close, await-closed,
release-handle and observe. C provides its independent bootstrap and checksum.
Unknown exports, typedefs, records, native types, function attributes, macro
declarations or status values are refused. `MadbError` fields and compiler-observed
size/alignment/offsets must match the ABI. Reordered fields, changed pointer
constness or widths, and changed calling conventions cannot silently enter IR.

The explicit roles fold pointer/length into the bounded byte buffer, status/error
outputs into the established error protocol, value output arguments into logical
results, and callback/context into the versioned callback contract. `handle` is
the SDK's registered resource-token type; runtime engine/subscription checks are
still required. Bootstrap returns the fixed 64-byte identity payload. Callback
threading, retention, reentrancy, close, and ownership policies remain explicit
project policies checked by the Rust resolver. No async/queue/stream semantics are
inferred from this synchronous surface.
Observe calls are `unbounded`: the initial foreign callback runs synchronously
and has no declared time bound. Request-close does not wait for callback drain;
successful close waiting is the separate `await_closed` operation.

## Identity and outputs

`interface_digest` is SHA-256 over `madb.native-interface/1\n` and canonical JSON
containing `semantics`, `bootstrap_version: 1`, and the sorted actual native ABI:
function names, parameter names/types and returns; typedefs; record fields;
compiler-observed layouts; and public integer macros. Source locations, source
file bytes, runtime implementation bytes, profile, generation and candidate
identities are excluded. The profile digest uses the existing MadB profile rule.
This prevents generation/candidate hash cycles while binding the exact public
declarations and semantic version. A new observed ABI changes the interface
identity, or is refused when outside ABI v1.

All identity artifacts derive from the same `madb.native-identity/1` value.
Rust provider outputs are `include/madb_rust.h`, `identity.json`, and engine
`identity.rs` (`INTERFACE`/`PROFILE`). The optional test profile also supplies
`include/test-support.h`. C provider outputs are `include/madb_c.h`, `identity.h`
(`MADB_C_INTERFACE_DIGEST`/`MADB_C_PROFILE_DIGEST`), `identity.json`, and
`rust/madb-c-client/{Cargo.toml,build.rs,src/lib.rs,src/raw.rs,src/identity.rs}`.
The last Rust file uses `INTERFACE_DIGEST`/`PROFILE_DIGEST`; raw declarations come
from the real locked bindgen and the reviewed safe wrapper is an explicit input.

The final C header, optional combined test header, and generated Rust wrapper
receive real syntax/declaration compilation before OutputSet is returned. This
does not replace actual native-library symbol checks, linking, bootstrap mismatch
tests, or running the two independent consumers.

## Focused checks

```sh
cargo build -p madb-cli --bin madb-native-finalize --locked
PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest discover -s tests/integration/c05_reader -v
```

The focused suite currently uses the exact C03 Apple tool/SDK recipe and verified
generator receipt, defaulting to `/private/tmp/madb-c03-verified-tools`; override
its install root with `MADB_C03_TOOLS`. Missing tools are failures, not skipped
passes. The separate native Linux preparation/CI profile does not inherit Apple
runtime certification.
