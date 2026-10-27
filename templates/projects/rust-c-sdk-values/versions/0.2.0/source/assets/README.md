# Frozen Rust-to-C values project

Run `./madbw build --frozen` after changing `rust/src/lib.rs`, `consumer/main.c`,
or files under `consumer/include`. Every build captures fresh author bytes,
builds a complete SDK and consumer, and publishes only a complete verified
batch. The root CMake `madb_build` target uses this same command.

This finite recipe supports one Rust values source, macOS aarch64 and Debug.
It does not adopt arbitrary Cargo dependency graphs or asynchronous resources.
The bundled template/profile/SDK and project-local MadB image remain pinned.
Missing local tools can be restored with the same source-bound MadB image:
`madb project install-tool --root /absolute/project --frozen`.

Previous complete batches remain under `.madb/build/attempts`; failures do not
replace the previous current result. Do not edit the origin or descriptor to
upgrade a project. IDE import/debug and existing-project migration have separate
acceptance and are not completed by this template.
