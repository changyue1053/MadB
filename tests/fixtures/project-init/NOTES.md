# I01 shared format fixtures

The files listed in `fixture-provenance.json` are byte-for-byte copies of the
approved round-seven design assets. Their original model-only labels, source
paths, hashes and hypothetical creation facts are preserved. Tests resolve
fixture-relative paths; they do not read those historical absolute paths.

`format-vectors.json` supplies structural expectations shared by the Rust and
independent Python schema engines. A schema-valid negative design example is
explicitly marked as requiring semantic, package or frozen-fact rejection.
The three raw lexical negatives are checked before schema validation.

These fixtures are not a trusted executable template catalog, a Host commit
receipt, an initialization run or a platform/IDE acceptance report.
