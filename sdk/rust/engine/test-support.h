/* C05 fixture controls, available only in the separately identified test-support
 * build. Include the candidate's generated engine header before this file.
 * These declarations are not part of the default Release SDK export set. */
#ifndef MADB_FIRST_ROUTE_TEST_SUPPORT_H
#define MADB_FIRST_ROUTE_TEST_SUPPORT_H
#ifdef __cplusplus
extern "C" {
#endif
typedef struct MadbTestCounts {
    uint64_t engines_created;
    uint64_t engines_released;
    uint64_t subscriptions_created;
    uint64_t subscriptions_released;
    uint64_t active_operations;
    uint64_t source_leases;
    uint64_t callbacks_started;
    uint64_t callbacks_finished;
    uint64_t registry_entries;
    uint64_t diagnostic_failures;
} MadbTestCounts;
/* Point 1: checksum permit acquired, before input access.
 * Point 2: source has callback/context and lease, before callback entry.
 * arm -> start worker -> wait -> assert/close -> resume -> join. */
MadbStatus madb_fixture_test_arm(uint32_t point, MadbError *out_error);
MadbStatus madb_fixture_test_wait(uint32_t point, uint32_t timeout_ms, MadbError *out_error);
MadbStatus madb_fixture_test_resume(uint32_t point, MadbError *out_error);
MadbStatus madb_fixture_test_snapshot(MadbTestCounts *out_counts, MadbError *out_error);
/* Synchronous serial-source delivery. Already admitted delivery may finish after
 * request_close; await_closed cannot succeed until its lease is returned. */
MadbStatus madb_fixture_test_emit(MadbHandle subscription, uint32_t state, MadbError *out_error);
/* 1: next registration fails after insertion, before callback; 2: next checksum
 * panics after admission; 3: diagnostic sink panics (persistent, isolated). */
MadbStatus madb_fixture_test_set_fault(uint32_t fault, uint32_t enabled, MadbError *out_error);
/* Only an empty registry accepts monotonic advancement. Use UINT64_MAX in a
 * separate process/end-of-run case; exhausted identities cannot be reset. */
MadbStatus madb_fixture_test_set_next_token(uint64_t next, MadbError *out_error);
#ifdef __cplusplus
}
#endif
#endif
