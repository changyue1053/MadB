# Template bytes and finite rendering

`madb-templates` implements the pure byte operations adopted by
[ADR-0023](../../docs/adr/0023-project-template-and-init-result.md).
It does not read files, acquire packages, extract archives, invoke tools or
obtain Host permission to create a project.

`parse_archive` requires the complete original bytes and the expected digest
and size from a trusted catalog entry. It accepts only the approved ordinary
file USTAR subset: canonical metadata and octal fields, verified checksums,
ASCII root-relative names, sorted entries, zero padding and exactly two zero
trailer blocks. The full original archive remains immutable. Path conflicts
and all-file budgets reuse the pure project rules, including the manifest.

`CheckedArchive::check_inventory` compares the externally pinned manifest and
its complete `files` inventory against every actual entry and payload. It does
not ignore unselected entries. `CheckedInventory::referenced_bytes` verifies a
File/FileRef's raw identity in that single package root; schema and reference
closure semantics are checked by the contracts adapter. A nested manifest does
not change the package root. These values do not establish catalog trust or an
executable recipe.

`render_asset` obtains source bytes from that checked inventory. Local-state
assets return `None` and only reserve their path. All other assets select one
of three renderers:

- `copy` preserves every byte, including marker-looking text.
- `json-values` strictly parses the source, enforces the 4096-element array
  limit at every depth, resolves nonoverlapping existing scalar/null pointers,
  replaces them with typed parameter values, and emits canonical JSON plus LF.
- `identifiers` handles only the C/TOML/CMake finite identifier form. It scans
  source markers once, requires all declared parameters to appear, rejects
  unknown/malformed markers and reserved/unsafe values, and bounds the output.

Recognized MadB JSON output is passed through the existing schema decoder and
an explicit semantic-validation callback. The trusted composition root must
bind that callback to the applicable contracts and frozen-reference checks.
An unconditional `Ok` callback is only a test substitute, not semantic proof.
Changing an existing MadB document's schema cannot bypass this step. General
business JSON does not acquire a MadB document identity or execution rights.

Rendering does not certify arbitrary token contexts. The finite recipe must
also bind the original asset context through the trusted package identity and
validate its C/TOML/CMake layout; real parsing, compilation and execution remain
separate checks. Provenance bytes are exposed for the application's same
initial checked file plan; this adapter does not append files after commit.

Run focused checks with `cargo test --locked -p madb-templates`. The tests
exercise original design package bytes and independent malformed archives,
inventory mismatch, pointer boundaries, scalar/array budgets, marker injection,
and callback rejection. They do not claim real project creation or IDE use.
