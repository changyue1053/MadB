# Rust/C values project

The template creates a natural Rust declaration and an ordinary C consumer.
`madb init --verify structure` creates and verifies the initial file inventory.
`--verify build` additionally requires the installed Rust/C recipe and its
locked tools to generate the SDK and link `madb_demo` in the final project root.
`--verify run` also executes that ordinary consumer, whose initial expected
output is `7:x` followed by a newline.

The pinned profile and toolchain in `.madb/template-input` retain their original
bytes. CMake 4.4.4 and Ninja 1.13.2 consume the exact checked SDK candidate;
the generated header's ordinary helper verifies its identity before calling
native code. The consumer records the candidate library directory in its build
RPATH and does not require `DYLD_LIBRARY_PATH`. Missing or mismatched tools are
reported before project creation. Template assets do not install tools or grant
execution authority; the selected request still requires its actual checks.
The `.madb/local-machine.json` path is reserved local state and is not created.

This finite template does not claim IDE import, debugging, Wrapper integration,
automatic regeneration after edits, or support for other host/target profiles.
