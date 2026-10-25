# C07 private application-tool input verification

Scope: C07 application CLI acquisition and installed-byte agreement under the
current ADR-0010/tool-input boundaries. This is not an installer, public schema,
new CLI, profile admission, source provenance closure or C07 acceptance.

`acquire_inventory(archive, expected_sha256=..., expected_size=...)` requires an
explicit local ZIP and caller-reviewed checksum/size. It hashes the archive and
streams every member without extraction. The private result records local
agreement with a caller-reviewed expected checksum that was locally computed
from the user-supplied archive; it explicitly does not claim an
official checksum or signature was verified. The caller persists it as an
ordinary frozen input. Load saved JSON with the existing strict
`preflight.read_json_metadata` reader, and verify the saved file's original
FileRef before consuming it; the inventory's own digest is not authentication.

`verify_inventory(inventory, root)` checks the explicit installed tree against
that original closed inventory and rejects missing, added or changed entries.
It reads strict SDK/package metadata only after byte agreement, checks component
consistency and reports the exact launcher/image/link identities. It never
executes any launcher, including hdc. Future workers must call it before and
after their phase with the same frozen inventory. Ordinary changes are detected;
this read-only Python check does not provide isolation from an active attacker
mutating files concurrently.

The supported archive layout has one explicit `command-line-tools/` root,
explicit parent directories, Unix regular files/directories/symlinks and only
stored or DEFLATE entries. Encrypted, privileged, special, duplicate,
case/Unicode-colliding, escaping or noncanonical names fail. Limits are 200,000
entries, 1 GiB per member and 12 GiB total uncompressed bytes. Symlinks must use
relative targets, have no escaping/cyclic chain and terminate at an inventoried
regular file. Every traversed intermediate component must be a real inventoried
directory, including before `..`; directory symlinks are unsupported. All
installed symlinks are strictly resolved against their closed identity, including
non-launcher links. Link text is exact. Symlink
permission bits are not compared because macOS extraction does not preserve
them; regular-file and directory permission bits are compared.

The only metadata exclusions are top-level ZIP `__MACOSX` directories with
non-executable AppleDouble `._` sidecars that name a real archive member, and
regular non-executable files named exactly `.DS_Store`. These are recorded.
Arbitrary `._*`, extra installed `__MACOSX`, caches or executable metadata names
are not ignored. No mismatch repairs, cache deletion or ignore-list expansion
occur. CPF compiler/LLVM/sysroot/runtime inputs stay a separate incomplete group.

Focused verification:

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/c07_tools -v
```

The synthetic ZIP/tree fixtures exercise real hashing, ZIP decoding and
filesystem changes. Their launcher marker stays absent. They do not prove a
target compiler invocation, HAR/HAP consumption, r3 runtime or device behavior.
