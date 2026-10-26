# L02 diagnostic and explain checks

The ordinary Python discovery suite checks exact existing ArkTS diagnostic matching and source binding without invoking a compiler. The Rust `madb-contracts` `explain` test exercises the real application use case and format adapter, including the finite bundled catalog, no-context/unknown-code zero reads, source pins, UTF-16/BOM points, and I/O versus conflict exits. The shared Rust/Python format suites consume `explain-format-vectors.json`.

```sh
python -B -m unittest discover -s tests/integration/l02_diagnostics -v
cargo test -p madb-contracts --test explain --locked --offline
```

Real reader checks are explicit and require a current, pinned ArkTS tool observation. They do not silently skip a missing tool. Use the project's locked Python and a freshly built `madb` binary:

```sh
python -B tests/integration/l02_diagnostics/test_declaration_diagnostics.py \
  --tool-observation /absolute/path/tool-observation.json \
  --report /absolute/path/new-reader-report.json
python -B tests/integration/l02_diagnostics/check_explain_cli.py \
  --tool-observation /absolute/path/tool-observation.json \
  --madb /absolute/path/madb \
  --report /absolute/path/new-explain-report.json
```

The second command obtains two real coded reader failures, captures their exact source bytes and locations into the adopted `madb.diagnostic-context/1`, then invokes the public text/JSON explain commands. It verifies source/context identities, wrong-source and code-pair rejection, unknown-code rejection before missing-context reads, missing-file Host/3, and unchanged query workspace contents. It records the actual binary, tools, commands and relevant source hashes, and rechecks them before success. The test harness invokes the reader; the explanation query does not.

The single explanation data file is `adapters/generators/diagnostic-explanations.json`. Rust embeds it and the existing Python helper reads its two exact entries at module import. The helper remains limited to those two coded reader shapes; it does not infer codes from arbitrary stderr. The data file is an explicit ArkTS helper input, so changing it changes the generated tool/candidate input identity.

These reports establish host query/source-binding behavior. They do not prove SDK generation, resource/callback runtime behavior, independent Lab acceptance, publishing, or completion of L02's other acceptance conditions. Reports require a new output path; prior failures are retained.
