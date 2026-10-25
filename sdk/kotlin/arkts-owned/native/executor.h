#ifndef MADB_KN_EXECUTOR_H
#define MADB_KN_EXECUTOR_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Executor primitives only. Kotlin owns all work queues, values and decisions. */
typedef struct madb_kn_executor madb_kn_executor;
madb_kn_executor *madb_kn_executor_create(uint64_t generation);
int madb_kn_executor_start(madb_kn_executor *executor);
int madb_kn_executor_wake(madb_kn_executor *executor);
int madb_kn_executor_request_stop(madb_kn_executor *executor);
/* Must run in the reserved non-env cleanup executor. No detach fallback. */
int madb_kn_executor_join(madb_kn_executor *executor);
int madb_kn_executor_destroy(madb_kn_executor *executor);
uint64_t madb_kn_executor_metric(madb_kn_executor *executor, int index);
/* Actual Kotlin CName bridge. 1=one unit done, 0=idle, -1=stop, -2=failure. */
int madb_kn_business_step(uint64_t generation);
#ifdef __cplusplus
}
#endif
#endif
