#define MADB_NAPI_OBSERVER_NO_WRAPPERS
#include "observer.hpp"

namespace gateway::napi_observer {
namespace {
thread_local Scope *current = nullptr;
std::array<std::atomic<uint64_t>, Count - LocalCount> process{};
void increment(std::atomic<uint64_t> &counter) noexcept {
    auto value = counter.load();
    while (value < DiagnosticOverflow && !counter.compare_exchange_weak(value, value + 1)) {}
}
void mark(State *s, Field f) noexcept { if (s) increment(s->counters[f]); }
void global(Field f) noexcept { increment(process[f - LocalCount]); }
bool cleanup(Mode m) noexcept { return m == Mode::Cleanup || m == Mode::CleanupAfterJoin; }
}
void State::reset(napi_env e, std::thread::id t) noexcept {
    env = e;
    thread = t;
    for (auto &n : counters) n = 0;
    reference = nullptr;
    tsfn = nullptr;
    hook = nullptr;
    counters[Format] = 1;
}
LocalSnapshot State::localSnapshot() const noexcept {
    LocalSnapshot result{};
    for (size_t i = 0; i < LocalCount; ++i) result[i] = counters[i].load();
    return result;
}
ProcessSnapshot processSnapshot() noexcept {
    ProcessSnapshot result{};
    for (size_t i = 0; i < result.size(); ++i) result[i] = process[i].load();
    return result;
}
Snapshot State::snapshot() const noexcept {
    Snapshot result{};
    const auto local = localSnapshot();
    const auto global = processSnapshot();
    for (size_t i = 0; i < LocalCount; ++i) result[i] = local[i];
    for (size_t i = LocalCount; i < Count; ++i) result[i] = global[i - LocalCount];
    return result;
}
Scope::Scope(Mode mode, napi_env env, State *state) noexcept
    : mode_(mode), env_(env), thread_(std::this_thread::get_id()), state_(state), previous_(current) {
    current = this;
    if (!env && mode == Mode::Wake) mark(state, NullWakeEntered);
    if (!env && mode == Mode::Finalizer) mark(state, NullFinalizerEntered);
}
Scope::~Scope() { current = previous_; }
State *bound() noexcept { return current ? current->state_ : nullptr; }
Binding::Binding(State &state) noexcept
    : scope_(current), previous_(current ? current->state_ : nullptr) {
    if (scope_) scope_->state_ = &state;
}
Binding::~Binding() { if (scope_) scope_->state_ = previous_; }
void wakeDestroyed(State &state) noexcept {
    if (current && current->mode_ == Mode::Wake && !current->env_ && current->state_ == &state)
        mark(&state, NullWakeDestroyed);
}
void attempt(Kind kind, napi_env env, const void *handle) noexcept {
    global(GlobalAttempts);
    State *s = bound();
    mark(s, Attempts);
    if (!current) {
        global(GlobalUnscopedAttempts);
        global(GlobalIllegalAttempts);
        return;
    }
    const Mode mode = current->mode_;
    bool allowed = false, threadBad = false, envBad = false, ownerBad = false;
    const bool callback = mode == Mode::Entry || (mode == Mode::Wake && current->env_);
    const bool ownerMode = callback || cleanup(mode) || mode == Mode::OwnerLoop;
    const bool envKind = kind == Kind::Env || kind == Kind::RefCreate || kind == Kind::RefRead ||
                         kind == Kind::RefDelete || kind == Kind::TsfnCreate || kind == Kind::HookAdd;
    if (envKind) {
        allowed = callback || (kind == Kind::RefDelete &&
                               (mode == Mode::Cleanup || mode == Mode::OwnerLoop));
        threadBad = std::this_thread::get_id() != current->thread_ ||
                    (s && std::this_thread::get_id() != s->thread);
        envBad = !env || env != current->env_ || (s && env != s->env);
        if (kind != Kind::Env) ownerBad = !s;
        if (s && (kind == Kind::RefRead || kind == Kind::RefDelete))
            ownerBad = !handle || handle != s->reference.load();
        if (s && kind == Kind::RefCreate) ownerBad = s->reference.load() != nullptr;
        if (s && kind == Kind::TsfnCreate) ownerBad = s->tsfn.load() != nullptr;
        if (s && kind == Kind::HookAdd) ownerBad = s->hook.load() != nullptr;
    } else if (kind == Kind::TsfnCall || kind == Kind::TsfnRelease) {
        allowed = ownerMode || (kind == Kind::TsfnCall && mode == Mode::Worker);
        ownerBad = !s || !handle || handle != s->tsfn.load();
        threadBad = ownerMode && (!s || std::this_thread::get_id() != s->thread);
        // Enqueue/release have no napi_env parameter. The owned TSFN is their authority.
        // This gateway releases only from its owner callback/cleanup, never from its worker.
    } else if (kind == Kind::HookRemove) {
        allowed = ownerMode;
        threadBad = !s || std::this_thread::get_id() != s->thread;
        ownerBad = !s || !handle || handle != s->hook.load();
    } else if (kind == Kind::Module) {
        allowed = mode == Mode::Module;
        global(GlobalModuleRegistrations);
    }
    if (threadBad) mark(s, WrongThread);
    if (envBad) mark(s, WrongEnv);
    if (ownerBad) mark(s, InvalidOwner);
    if (!allowed || threadBad || envBad || ownerBad) {
        mark(s, IllegalAttempts);
        global(GlobalIllegalAttempts);
    }
}
napi_status createReference(napi_env e, napi_value v, uint32_t count, napi_ref *out) {
    attempt(Kind::RefCreate, e);
    State *s = bound();
    const auto status = napi_create_reference(e, v, count, out);
    if (status == napi_ok) global(GlobalRefCreated);
    if (status == napi_ok && s && out) s->reference = *out;
    return status;
}
napi_status readReference(napi_env e, napi_ref ref, napi_value *out) {
    attempt(Kind::RefRead, e, ref);
    return napi_get_reference_value(e, ref, out);
}
napi_status deleteReference(napi_env e, napi_ref ref) {
    attempt(Kind::RefDelete, e, ref);
    State *s = bound();
    const bool inCleanup = current && cleanup(current->mode());
    const auto status = napi_delete_reference(e, ref);
    if (status == napi_ok) global(GlobalRefDeleted);
    if (status == napi_ok && s) {
        s->reference = nullptr;
        if (inCleanup) mark(s, CleanupRefDeleted);
    }
    return status;
}
napi_status createTsfn(napi_env e, napi_value fn, napi_value resource, napi_value name,
                       size_t queue, size_t threads, void *data, napi_finalize finalize,
                       void *context, napi_threadsafe_function_call_js js,
                       napi_threadsafe_function *out) {
    attempt(Kind::TsfnCreate, e);
    State *s = bound();
    const auto status = napi_create_threadsafe_function(e, fn, resource, name, queue, threads,
                                                       data, finalize, context, js, out);
    if (status == napi_ok) global(GlobalTsfnCreated);
    if (status == napi_ok && s && out) s->tsfn = *out;
    return status;
}
napi_status callTsfn(napi_threadsafe_function fn, void *data, napi_threadsafe_function_call_mode mode) {
    attempt(Kind::TsfnCall, nullptr, fn);
    return napi_call_threadsafe_function(fn, data, mode);
}
napi_status releaseTsfn(napi_threadsafe_function fn, napi_threadsafe_function_release_mode mode) {
    attempt(Kind::TsfnRelease, nullptr, fn);
    State *s = bound();
    const bool inCleanup = current && cleanup(current->mode());
    const auto status = napi_release_threadsafe_function(fn, mode);
    if (status == napi_ok) global(GlobalTsfnReleased);
    if (status == napi_ok && s) {
        s->tsfn = nullptr;
        if (inCleanup) mark(s, CleanupTsfnReleased);
    }
    return status;
}
napi_status addHook(napi_env e, napi_async_cleanup_hook hook, void *data,
                    napi_async_cleanup_hook_handle *out) {
    attempt(Kind::HookAdd, e);
    State *s = bound();
    const auto status = napi_add_async_cleanup_hook(e, hook, data, out);
    if (status == napi_ok) global(GlobalHookAdded);
    if (status == napi_ok && s && out) s->hook = *out;
    return status;
}
napi_status removeHook(napi_async_cleanup_hook_handle hook) {
    attempt(Kind::HookRemove, nullptr, hook);
    State *s = bound();
    const bool inCleanup = current && cleanup(current->mode());
    const auto status = napi_remove_async_cleanup_hook(hook);
    if (status == napi_ok) global(GlobalHookRemoved);
    if (status == napi_ok && s) {
        s->hook = nullptr;
        if (inCleanup) mark(s, CleanupHookRemoved);
    }
    return status;
}
void registerModule(napi_module *module) {
    attempt(Kind::Module);
    napi_module_register(module);
}
} // namespace gateway::napi_observer
