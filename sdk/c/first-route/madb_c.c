/* C05 stateless provider. The pipeline must supply the matching identity.h. */
#include "madb_c.h"
#include "identity.h"

#include <stddef.h>
#include <string.h>

_Static_assert(sizeof(MadbStatus) == 4, "MadbStatus must be int32_t");
_Static_assert(sizeof(MadbError) == 8, "MadbError layout changed");
_Static_assert(_Alignof(MadbError) == 4, "MadbError alignment changed");
_Static_assert(offsetof(MadbError, code) == 4, "MadbError.code offset changed");
_Static_assert(sizeof(MADB_C_INTERFACE_DIGEST) == 32, "interface digest must be 32 bytes");
_Static_assert(sizeof(MADB_C_PROFILE_DIGEST) == 32, "profile digest must be 32 bytes");

MadbStatus madb_c_fixture_bootstrap(uint32_t requested_version,
                                   uint8_t *out_identity, uint64_t capacity) {
    if (out_identity == NULL || capacity != 64) {
        return MADB_INVALID_ARGUMENT;
    }
    memset(out_identity, 0, 64);
    if (requested_version != 1) {
        return MADB_UNSUPPORTED;
    }
    memcpy(out_identity, MADB_C_INTERFACE_DIGEST, 32);
    memcpy(out_identity + 32, MADB_C_PROFILE_DIGEST, 32);
    return MADB_OK;
}

MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes,
                                  uint64_t *out_sum, MadbError *out_error) {
    /* Null/alignment can be checked; allocation validity and aliasing cannot.
     * Clear each valid output independently, even if the other one is invalid. */
    const int sum_valid = out_sum != NULL && (uintptr_t)out_sum % _Alignof(uint64_t) == 0;
    const int error_valid = out_error != NULL && (uintptr_t)out_error % _Alignof(MadbError) == 0;
    if (sum_valid) {
        *out_sum = 0;
    }
    if (error_valid) {
        *out_error = (MadbError){0, 0};
    }
    if (!sum_valid || !error_valid || length_bytes > 65536 ||
        length_bytes > SIZE_MAX || (length_bytes != 0 && data == NULL)) {
        return MADB_INVALID_ARGUMENT;
    }
    uint64_t sum = 0;
    for (size_t index = 0; index < (size_t)length_bytes; ++index) {
        sum += data[index];
    }
    *out_sum = sum;
    return MADB_OK;
}
