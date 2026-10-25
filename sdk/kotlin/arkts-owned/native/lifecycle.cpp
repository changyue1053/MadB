#include "lifecycle.h"
#include "main_gate.h"
#include <uv.h>
#include <pthread.h>
#include <atomic>
#include <cerrno>
#include <new>

struct madb_kn_lifecycle {
    // Owner-thread fields. workResult is written by work and read only after
    // libuv's work->after-work handoff. Metrics are independently atomic.
    napi_env env;
    uv_loop_t *loop = nullptr;
    napi_async_cleanup_hook_handle hook = nullptr;
    pthread_t ownerThread;
    madb_kn_lifecycle_callbacks callbacks;
    void *context;
    uv_work_t work{};
    madb_kn_executor *executor = nullptr; // Borrow only: Kotlin remains its owner.
    uint64_t generation = 0;
    int workResult = EBUSY;
    unsigned callbackDepth = 0;
    bool jobActive = false;
    bool envClosing = false;
    bool poisoned = false;
    explicit madb_kn_lifecycle(napi_env e,
                              const madb_kn_lifecycle_callbacks &c, void *data)
        : env(e), ownerThread(pthread_self()), callbacks(c), context(data) {
        work.data = this;
    }
};
namespace {
std::atomic<uint64_t> metrics[MADB_KN_LIFE_METRIC_COUNT]{};
void increment(int field) noexcept { metrics[field].fetch_add(1, std::memory_order_relaxed); }
void decrement(int field) noexcept { metrics[field].fetch_sub(1, std::memory_order_relaxed); }
struct ReturnMarker {
    int field;
    ~ReturnMarker() { increment(field); }
};
void recordError(madb_kn_lifecycle *owner, int error) noexcept {
    metrics[MADB_KN_LIFE_LAST_ERROR].store(static_cast<uint64_t>(error), std::memory_order_relaxed);
    if (owner && !owner->poisoned) {
        owner->poisoned = true;
        increment(MADB_KN_LIFE_RESIDUAL_CONTEXTS);
    }
}
bool onOwner(const madb_kn_lifecycle *owner) noexcept {
    if (!owner) return false;
    if (pthread_equal(pthread_self(), owner->ownerThread)) return true;
    increment(MADB_KN_LIFE_OWNER_REJECTED);
    return false;
}
void callbackError(madb_kn_lifecycle *owner) noexcept {
    increment(MADB_KN_LIFE_CALLBACK_FAILED);
    recordError(owner, EIO);
}
int callGate(madb_kn_lifecycle *owner) noexcept {
    ++owner->callbackDepth;
    int result = EIO;
    try { result = owner->callbacks.on_env_closing(owner->context); }
    catch (...) { result = EIO; }
    --owner->callbackDepth;
    if (result != 0) callbackError(owner);
    return result;
}
int callJoined(madb_kn_lifecycle *owner, uint64_t generation, int joinResult) noexcept {
    ++owner->callbackDepth;
    int result = EIO;
    try {
        result = owner->callbacks.after_join(owner->context, generation,
                                            joinResult, owner->envClosing ? 1 : 0);
    } catch (...) { result = EIO; }
    --owner->callbackDepth;
    if (result != 0) callbackError(owner);
    return result;
}
int callReady(madb_kn_lifecycle *owner) noexcept {
    ++owner->callbackDepth;
    int result = -1;
    try { result = owner->callbacks.managed_ready(owner->context, owner->envClosing ? 1 : 0); }
    catch (...) { result = -1; }
    --owner->callbackDepth;
    if (result != 0 && result != 1) { callbackError(owner); return -1; }
    return result;
}
void joinWork(uv_work_t *work) noexcept {
    auto *owner = static_cast<madb_kn_lifecycle *>(work->data);
    increment(MADB_KN_LIFE_JOB_STARTED);
    // The executor independently rejects owner-thread/self join.
    owner->workResult = madb_kn_executor_join(owner->executor);
    increment(MADB_KN_LIFE_JOB_FINISHED);
}
void afterJoin(uv_work_t *work, int status) noexcept {
    auto *owner = static_cast<madb_kn_lifecycle *>(work->data);
    increment(MADB_KN_LIFE_AFTER_JOIN_ENTERED);
    ReturnMarker returned{MADB_KN_LIFE_AFTER_JOIN_RETURNED};
    // Never retry a job or free a borrowed executor after a malformed callback.
    if (!onOwner(owner) || !owner->jobActive) {
        recordError(owner, EIO);
        return;
    }
    owner->jobActive = false;
    decrement(MADB_KN_LIFE_JOB_LIVE);
    const int result = status == 0 ? owner->workResult : EIO;
    if (status != 0) metrics[MADB_KN_LIFE_LAST_UV_STATUS].store(static_cast<uint64_t>(-static_cast<int64_t>(status)), std::memory_order_relaxed);
    if (result == 0) increment(MADB_KN_LIFE_JOIN_SUCCEEDED);
    else { increment(MADB_KN_LIFE_JOIN_FAILED); recordError(owner, result); }
    const uint64_t generation = owner->generation;
    if (result == 0) {
        // A successful join ends the native borrow BEFORE calling Kotlin.
        // Kotlin may destroy the executor and then fail/throw. An eventual
        // env-cleanup hook must never dereference that former borrow again.
        owner->executor = nullptr;
        owner->generation = 0;
        increment(MADB_KN_LIFE_EXECUTOR_DETACHED);
    }
    (void)callJoined(owner, generation, result);
    if (owner->envClosing && !owner->poisoned) {
        // Retire may free owner. There is no access after this call.
        (void)madb_kn_lifecycle_retire(owner);
    }
}
int queueJoin(madb_kn_lifecycle *owner) noexcept {
    if (!owner->executor || owner->jobActive) return 0;
    increment(MADB_KN_LIFE_STOP_REQUESTS);
    const int stop = madb_kn_executor_request_stop(owner->executor);
    if (stop != 0) { recordError(owner, stop); return stop; }
    owner->workResult = EBUSY;
    owner->jobActive = true;
    // The uv_work_t belongs to Runtime admission, not the business quota.
    const int status = uv_queue_work(owner->loop, &owner->work, joinWork, afterJoin);
    if (status != 0) {
        owner->jobActive = false;
        metrics[MADB_KN_LIFE_LAST_UV_STATUS].store(static_cast<uint64_t>(-static_cast<int64_t>(status)), std::memory_order_relaxed);
        recordError(owner, EIO);
        return EIO;
    }
    increment(MADB_KN_LIFE_JOB_QUEUED);
    increment(MADB_KN_LIFE_JOB_LIVE);
    return 0;
}
void envCleanup(napi_async_cleanup_hook_handle handle, void *data) noexcept {
    auto *owner = static_cast<madb_kn_lifecycle *>(data);
    increment(MADB_KN_LIFE_ENV_CLEANUP_ENTERED);
    ReturnMarker returned{MADB_KN_LIFE_ENV_CLEANUP_RETURNED};
    if (!onOwner(owner) || owner->envClosing || handle != owner->hook) {
        recordError(owner, EIO);
        return;
    }
    owner->envClosing = true;
    (void)callGate(owner);
    // Even a failed managed gate must not leave a running thread unchecked.
    // Existing residuals still prevent successful retirement.
    (void)queueJoin(owner);
    if (!owner->poisoned && !owner->jobActive) {
        // No worker case. Callback has returned; removal/free does not join.
        (void)madb_kn_lifecycle_retire(owner);
    }
}
}

extern "C" int madb_kn_lifecycle_create(
    napi_env env, const madb_kn_lifecycle_callbacks *callbacks,
    void *context, madb_kn_lifecycle **out) {
    if (!out) return EINVAL;
    *out = nullptr;
    if (!env || !callbacks || !callbacks->on_env_closing ||
        !callbacks->after_join || !callbacks->managed_ready) return EINVAL;
    if (!madb_kn_is_main_thread()) { increment(MADB_KN_LIFE_MAIN_REJECTED); return EPERM; }
    auto *owner = new(std::nothrow) madb_kn_lifecycle(env, *callbacks, context);
    if (!owner) { increment(MADB_KN_LIFE_CREATE_FAILED); return ENOMEM; }
    napi_status status = napi_get_uv_event_loop(env, &owner->loop);
    if (status == napi_ok && owner->loop) {
        status = napi_add_async_cleanup_hook(env, envCleanup, owner, &owner->hook);
    } else if (status == napi_ok) { status = napi_generic_failure; }
    if (status != napi_ok) {
        metrics[MADB_KN_LIFE_LAST_NAPI_STATUS].store(status, std::memory_order_relaxed);
        increment(MADB_KN_LIFE_CREATE_FAILED);
        delete owner;
        return EIO;
    }
    // Successful add owns this context until successful remove. No callbacks,
    // business thread, or Kotlin initialization occur during admission here.
    increment(MADB_KN_LIFE_CONTEXT_CREATED);
    increment(MADB_KN_LIFE_CONTEXT_LIVE);
    increment(MADB_KN_LIFE_HOOK_ADDED);
    increment(MADB_KN_LIFE_HOOK_LIVE);
    *out = owner;
    return 0;
}
extern "C" int madb_kn_lifecycle_attach(madb_kn_lifecycle *owner,
                                        madb_kn_executor *executor, uint64_t generation) {
    if (!onOwner(owner) || !executor || !generation) return EINVAL;
    if (owner->poisoned) return EIO;
    if (owner->envClosing || owner->executor || owner->jobActive || owner->callbackDepth) return EBUSY;
    owner->executor = executor;
    owner->generation = generation;
    increment(MADB_KN_LIFE_EXECUTOR_ATTACHED);
    return 0;
}
extern "C" int madb_kn_lifecycle_request_join(madb_kn_lifecycle *owner) {
    if (!onOwner(owner)) return EINVAL;
    if (owner->poisoned) return EIO;
    if (owner->callbackDepth) return EBUSY;
    return queueJoin(owner);
}
extern "C" int madb_kn_lifecycle_retire(madb_kn_lifecycle *owner) {
    if (!onOwner(owner)) return EINVAL;
    if (owner->poisoned) return EIO;
    if (owner->callbackDepth || owner->jobActive || owner->executor) return EBUSY;
    const int ready = callReady(owner);
    if (ready < 0 || owner->poisoned) return EIO;
    if (!ready) return EBUSY;
    const napi_status status = napi_remove_async_cleanup_hook(owner->hook);
    if (status != napi_ok) {
        metrics[MADB_KN_LIFE_LAST_NAPI_STATUS].store(status, std::memory_order_relaxed);
        recordError(owner, EIO);
        return EIO;
    }
    owner->hook = nullptr;
    increment(MADB_KN_LIFE_HOOK_REMOVED);
    decrement(MADB_KN_LIFE_HOOK_LIVE);
    delete owner;
    increment(MADB_KN_LIFE_CONTEXT_DESTROYED);
    decrement(MADB_KN_LIFE_CONTEXT_LIVE);
    return 0;
}
extern "C" uint64_t madb_kn_lifecycle_metric(int index) {
    if (index < 0 || index >= MADB_KN_LIFE_METRIC_COUNT) return UINT64_MAX;
    return metrics[index].load(std::memory_order_relaxed);
}
