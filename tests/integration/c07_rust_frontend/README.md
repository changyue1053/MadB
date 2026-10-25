# Rust frontend boundary checks

Run `python -m unittest discover -s tests/integration/c07_rust_frontend`.
These focused rejection tests preserve the shared gateway statements, require
exact marker/observer authority, preserve raw zero-argument calls, and reject
export-order/type or ordinary/test mismatches. They do not certify a target SDK.

Real acceptance uses the existing C04 entry: fresh proc macros and resolved
features in Read/Generate, actual C++ archive and Rust type checking in Compile,
then ohrs code generation/link and ELF checks. Ordinary/test exports must be
16/32, with the same actual Rust provider. HAR and independent target validation
remain separate stages. Compiler stdout retains the actual macro expansion;
gateway observer inventory includes the generated raw API and C bridge TU.
