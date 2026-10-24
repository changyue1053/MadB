#include "c_provider.h"
#include <stddef.h>

uint64_t madb_probe_c_echo(uint64_t value) { return value; }

MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes,
                                  uint64_t *out_sum, MadbError *out_error) {
    const int sum_valid = out_sum != NULL && (uintptr_t)out_sum % _Alignof(uint64_t) == 0;
    const int error_valid = out_error != NULL && (uintptr_t)out_error % _Alignof(MadbError) == 0;
    if (sum_valid) *out_sum = 0;
    if (error_valid) *out_error = (MadbError){0, 0};
    if (!sum_valid || !error_valid || length_bytes > UINT64_C(65536)) return MADB_INVALID_ARGUMENT;
    if (length_bytes == 0) return MADB_OK;
    if (data == NULL || length_bytes > SIZE_MAX) return MADB_INVALID_ARGUMENT;
    uint64_t sum = 0;
    for (size_t index = 0; index < (size_t)length_bytes; ++index) sum += data[index];
    *out_sum = sum;
    return MADB_OK;
}
