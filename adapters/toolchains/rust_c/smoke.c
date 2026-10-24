/* C05 package-build smoke; independent exhaustive consumers remain in MadB-Lab. */
#if defined(MADB_PROVIDER_RUST)
#include "madb_rust.h"
#define BOOTSTRAP madb_fixture_bootstrap
#define PROVIDER "rust"
#elif defined(MADB_PROVIDER_C)
#include "madb_c.h"
#define BOOTSTRAP madb_c_fixture_bootstrap
#define PROVIDER "c"
#else
#error "The worker must select exactly one provider"
#endif
#include "expected_identity.h"
#include <stdio.h>
#include <string.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "native smoke failed at line %d: %s\n", __LINE__, #condition); \
        return 1; \
    } \
} while (0)

int main(void) {
    uint8_t identity[64];
    memset(identity, 0xff, sizeof(identity));
    CHECK(BOOTSTRAP(1, identity, sizeof(identity)) == MADB_OK);
    CHECK(memcmp(identity, EXPECTED_INTERFACE, 32) == 0);
    CHECK(memcmp(identity + 32, EXPECTED_PROFILE, 32) == 0);
    CHECK(BOOTSTRAP(2, identity, sizeof(identity)) == MADB_UNSUPPORTED);
    for (size_t index = 0; index < sizeof(identity); ++index) {
        CHECK(identity[index] == 0);
    }
    CHECK(BOOTSTRAP(1, identity, sizeof(identity) - 1) == MADB_INVALID_ARGUMENT);
    uint64_t sum = UINT64_C(999);
    MadbError error = {123, -123};
    const uint8_t input[] = {0, 1, 128, 255};
#if defined(MADB_PROVIDER_RUST)
    MadbHandle engine = 0;
    CHECK(madb_fixture_create(&engine, &error) == MADB_OK);
    CHECK(engine != 0 && error.domain == 0 && error.code == 0);
    CHECK(madb_fixture_checksum(engine, input, sizeof(input), &sum, &error) == MADB_OK);
#else
    CHECK(madb_c_fixture_checksum(input, sizeof(input), &sum, &error) == MADB_OK);
#endif
    CHECK(sum == 384 && error.domain == 0 && error.code == 0);
#if defined(MADB_PROVIDER_RUST)
    CHECK(madb_fixture_checksum(engine, NULL, 0, &sum, &error) == MADB_OK);
#else
    CHECK(madb_c_fixture_checksum(NULL, 0, &sum, &error) == MADB_OK);
#endif
    CHECK(sum == 0 && error.domain == 0 && error.code == 0);
    sum = UINT64_C(999);
    error = (MadbError){123, -123};
#if defined(MADB_PROVIDER_RUST)
    CHECK(madb_fixture_checksum(engine, input, 65537, &sum, &error) == MADB_INVALID_ARGUMENT);
#else
    CHECK(madb_c_fixture_checksum(input, 65537, &sum, &error) == MADB_INVALID_ARGUMENT);
#endif
    CHECK(sum == 0 && error.domain == 0 && error.code == 0);
#if defined(MADB_PROVIDER_RUST)
    CHECK(madb_fixture_request_close(engine, &error) == MADB_OK);
    CHECK(madb_fixture_request_close(engine, &error) == MADB_OK);
    CHECK(madb_fixture_await_closed(engine, 1000, &error) == MADB_OK);
    CHECK(madb_fixture_release_handle(engine, &error) == MADB_OK);
    CHECK(error.domain == 0 && error.code == 0);
#endif
    puts("MADB_NATIVE_SMOKE_OK provider=" PROVIDER);
    return 0;
}
