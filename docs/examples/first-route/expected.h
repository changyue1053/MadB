/* C02 reviewed ABI shape. Declarations only; C03/C05 provide generated code and implementation. */
#ifndef MADB_FIXTURE_EXPECTED_H
#define MADB_FIXTURE_EXPECTED_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t MadbStatus;
typedef uint64_t MadbHandle; /* 0 invalid; SDK-instance registry token, never a pointer. */
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

typedef struct MadbError { uint32_t domain; int32_t code; } MadbError;
/* Domain/code are zero for success and lifecycle failures; nonzero domain only for DOMAIN_ERROR. */

MadbStatus madb_fixture_create(MadbHandle *out_engine, MadbError *out_error);
MadbStatus madb_fixture_checksum(MadbHandle engine, const uint8_t *data, uint64_t length_bytes,
                                uint64_t *out_sum, MadbError *out_error);
MadbStatus madb_fixture_request_close(MadbHandle engine, MadbError *out_error);
MadbStatus madb_fixture_await_closed(MadbHandle engine, uint32_t timeout_ms, MadbError *out_error);
MadbStatus madb_fixture_release_handle(MadbHandle engine, MadbError *out_error);

/* Observer context is borrowed and leased until await_closed succeeds on the subscription.
 * A callback can run synchronously before observe returns. Never hold the registry lock
 * while invoking it. No foreign exception/panic may unwind through this function pointer. */
typedef void (*MadbStateCallback)(void *context, uint64_t sequence, uint32_t state);
MadbStatus madb_fixture_observe(MadbHandle engine, MadbStateCallback callback, void *context,
                               MadbHandle *out_subscription, MadbError *out_error);
/* request/await/release above also accept subscription handles; checksum/observe require engine. */

/* Independent C provider consumed by Rust; this is not the observer callback path. */
MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes,
                                  uint64_t *out_sum, MadbError *out_error);
#ifdef __cplusplus
}
#endif
#endif
