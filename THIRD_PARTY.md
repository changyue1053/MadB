# Third-party scope and publication review

The host and repository checks use the following direct dependencies; the original C01 entries remain listed for continuity. Cargo packages are resolved by Cargo.lock; Python wheels and versions are fixed by requirements-dev.lock. Project-authored files use Apache-2.0; third-party rights remain unchanged.

| Component | Version | Declared license |
|---|---|---|
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| rustix (Unix host reads/process supervision) | 1.0.8 | Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT |
| subprocess (Unix execve launcher) | 1.2.1 | Apache-2.0 OR MIT |
| signal-hook (Unix CLI cancellation) | 0.4.5 | MIT OR Apache-2.0 |
| signal-hook-registry (signal registration) | 1.4.8 | MIT OR Apache-2.0 |
| libproc (macOS group observation) | 0.14.11 | MIT |
| errno (macOS query error state) | 0.3.14 | MIT OR Apache-2.0 |
| clap | 4.6.7 | MIT OR Apache-2.0 |
| clap_builder | 4.6.7 | MIT OR Apache-2.0 |
| clap_lex | 1.1.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| serde (host) | 1.0.229 | MIT OR Apache-2.0 |
| serde_json (host) | 1.0.154 | MIT OR Apache-2.0 |
| jsonschema (Rust host; HTTP/file resolution disabled) | 0.60.1 | MIT |
| sha2 (host) | 0.11.1 | MIT OR Apache-2.0 |
| toml (host; parser only) | 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 |
| jsonschema (checks only) | 4.26.0 | MIT |
| jsonschema-specifications (checks only) | 2025.9.1 | MIT |
| referencing (checks only) | 0.37.0 | MIT |
| rpds-py (checks only) | 2026.9.1 | MIT |
| attrs (checks only) | 26.1.0 | MIT |

The pinned GitHub Actions are maintained in .github/workflows/ci.yml; their upstream licenses apply. No upstream generator implementation or embedded template fragment is bundled. The C02 ABI fixtures are project-authored expectations. C03 additionally records declarations generated from these project-authored fixtures; these are test baselines, not a published SDK.

The project owner authorized public visibility on 2026-10-24. The C01 publication scope covers the project baseline and authored design/research material. Historical research and cited excerpts in docs/references retain their source attribution and any applicable third-party rights. The original ZIP contains 20 Markdown/JSON records for traceability; it is not a separately licensed upstream source distribution. No third-party native library or generator implementation is embedded in it. Publication checks are recorded in [the C01 review](docs/reviews/2026-10-24-c01-public-repositories.md).

Future imported code, template fragments and release assets require their own source and license review before redistribution. Public visibility and the project LICENSE do not grant rights to third-party material.

Package coordinates are not reserved by publish=false. The initial distribution channel is the owned GitHub repositories; future crate/Maven/Swift/HAR coordinates must be verified when those channels are enabled.

C02 resolved 99 registry packages, including target-specific dependencies; their versions, registry checksums and declared license expressions are recorded in [the dependency inventory](docs/reviews/c02-dependency-licenses.json). All reported a license. The `r-efi` alternative permits MIT; Unicode data dependencies additionally require Unicode-3.0 notices. This source-only change does not publish a compiled distribution. C10 must collect applicable license texts/notices for the dependencies actually redistributed in a release; a metadata inventory alone is not that bundle.

C03 invokes bindgen-cli 0.73.2 (BSD-3-Clause) and cbindgen 0.29.4 (MPL-2.0) as external development tools. Their source archive and dependency-lock hashes are in `tests/integration/c03/toolchain.json`; the verified installer fetches them from crates.io. Tool binaries, Apple SDKs and Rust standard-library archives are not committed or redistributed. Generated test declarations derive from project-authored inputs; a future compiled SDK/tool distribution still requires the C10 license/notice review.

C04 uses the safe `rustix` filesystem APIs without relaxing the workspace unsafe-code prohibition. The 14 added resolved packages, including target-specific dependencies, are listed in [the C04 dependency inventory](docs/reviews/c04-dependency-licenses.json). No dependency source or binary is vendored by this change; release notice collection remains C10.

The C04 process slice adds a pinned safe execve launcher and macOS group inspection. Its 16 additional resolved packages, including libproc's bindgen 0.72.1 build dependency and target-only packages, are recorded in [the process dependency inventory](docs/reviews/c04-process-dependency-licenses.json). This bindgen version generates host system declarations; it does not change the C03 route generator pin. Linux does not build the macOS-only libproc dependency. No dependency sources/binaries are vendored; C10 remains responsible for redistributed notices.
