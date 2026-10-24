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

The project owner authorized public visibility on 2026-10-24. The C01 publication scope covers the project baseline and authored design/research material. Historical research and cited excerpts in docs/references retain their source attribution and any applicable third-party rights. The original ZIP contains 20 Markdown/JSON records for traceability; it is not a separately licensed upstream source distribution. No third-party native library or generator implementation is embedded in it. Publication checks are recorded in [the C01 review](docs/reviews/2026-10-24-c01-public-repositories.md).

Future imported code, template fragments and release assets require their own source and license review before redistribution. Public visibility and the project LICENSE do not grant rights to third-party material.

Package coordinates are not reserved by publish=false. The initial distribution channel is the owned GitHub repositories; future crate/Maven/Swift/HAR coordinates must be verified when those channels are enabled.
