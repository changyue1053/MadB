# Third-party scope and publication review

C01 uses the following locked development dependencies. Cargo packages are resolved by Cargo.lock; Python wheels and versions are fixed by requirements-dev.lock. Project-authored files use Apache-2.0; third-party rights remain unchanged.

| Component | Version | Declared license |
|---|---|---|
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| clap | 4.6.7 | MIT OR Apache-2.0 |
| clap_builder | 4.6.7 | MIT OR Apache-2.0 |
| clap_lex | 1.1.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| jsonschema (checks only) | 4.26.0 | MIT |
| jsonschema-specifications (checks only) | 2025.9.1 | MIT |
| referencing (checks only) | 0.37.0 | MIT |
| rpds-py (checks only) | 2026.9.1 | MIT |
| attrs (checks only) | 26.1.0 | MIT |

The pinned GitHub Actions are maintained in .github/workflows/ci.yml; their upstream licenses apply. No upstream generator implementation, embedded template fragment or generated SDK has been copied into C01 product code.

Historical research and cited excerpts in docs/references retain their source attribution. The original ZIP snapshot is an internal traceability artifact, not a rights clearance or an independently licensed third-party source bundle. Repository visibility is private during preparation. Before changing visibility or publishing artifacts, the project owner must review redistribution rights for archived material and any new embedded fragments; unreviewed content must remain excluded from public distribution.

Package coordinates are not reserved by publish=false. The initial distribution channel is the owned GitHub repositories; future crate/Maven/Swift/HAR coordinates must be verified when those channels are enabled.
