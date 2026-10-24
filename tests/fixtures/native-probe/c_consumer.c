/* Independent C program: public generated header + Rust library only. */
#include "rust_provider.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(MadbError) == 8, "error size");
_Static_assert(_Alignof(MadbError) == 4, "error alignment");
_Static_assert(offsetof(MadbError, code) == 4, "error code offset");
_Static_assert(sizeof(MadbProbeLayout) == 32, "probe size");
_Static_assert(_Alignof(MadbProbeLayout) == 8, "probe alignment");
_Static_assert(offsetof(MadbProbeLayout, count) == 8, "count offset");
_Static_assert(offsetof(MadbProbeLayout, data) == 16, "pointer offset");
_Static_assert(offsetof(MadbProbeLayout, length_bytes) == 24, "length offset");

int main(void) {
    const uint8_t bytes[] = {255, 0, 1, 128};
    uint64_t sum = 99;
    MadbError error = {99, 99};
    assert(madb_probe_rust_echo(UINT64_MAX) == UINT64_MAX);
    assert(madb_probe_rust_echo(UINT64_C(9007199254740993)) == UINT64_C(9007199254740993));
    assert(madb_probe_rust_checksum(bytes, sizeof bytes, &sum, &error) == MADB_OK);
    assert(sum == 384 && error.domain == 0 && error.code == 0);
    assert(madb_probe_rust_checksum(NULL, 0, &sum, &error) == MADB_OK && sum == 0);
    assert(madb_probe_rust_checksum(bytes, 0, &sum, &error) == MADB_OK && sum == 0);
    sum = 99; error = (MadbError){99, 99};
    assert(madb_probe_rust_checksum(NULL, 1, &sum, &error) == MADB_INVALID_ARGUMENT);
    assert(sum == 0 && error.domain == 0 && error.code == 0);
    sum = 99;
    assert(madb_probe_rust_checksum(bytes, UINT64_MAX, &sum, &error) == MADB_INVALID_ARGUMENT && sum == 0);
    assert(madb_probe_rust_checksum(bytes, 1, NULL, &error) == MADB_INVALID_ARGUMENT);
    sum = 99;
    assert(madb_probe_rust_checksum(bytes, 1, &sum, NULL) == MADB_INVALID_ARGUMENT && sum == 0);
    uint8_t maximum[65536];
    memset(maximum, 255, sizeof maximum);
    assert(madb_probe_rust_checksum(maximum, sizeof maximum, &sum, &error) == MADB_OK);
    assert(sum == UINT64_C(16711680));
    puts("C_TO_RUST_OK cases=10");
    return 0;
}
