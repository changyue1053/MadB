# Independent C provider

`madb_c.h` is the author-owned input for locked bindgen. `madb_c.c` implements only
stateless checksum and the allocation-free bootstrap query. Status values and
`MadbError` preserve the reviewed first-route ABI.

The production build supplies `identity.h` with two `static const uint8_t [32]`
arrays named `MADB_C_INTERFACE_DIGEST` and `MADB_C_PROFILE_DIGEST`, derived from the
accepted interface and resolved profile. No generated identity is stored here;
compiling without that header must fail. Build the C11 source with its directory
and the generated identity directory as explicit include roots, then archive it
as the candidate's `lib/libmadb_c.a`.

Bootstrap version 1 requires a non-null output and capacity exactly 64. A valid
output is cleared before the version check; an unknown version returns
`Unsupported`. Invalid output/capacity returns `InvalidArgument` without writing
an output with an invalid capacity. Success returns the interface digest followed
by the profile digest. Checksum borrows at most 65536 bytes through return, allows
null only at length zero, clears each valid output first, and sums unsigned bytes.
It keeps no state or pointers and supports concurrent calls with disjoint outputs.
