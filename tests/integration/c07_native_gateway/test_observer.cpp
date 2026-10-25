// Host-only recording stubs. No JS engine, libace, or real Node-API library is linked.
#include <napi/native_api.h>
#include <cstdlib>
#include <iostream>
#include <memory>
#include <thread>
static size_t stubCalls = 0;
static napi_status nextStatus = napi_ok;
static uint64_t (*readAttempts)() = nullptr;
static uint64_t attemptsSeenByStub = 0;
static napi_status stub() {
    ++stubCalls;
    if (readAttempts) attemptsSeenByStub = readAttempts();
    return nextStatus;
}
template<class T> T pointer(uintptr_t n) { return reinterpret_cast<T>(n); }
extern "C" {
napi_status napi_create_int32(napi_env, int32_t, napi_value *out) {
    *out = pointer<napi_value>(9); return stub();
}
napi_status napi_create_reference(napi_env, napi_value, uint32_t, napi_ref *out) {
    if (nextStatus == napi_ok) *out = pointer<napi_ref>(10); return stub();
}
napi_status napi_get_reference_value(napi_env, napi_ref, napi_value *out) {
    *out = pointer<napi_value>(9); return stub();
}
napi_status napi_delete_reference(napi_env, napi_ref) { return stub(); }
napi_status napi_create_threadsafe_function(napi_env, napi_value, napi_value, napi_value,
    size_t, size_t, void *, napi_finalize, void *, napi_threadsafe_function_call_js,
    napi_threadsafe_function *out) {
    if (nextStatus == napi_ok) *out = pointer<napi_threadsafe_function>(11); return stub();
}
napi_status napi_call_threadsafe_function(napi_threadsafe_function, void *, napi_threadsafe_function_call_mode) {
    return stub();
}
napi_status napi_release_threadsafe_function(napi_threadsafe_function, napi_threadsafe_function_release_mode) {
    return stub();
}
napi_status napi_add_async_cleanup_hook(napi_env, napi_async_cleanup_hook, void *, napi_async_cleanup_hook_handle *out) {
    if (nextStatus == napi_ok) *out = pointer<napi_async_cleanup_hook_handle>(12); return stub();
}
napi_status napi_remove_async_cleanup_hook(napi_async_cleanup_hook_handle) { return stub(); }
void napi_module_register(napi_module *) { (void)stub(); }
}
#include "observer.hpp"
using namespace gateway::napi_observer;
#define require(expr) do { if (!(expr)) { std::cerr << "failure " << __LINE__ << ": " #expr "\n"; std::abort(); } } while (false)
static State *observed = nullptr;
static uint64_t readBoundAttempts() { return observed->snapshot()[Attempts]; }
static void createOwners(State &s) {
    Scope scope(Mode::Entry, s.env);
    Binding binding(s);
    napi_ref ref = nullptr;
    napi_threadsafe_function tsfn = nullptr;
    napi_async_cleanup_hook_handle hook = nullptr;
    require(napi_create_reference(s.env, pointer<napi_value>(9), 1, &ref) == napi_ok);
    require(napi_create_threadsafe_function(s.env, nullptr, nullptr, nullptr, 1, 1,
                                          nullptr, nullptr, nullptr, nullptr, &tsfn) == napi_ok);
    require(napi_add_async_cleanup_hook(s.env, nullptr, nullptr, &hook) == napi_ok);
    require(s.reference == ref && s.tsfn == tsfn && s.hook == hook);
}
int main() {
    auto env = pointer<napi_env>(1), other = pointer<napi_env>(2);
    State s;
    s.reset(env, std::this_thread::get_id());
    observed = &s;
    readAttempts = readBoundAttempts;
    napi_value value = nullptr;
    { // Unbound bootstrap calls still have real callback authority and global accounting.
        Scope scope(Mode::Entry, env);
        const auto before = s.snapshot();
        require(napi_create_int32(env, 0, &value) == napi_ok);
        require(s.snapshot()[Attempts] == 0);
        require(s.snapshot()[GlobalAttempts] == before[GlobalAttempts] + 1);
        require(s.snapshot()[GlobalIllegalAttempts] == 0);
    }
    createOwners(s);
    require(s.snapshot()[Attempts] == 3 && s.snapshot()[IllegalAttempts] == 0);
    require(attemptsSeenByStub == 3); // Observation precedes the genuine/stub call.
    {
        Scope scope(Mode::Entry, env, &s);
        require(napi_get_reference_value(env, s.reference, &value) == napi_ok);
        nextStatus = napi_generic_failure;
        require(napi_create_int32(env, 1, &value) == napi_generic_failure);
        nextStatus = napi_ok;
    }
    require(s.snapshot()[IllegalAttempts] == 0);
    std::thread worker([&] {
        Scope scope(Mode::Worker, nullptr, &s);
        require(napi_call_threadsafe_function(s.tsfn, nullptr, napi_tsfn_nonblocking) == napi_ok);
        require(napi_create_int32(env, 1, &value) == napi_ok); // Deliberate host-only negative.
    });
    worker.join();
    require(s.snapshot()[IllegalAttempts] == 1 && s.snapshot()[WrongThread] == 1);
    {
        Scope scope(Mode::Entry, other, &s);
        require(napi_create_int32(other, 1, &value) == napi_ok);
    }
    require(s.snapshot()[IllegalAttempts] == 2 && s.snapshot()[WrongEnv] >= 1);
    {
        Scope scope(Mode::Wake, nullptr, &s);
        require(napi_create_int32(env, 1, &value) == napi_ok);
        wakeDestroyed(s); // Host classifier check only; target mark follows actual destruction.
    }
    require(s.snapshot()[IllegalAttempts] == 3);
    require(s.snapshot()[NullWakeEntered] == 1 && s.snapshot()[NullWakeDestroyed] == 1);
    {
        Scope scope(Mode::Finalizer, nullptr, &s);
        require(napi_call_threadsafe_function(s.tsfn, nullptr, napi_tsfn_nonblocking) == napi_ok);
    }
    require(s.snapshot()[IllegalAttempts] == 4 && s.snapshot()[NullFinalizerEntered] == 1);
    {
        Scope scope(Mode::Cleanup, env, &s);
        nextStatus = napi_generic_failure;
        require(napi_delete_reference(env, s.reference) == napi_generic_failure);
        require(s.reference != nullptr && s.snapshot()[CleanupRefDeleted] == 0);
        nextStatus = napi_ok;
        require(napi_delete_reference(env, s.reference) == napi_ok);
        require(napi_create_int32(env, 1, &value) == napi_ok); // Value creation forbidden in hook.
    }
    require(s.snapshot()[IllegalAttempts] == 5 && s.snapshot()[CleanupRefDeleted] == 1);
    {
        Scope scope(Mode::CleanupAfterJoin, env, &s);
        nextStatus = napi_generic_failure;
        require(napi_release_threadsafe_function(s.tsfn, napi_tsfn_abort) == napi_generic_failure);
        require(napi_remove_async_cleanup_hook(s.hook) == napi_generic_failure);
        require(s.tsfn && s.hook);
        nextStatus = napi_ok;
        require(napi_release_threadsafe_function(s.tsfn, napi_tsfn_abort) == napi_ok);
        require(napi_remove_async_cleanup_hook(s.hook) == napi_ok);
    }
    require(s.snapshot()[IllegalAttempts] == 5);
    require(s.snapshot()[CleanupTsfnReleased] == 1 && s.snapshot()[CleanupHookRemoved] == 1);
    {
        Scope scope(Mode::Worker, nullptr, &s);
        require(napi_call_threadsafe_function(pointer<napi_threadsafe_function>(11), nullptr,
                                             napi_tsfn_nonblocking) == napi_ok);
    }
    require(s.snapshot()[IllegalAttempts] == 6 && s.snapshot()[InvalidOwner] == 1);
    {
        Scope outer(Mode::Entry, env, &s);
        const auto before = s.snapshot();
        { Scope inner(Mode::Entry, other); require(bound() == nullptr); }
        require(bound() == &s && s.snapshot() == before); // Destructor does not write counters.
    }
    const auto beforeUnscoped = s.snapshot();
    require(napi_create_int32(env, 1, &value) == napi_ok);
    require(s.snapshot()[GlobalUnscopedAttempts] == beforeUnscoped[GlobalUnscopedAttempts] + 1);
    require(s.snapshot()[GlobalIllegalAttempts] == beforeUnscoped[GlobalIllegalAttempts] + 1);
    {
        Scope module(Mode::Module, nullptr);
        napi_module_register(nullptr);
    }
    require(s.snapshot()[GlobalModuleRegistrations] == 1);
    State wrongThread;
    wrongThread.reset(env, std::this_thread::get_id());
    createOwners(wrongThread);
    std::thread falseEntry([&] {
        Scope scope(Mode::Entry, env, &wrongThread);
        // A fabricated Entry scope cannot authorize owner teardown from a foreign thread.
        nextStatus = napi_generic_failure;
        require(napi_release_threadsafe_function(wrongThread.tsfn, napi_tsfn_abort) == napi_generic_failure);
        require(napi_remove_async_cleanup_hook(wrongThread.hook) == napi_generic_failure);
        nextStatus = napi_ok;
    });
    falseEntry.join();
    require(wrongThread.localSnapshot()[IllegalAttempts] == 2);
    require(wrongThread.localSnapshot()[WrongThread] == 2);
    require(wrongThread.tsfn && wrongThread.hook);
    {
        Scope scope(Mode::CleanupAfterJoin, env, &wrongThread);
        require(napi_delete_reference(env, wrongThread.reference) == napi_ok);
    }
    require(wrongThread.localSnapshot()[IllegalAttempts] == 3); // Deletion belongs to immediate hook.
    {
        Scope scope(Mode::JoinWork, env, &wrongThread);
        require(napi_create_int32(env, 1, &value) == napi_ok);
    }
    require(wrongThread.localSnapshot()[IllegalAttempts] == 4);
    State retired;
    retired.reset(env, std::this_thread::get_id());
    {
        Scope callback(Mode::Entry, env);
        struct LastOwner {
            State &state;
            napi_env replacement;
            LastOwner(State &s, napi_env e) : state(s), replacement(e) {}
            ~LastOwner() {
                require(bound() != &state);
                state.reset(replacement, std::this_thread::get_id());
            }
        };
        try {
            auto owner = std::make_unique<LastOwner>(retired, other);
            Binding binding(retired); // Must unwind before the actual owner.
            require(napi_create_int32(env, 1, &value) == napi_ok);
            throw 1;
        } catch (int) {
            require(bound() == nullptr);
            require(retired.localSnapshot()[Attempts] == 0);
            require(napi_create_int32(env, 1, &value) == napi_ok); // Valid unbound error/catch path.
            require(retired.localSnapshot()[Attempts] == 0); // No write into reused slot.
        }
    }
    State overflow;
    overflow.reset(env, std::this_thread::get_id());
    overflow.counters[Attempts] = DiagnosticOverflow - 1;
    {
        Scope scope(Mode::Entry, env, &overflow);
        require(napi_create_int32(env, 1, &value) == napi_ok);
        require(napi_create_int32(env, 1, &value) == napi_ok);
    }
    require(overflow.localSnapshot()[Attempts] == DiagnosticOverflow); // Saturates; cannot wrap low.
    require(s.snapshot()[GlobalRefCreated] == 2 && s.snapshot()[GlobalRefDeleted] == 2);
    require(s.snapshot()[GlobalTsfnCreated] == 2 && s.snapshot()[GlobalTsfnReleased] == 1);
    require(s.snapshot()[GlobalHookAdded] == 2 && s.snapshot()[GlobalHookRemoved] == 1);
    require(s.snapshot()[GlobalAttempts] == stubCalls);
    std::cout << "observer host controls passed; " << stubCalls
              << " recording-stub calls; no VM or target-lifetime claim\n";
}
