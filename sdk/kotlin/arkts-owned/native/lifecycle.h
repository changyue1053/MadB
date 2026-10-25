#ifndef MADB_KN_LIFECYCLE_H
#define MADB_KN_LIFECYCLE_H
#include <stdint.h>
#include <node_api.h>
#include "executor.h"
#ifdef __cplusplus
extern "C" {
#endif

typedef struct madb_kn_lifecycle madb_kn_lifecycle;
typedef struct madb_kn_lifecycle_callbacks {
    /* Owner env, at actual async-hook entry only. No user callback. */
    int (*on_env_closing)(void *context);
    /* Owner loop after actual join. In env-closing mode: no JS/NAPI calls.
     * join_result==0 ends the native borrow BEFORE this callback. Kotlin then
     * releases its work/executor, even if its callback later fails. On nonzero
     * join_result the executor must remain valid; no destroy is permitted. */
    int (*after_join)(void *context, uint64_t generation,
                      int join_result, int env_closing);
    /* Pure query: 1=managed business/valid-env cleanup done, 0=pending,
     * negative=failure. Do not wait for TSFN finalizers blocked by this hook. */
    int (*managed_ready)(void *context, int env_closing);
} madb_kn_lifecycle_callbacks;

/* All lifecycle entrypoints except metric must run on the creating owner thread.
 * Registration/bootstrap main+identity gates precede all Kotlin CName calls.
 * create does not initialize Kotlin, create a worker, a TSFN, or a dispatcher ref.
 * On failure *out is null. Status is an errno value; exact platform statuses are
 * also retained in static metrics. Callbacks/context are borrowed until retire. */
int madb_kn_lifecycle_create(napi_env env,
                            const madb_kn_lifecycle_callbacks *callbacks,
                            void *context, madb_kn_lifecycle **out);
/* Attach before starting the worker. At most one attached executor. The Kotlin
 * owner must support retirement of a failed/not-started executor after stop. */
int madb_kn_lifecycle_attach(madb_kn_lifecycle *owner,
                            madb_kn_executor *executor, uint64_t generation);
/* Nonblocking. Requests executor stop and queues the preallocated uv_work_t.
 * No synchronous owner-thread join, detach, or per-call cleanup worker. */
int madb_kn_lifecycle_request_join(madb_kn_lifecycle *owner);
/* 0 frees owner: caller must clear its pointer. EBUSY means not yet ready.
 * Other errors retain a poisoned context; do not retry as a success path.
 * Actual env cleanup invokes this automatically after callbacks/worker drain. */
int madb_kn_lifecycle_retire(madb_kn_lifecycle *owner);

enum madb_kn_lifecycle_metric_index {
    MADB_KN_LIFE_CONTEXT_CREATED,
    MADB_KN_LIFE_CONTEXT_DESTROYED,
    MADB_KN_LIFE_CONTEXT_LIVE,
    MADB_KN_LIFE_HOOK_ADDED,
    MADB_KN_LIFE_HOOK_REMOVED,
    MADB_KN_LIFE_HOOK_LIVE,
    MADB_KN_LIFE_ENV_CLEANUP_ENTERED,
    MADB_KN_LIFE_ENV_CLEANUP_RETURNED,
    MADB_KN_LIFE_JOB_QUEUED,
    MADB_KN_LIFE_JOB_STARTED,
    MADB_KN_LIFE_JOB_FINISHED,
    MADB_KN_LIFE_AFTER_JOIN_ENTERED,
    MADB_KN_LIFE_AFTER_JOIN_RETURNED,
    MADB_KN_LIFE_JOB_LIVE,
    MADB_KN_LIFE_STOP_REQUESTS,
    MADB_KN_LIFE_EXECUTOR_ATTACHED,
    MADB_KN_LIFE_EXECUTOR_DETACHED,
    MADB_KN_LIFE_JOIN_SUCCEEDED,
    MADB_KN_LIFE_JOIN_FAILED,
    MADB_KN_LIFE_CALLBACK_FAILED,
    MADB_KN_LIFE_RESIDUAL_CONTEXTS,
    MADB_KN_LIFE_LAST_ERROR,
    MADB_KN_LIFE_LAST_NAPI_STATUS,
    MADB_KN_LIFE_LAST_UV_STATUS,
    MADB_KN_LIFE_CREATE_FAILED,
    MADB_KN_LIFE_MAIN_REJECTED,
    MADB_KN_LIFE_OWNER_REJECTED,
    MADB_KN_LIFE_METRIC_COUNT
};
/* Pure native process-cumulative counters and live gauges; no freed-context
 * dereference. Unknown index returns UINT64_MAX, never an invented zero.
 * Separate reads are not a coherent concurrent snapshot. JOIN_SUCCEEDED means
 * executor.join returned0; executor metrics distinguish never-started retirement
 * from physical pthread_join. Kotlin/TSFN responsibilities are not included. */
uint64_t madb_kn_lifecycle_metric(int index);

#ifdef __cplusplus
}
#endif
#endif
