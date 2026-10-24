#include "rust_provider.h"
/* Must fail: the published length is uint64_t, not uint32_t. */
MadbStatus (*wrong_signature)(const uint8_t *, uint32_t, uint64_t *, MadbError *) =
    madb_probe_rust_checksum;
