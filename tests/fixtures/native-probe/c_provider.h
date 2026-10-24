#ifndef MADB_C03_C_PROVIDER_H
#define MADB_C03_C_PROVIDER_H
#include <stdint.h>
typedef int32_t MadbStatus;
#define MADB_OK 0
#define MADB_INVALID_ARGUMENT 1
typedef struct MadbError { uint32_t domain; int32_t code; } MadbError;
typedef struct MadbProbeLayout {
    uint8_t tag;
    uint64_t count;
    const uint8_t *data;
    uint64_t length_bytes;
} MadbProbeLayout;
/* Same bounded, synchronous borrow/output obligations as rust_provider.rs.
 * This symbol implements the independent C provider from the C02 ABI shape. */
MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes,
                                  uint64_t *out_sum, MadbError *out_error);
uint64_t madb_probe_c_echo(uint64_t value);
#endif
