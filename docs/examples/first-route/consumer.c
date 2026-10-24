/* Compile-only signature consumer. It neither links nor certifies the runtime. */
#include "expected.h"
#include <stddef.h>
_Static_assert(sizeof(MadbError) == 8, "MadbError size");
_Static_assert(_Alignof(MadbError) == 4, "MadbError alignment");
_Static_assert(offsetof(MadbError, code) == 4, "MadbError code offset");
_Static_assert(sizeof(MadbHandle) == 8, "handle width");
_Static_assert(MADB_OK == 0 && MADB_INVALID_ARGUMENT == 1 && MADB_INVALID_HANDLE == 2,
               "stable argument and handle codes");
_Static_assert(MADB_CLOSING == 3 && MADB_CLOSED == 4 && MADB_BUSY == 5 && MADB_TIMEOUT == 6,
               "stable close codes");
_Static_assert(MADB_WOULD_DEADLOCK == 7 && MADB_INTERNAL_FAILURE == 8 &&
               MADB_RESOURCE_EXHAUSTED == 9 && MADB_DOMAIN_ERROR == 10 && MADB_UNSUPPORTED == 11,
               "stable failure codes");
static void on_state(void *context, uint64_t sequence, uint32_t state) {
    (void)context; (void)sequence; (void)state;
}
MadbStatus check_signatures(MadbHandle handle) {
    MadbError error = {0, 0};
    MadbHandle subscription = 0;
    uint64_t sum = 0;
    MadbStatus status = madb_fixture_create(&handle, &error);
    status = madb_fixture_checksum(handle, NULL, 0, &sum, &error);
    status = madb_fixture_observe(handle, on_state, NULL, &subscription, &error);
    status = madb_fixture_request_close(subscription, &error);
    status = madb_fixture_await_closed(subscription, 0, &error);
    status = madb_fixture_release_handle(subscription, &error);
    status = madb_c_fixture_checksum(NULL, 0, &sum, &error);
    return status;
}
