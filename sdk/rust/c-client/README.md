# MadB C client

This is the author-owned safe wrapper for C05's independent C provider. The
public package is `madb-c-client`; its only runtime dependency is the platform C
library delivered in the same candidate. It has no Rust package dependencies.

```rust
use madb_c_client::Client;

fn main() -> Result<(), madb_c_client::Error> {
    let client = Client::connect()?;
    assert_eq!(client.checksum(&[1, 2, 255])?, 258);
    Ok(())
}
```

`connect` calls only the fixed bootstrap ABI and compares interface and profile
identities before returning a client. It distinguishes `AbiMismatch`,
`ProfileMismatch`, `Unsupported`, and `Native { status, domain, code }`. The client
borrows byte slices only for each synchronous call; 65536 bytes is the maximum.
Empty slices return zero. No handles, cleanup operation, callbacks, or background
workers are involved. Concurrent calls with disjoint outputs are supported.

The production pipeline places these sources at `rust/madb-c-client` and adds
`src/raw.rs` from locked bindgen plus `src/identity.rs` from the accepted interface
and profile identities. The latter defines `INTERFACE_DIGEST` and `PROFILE_DIGEST`
as `[u8; 32]`. Both modules are private. The same identity bytes are injected into
the C provider at build time.

The complete candidate also contains `lib/libmadb_c.a`; `build.rs` uses only that
relative location (`../../lib` from the crate). Missing generated files or native
library fail the build. There are no fallback declarations, default identities,
or build-time dependencies on MadB's source tree or CLI.

This source template alone is intentionally incomplete. Production verification
uses the generated candidate in a clean independent Lab consumer; handwritten
bindings used for local wrapper unit checks do not count as bindgen evidence.
