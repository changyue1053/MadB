# Rust/C values project

The installed template creates a natural Rust declaration and an ordinary C
consumer. `madb init --verify structure` verifies the complete initial file
inventory. It does not compile the SDK or run the C consumer.

The pinned profile and toolchain in `.madb/template-input` retain their original
bytes. Generation, CMake/Ninja configuration, linking, and running require the
matching installed recipe and independently observed tools; this installation
rejects `--verify build` and `--verify run` until that execution path is available.
The `.madb/local-machine.json` path is reserved local state and is not created.
