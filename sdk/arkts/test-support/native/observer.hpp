#pragma once
// C07 companion only. Observe calls; never grant production permissions or skip a real call.
#include <array>
#include <atomic>
#include <cstdint>
#include <napi/native_api.h>
#include <thread>
#include <utility>

namespace gateway::napi_observer {
enum class Mode { Entry, Module, Wake, Cleanup, CleanupAfterJoin, OwnerLoop, Worker, JoinWork, Finalizer };
enum class Kind { Env, RefCreate, RefRead, RefDelete, TsfnCreate, TsfnCall, TsfnRelease, HookAdd, HookRemove, Module };
enum Field : size_t {
    Format, Attempts, IllegalAttempts, WrongThread, WrongEnv, InvalidOwner,
    NullWakeEntered, NullWakeDestroyed, NullFinalizerEntered,
    CleanupRefDeleted, CleanupTsfnReleased, CleanupHookRemoved, LocalCount,
    GlobalAttempts = LocalCount, GlobalIllegalAttempts, GlobalUnscopedAttempts,
    GlobalModuleRegistrations, GlobalRefCreated, GlobalRefDeleted, GlobalTsfnCreated,
    GlobalTsfnReleased, GlobalHookAdded, GlobalHookRemoved, Count
};
constexpr uint64_t DiagnosticOverflow = 9007199254740992ULL;
using Snapshot = std::array<uint64_t, Count>;
using LocalSnapshot = std::array<uint64_t, LocalCount>;
using ProcessSnapshot = std::array<uint64_t, Count - LocalCount>;
struct State {
    // Set once per reserved slot, before publishing the Runtime. Reset only after physical retirement.
    napi_env env = nullptr;
    std::thread::id thread;
    std::array<std::atomic<uint64_t>, LocalCount> counters{};
    std::atomic<napi_ref> reference{nullptr};
    std::atomic<napi_threadsafe_function> tsfn{nullptr};
    std::atomic<napi_async_cleanup_hook_handle> hook{nullptr};
    void reset(napi_env, std::thread::id) noexcept;
    LocalSnapshot localSnapshot() const noexcept;
    // Convenience for host checks. Production history stores localSnapshot only.
    Snapshot snapshot() const noexcept;
};
ProcessSnapshot processSnapshot() noexcept;
class Scope final {
  public:
    Scope(Mode, napi_env, State * = nullptr) noexcept;
    ~Scope(); // Restore TLS only: no counters, allocation, or owner retention.
    Mode mode() const noexcept { return mode_; }
    Scope(const Scope &) = delete;
    Scope &operator=(const Scope &) = delete;
  private:
    Mode mode_;
    napi_env env_;
    std::thread::id thread_;
    State *state_;
    Scope *previous_;
    friend class Binding;
    friend void attempt(Kind, napi_env, const void *) noexcept;
    friend void wakeDestroyed(State &) noexcept;
    friend State *bound() noexcept;
};
// Declare after the actual local Owner (or as Entry's member after Owner).
// Destruction unbinds before that Owner can retire; this object owns no Runtime.
class Binding final {
  public:
    explicit Binding(State &) noexcept;
    ~Binding();
    Binding(const Binding &) = delete;
    Binding &operator=(const Binding &) = delete;
  private:
    Scope *scope_;
    State *previous_;
};
void attempt(Kind, napi_env = nullptr, const void * = nullptr) noexcept;
void wakeDestroyed(State &) noexcept;
State *bound() noexcept;

template <class Function, class... Args>
napi_status envCall(Function function, napi_env env, Args &&...args) {
    attempt(Kind::Env, env);
    return function(env, std::forward<Args>(args)...);
}
napi_status createReference(napi_env, napi_value, uint32_t, napi_ref *);
napi_status readReference(napi_env, napi_ref, napi_value *);
napi_status deleteReference(napi_env, napi_ref);
napi_status createTsfn(napi_env, napi_value, napi_value, napi_value, size_t, size_t,
                       void *, napi_finalize, void *, napi_threadsafe_function_call_js,
                       napi_threadsafe_function *);
napi_status callTsfn(napi_threadsafe_function, void *, napi_threadsafe_function_call_mode);
napi_status releaseTsfn(napi_threadsafe_function, napi_threadsafe_function_release_mode);
napi_status addHook(napi_env, napi_async_cleanup_hook, void *, napi_async_cleanup_hook_handle *);
napi_status removeHook(napi_async_cleanup_hook_handle);
void registerModule(napi_module *);
} // namespace gateway::napi_observer

// The checked inventory is closed. Include this after native_api.h and before codec helpers.
// Self references in function-like macros name the genuine SDK function; wrappers never recurse.
#ifndef MADB_NAPI_OBSERVER_NO_WRAPPERS
#define napi_add_async_cleanup_hook(...) ::gateway::napi_observer::addHook(__VA_ARGS__)
#define napi_call_function(...) ::gateway::napi_observer::envCall(::napi_call_function, __VA_ARGS__)
#define napi_call_threadsafe_function(...) ::gateway::napi_observer::callTsfn(__VA_ARGS__)
#define napi_close_handle_scope(...) ::gateway::napi_observer::envCall(::napi_close_handle_scope, __VA_ARGS__)
#define napi_create_array_with_length(...) ::gateway::napi_observer::envCall(::napi_create_array_with_length, __VA_ARGS__)
#define napi_create_double(...) ::gateway::napi_observer::envCall(::napi_create_double, __VA_ARGS__)
#define napi_create_int32(...) ::gateway::napi_observer::envCall(::napi_create_int32, __VA_ARGS__)
#define napi_create_object(...) ::gateway::napi_observer::envCall(::napi_create_object, __VA_ARGS__)
#define napi_create_reference(...) ::gateway::napi_observer::createReference(__VA_ARGS__)
#define napi_create_string_utf8(...) ::gateway::napi_observer::envCall(::napi_create_string_utf8, __VA_ARGS__)
#define napi_create_threadsafe_function(...) ::gateway::napi_observer::createTsfn(__VA_ARGS__)
#define napi_define_properties(...) ::gateway::napi_observer::envCall(::napi_define_properties, __VA_ARGS__)
#define napi_delete_reference(...) ::gateway::napi_observer::deleteReference(__VA_ARGS__)
#define napi_get_and_clear_last_exception(...) ::gateway::napi_observer::envCall(::napi_get_and_clear_last_exception, __VA_ARGS__)
#define napi_get_cb_info(...) ::gateway::napi_observer::envCall(::napi_get_cb_info, __VA_ARGS__)
#define napi_get_null(...) ::gateway::napi_observer::envCall(::napi_get_null, __VA_ARGS__)
#define napi_get_reference_value(...) ::gateway::napi_observer::readReference(__VA_ARGS__)
#define napi_get_undefined(...) ::gateway::napi_observer::envCall(::napi_get_undefined, __VA_ARGS__)
#define napi_get_uv_event_loop(...) ::gateway::napi_observer::envCall(::napi_get_uv_event_loop, __VA_ARGS__)
#define napi_get_value_double(...) ::gateway::napi_observer::envCall(::napi_get_value_double, __VA_ARGS__)
#define napi_get_value_string_utf16(...) ::gateway::napi_observer::envCall(::napi_get_value_string_utf16, __VA_ARGS__)
#define napi_get_value_string_utf8(...) ::gateway::napi_observer::envCall(::napi_get_value_string_utf8, __VA_ARGS__)
#define napi_is_exception_pending(...) ::gateway::napi_observer::envCall(::napi_is_exception_pending, __VA_ARGS__)
#define napi_module_register(...) ::gateway::napi_observer::registerModule(__VA_ARGS__)
#define napi_open_handle_scope(...) ::gateway::napi_observer::envCall(::napi_open_handle_scope, __VA_ARGS__)
#define napi_release_threadsafe_function(...) ::gateway::napi_observer::releaseTsfn(__VA_ARGS__)
#define napi_remove_async_cleanup_hook(...) ::gateway::napi_observer::removeHook(__VA_ARGS__)
#define napi_throw_error(...) ::gateway::napi_observer::envCall(::napi_throw_error, __VA_ARGS__)
#define napi_typeof(...) ::gateway::napi_observer::envCall(::napi_typeof, __VA_ARGS__)
#endif
