/* C05 author-owned C provider declarations; bindgen consumes this header. */
#ifndef MADB_C_FIRST_ROUTE_H
#define MADB_C_FIRST_ROUTE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t MadbStatus;
#define MADB_OK 0
#define MADB_INVALID_ARGUMENT 1
#define MADB_INVALID_HANDLE 2
#define MADB_CLOSING 3
#define MADB_CLOSED 4
#define MADB_BUSY 5
#define MADB_TIMEOUT 6
#define MADB_WOULD_DEADLOCK 7
#define MADB_INTERNAL_FAILURE 8
#define MADB_RESOURCE_EXHAUSTED 9
#define MADB_DOMAIN_ERROR 10
#define MADB_UNSUPPORTED 11

typedef struct MadbError {
    uint32_t domain;
    int32_t code;
} MadbError;

/* Fixed, allocation-free bootstrap ABI. A valid output is exactly 64 writable
 * bytes and is cleared before version validation. Invalid output/capacity is
 * InvalidArgument; otherwise an unknown version is Unsupported. Version 1 writes
 * interface digest [0, 32), followed by profile digest [32, 64).
 * A different capacity causes no output writes. */
MadbStatus madb_c_fixture_bootstrap(uint32_t requested_version,
                                   uint8_t *out_identity, uint64_t capacity);

/* Sum at most 65536 bytes as unsigned integers, synchronously on the caller's
 * thread. data may be NULL only for length_bytes == 0. Nonempty accepted inputs
 * must remain readable through return. Outputs must be writable, aligned and
 * disjoint from one another and the input; each valid output is cleared before
 * checking the other arguments. MadbError is zero on success and argument errors.
 * No input or output pointer is retained; concurrent disjoint calls are allowed.
 * Arbitrary invalid addresses violate this contract and cannot be validated. */
MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes,
                                  uint64_t *out_sum, MadbError *out_error);

#ifdef __cplusplus
}
#endif
#endif
