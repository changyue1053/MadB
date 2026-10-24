# C04 real execution, storage and cache acceptance

Run from the repository root:

```sh
.venv/bin/python -m unittest discover -s tests/integration/c04_package -p 'test_*.py' -v
cargo clippy -p madb-cli --example package_probe --example dependency_probe --locked -- -D warnings
```

A working Clang toolchain is required. macOS fixtures resolve the compiler and SDK through `xcrun`; Linux resolves Clang from `PATH`. Missing tools fail explicitly. Fixtures hash the selected compiler image and actually compile and execute consumers. These host tests do not certify a boundary route, SDK, device, or cross-compilation target.

## Proof boundaries

- `test_transactions.py`: real independent processes exercise the host store through `package_probe`. Python independently verifies content identities, persisted metadata, index lineage, file inventories and exact fixture bytes. Explicit file barriers establish ordering for concurrent publication, final input revalidation, cancellation, old consumer leases and `SIGKILL` at four transaction stages. Unknown nonempty domains and missing/corrupt ownership markers must fail without claiming or deleting unrelated files. Timeouts are watchdogs, never race ordering. Killing a process is not a power-loss durability test.
- `test_cli.py`: the public `madb` executable reads genuine execution bundles and port payloads. Its fixture generator checks known C source/header bytes with an exactly selected Clang image and `-fsyntax-only` before returning an OutputSet; syntax failure preserves the old source batch. It then runs actual compile/link/run tasks, publishes three fixed batch layers and packages an object consumed by a fresh downstream C program. It also checks metadata preparation without existing IR, read-only queries, generation drift, corrupted inventories, missing preparation, incomplete named build subgraphs and real CLI signal cancellation.
- `test_cli_dependencies.py`: the public CLI explicitly prepares native dependencies, returns immutable receipt/output paths, and verifies cold/hot output through a real consumer. Generation must explicitly bind every receipt input. Missing preparation and a changed installed compiler cannot reuse an old receipt or silently start discovery. Managed output/cache locations inside an include search tree are rejected before any write.
- `test_dependencies.py`: `dependency_probe` composes the production restricted Clang discovery, preprocessing and cache adapters. Actual transitive headers, include search order, optional headers, directory membership, explicit macros/environment/context and compiler bytes determine reuse. Corrupt or missing cache components must miss, with the old entry retained unchanged in quarantine. The target triple is explicit; a native CLI target that conflicts with the selected profile is rejected. Cold and hot generated C is compiled and executed against independently stated numeric expectations. Unknown options and caller-supplied completeness claims cannot grant cache eligibility.

The protocol peer in `protocol_helper.py` is a deliberately synthetic locked fixture. Its nested compiler call is checked against an explicit image digest. It is not a production language generator or reader and does not prove a complete dependency closure; normal CLI generation of that helper remains uncacheable. The restricted Clang suite supplies the separate closure proof.

## Ownership and cleanup

`package_probe.rs` and `dependency_probe.rs` are test-only composition roots, not additional public JSON APIs. They exercise real application/host ports and report ownership outcomes; fixtures do not supply preapproved closures or production-computed expected identities.

`generate --check` recomputes in distinct private run directories outside the source tree. The CLI retains run evidence according to its process-owner policy; the test does not infer that arbitrary descendants are safe to delete. These known fixture peers create no background descendants. After their owned processes finish, the fixture explicitly removes only its recorded private run directories. Effective source, lock, current index, valid batches and cache stay unchanged during checks.

Return categories are asserted where meaningful: semantic drift is `1`, missing preparation or invalid peer payload is `2`, and corrupt committed storage remains `4` with `STORE004`. A nonzero exit alone does not prove cancellation: signal tests additionally require a cancelled diagnostic, retained evidence, an unchanged current batch and a reaped direct helper process.
