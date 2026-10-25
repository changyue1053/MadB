#ifndef MADB_KN_THREAD_PROBE_H
#define MADB_KN_THREAD_PROBE_H
#include <stdint.h>
#include "executor.h"
#include "lifecycle.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Test profile only. No resource owner or scheduler. Snapshot is coherent under
 * the executor state mutex; thread comparisons are valid only while held. */
enum { MADB_KN_THREAD_FRAME_SIZE = 21 };
int madb_kn_executor_test_command(madb_kn_executor *, int action, uint64_t *, int capacity);
int madb_kn_executor_test_cleanup_release(madb_kn_executor *);
int madb_kn_lifecycle_test_command(madb_kn_lifecycle *, int action, uint64_t *, int capacity);
#ifdef __cplusplus
}
#endif
#endif
