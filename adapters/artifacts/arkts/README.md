# ArkTS HAR container preparation

`har.py` is a private packaging utility, not the complete C07 Package port. It
reads a bounded gzip/tar HAR without extraction, preserves file bytes and modes,
sorts members, and normalizes user/time metadata. The final reader compares the
output with the original member set. The selected recipe rejects links, devices,
sparse files, unsafe or aliased names, unknown PAX semantics and exceeded budgets.
It does not infer API/profile compatibility from a manifest or native filename.

Two actual cold Hvigor 6.26.8 Release/obfuscated builds produced identical member
contents but differing tar times. This step produced identical archives, and
formal ohpm 26.0.0.630 installed all 30 files with matching bytes. See the
[C07 observation](../../../docs/reviews/c07-har-replay-observation.json).
These are basic mechanism fixtures, not complete SDK candidates or target tests.

The future Package adapter must bind this recipe and the immutable Build inputs.
The current experiment sets `nativeLib.debugSymbol.strip=false`; any stripping
must be an explicit Build output before packaging. This utility neither compiles
nor silently changes native libraries. The final HAR manifest, dependency closure,
bootstrap identity and independent HAP consumption remain separate C07 checks.
