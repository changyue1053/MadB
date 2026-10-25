// Compiled separately into each test DSO. There is no shared observer library,
// no observer-owned NAPI handle, no allocation, and no lifecycle policy change.
#include <atomic>
#include <cstdint>
#include <cstring>
#include <pthread.h>
namespace {
constexpr int FixedFields = 18;
constexpr uint64_t Unknown = UINT64_MAX;
std::atomic<uint64_t> attempts[MADB_KN_OBSERVER_API_COUNT] = { MADB_KN_OBSERVER_COUNTER_INITIALIZERS };
std::atomic<uint64_t> totals[14] = {{0},{0},{0},{0},{0},{0},{0},{0},{0},{0},{0},{0},{0},{0}}; // indices 4..17 of the public frame
uintptr_t threadToken() noexcept {
    const pthread_t thread = pthread_self();
    uintptr_t result = 0;
    static_assert(sizeof(thread) <= sizeof(result), "pthread identity must fit without truncation");
    std::memcpy(&result, &thread, sizeof(thread));
    return result;
}
#if MADB_KN_OBSERVER_ROLE == 0
// Intrusive nodes live in the actual callback's RAII stack frame. No TLS
// allocation, retained thread/env registry, or observer-owned heap is needed.
std::atomic_flag entriesLock = ATOMIC_FLAG_INIT;
madb_kn_napi_entry *entries = nullptr;
struct EntryLock final {
    EntryLock() noexcept { while (entriesLock.test_and_set(std::memory_order_acquire)) {} }
    ~EntryLock() { entriesLock.clear(std::memory_order_release); }
};
void *currentEntryEnv() noexcept {
    const uintptr_t thread = threadToken();
    const EntryLock lock;
    for (auto *entry = entries; entry; entry = entry->next)
        if (entry->thread == thread) return entry->env;
    return nullptr;
}
#else
std::atomic<uintptr_t> boundEnv{0};
std::atomic<uintptr_t> boundThread{0};
#endif
void increment(int field) noexcept { totals[field - 4].fetch_add(1, std::memory_order_relaxed); }
// category0: env argument;1: thread-neutral TSFN;2: process registration;
// 3: async cleanup handle, no env argument but still owner-thread constrained.
void recordAttempt(int index, int category, void *env) noexcept {
    attempts[index].fetch_add(1, std::memory_order_relaxed);
    increment(4);
    if (category == 1) { increment(6); return; }
    if (category == 2) { increment(7); return; }
#if MADB_KN_OBSERVER_ROLE == 0
    const void *owner = currentEntryEnv();
#else
    const uintptr_t owner = boundEnv.load(std::memory_order_acquire);
    const uintptr_t thread = boundThread.load(std::memory_order_acquire);
#endif
    if (category == 3) {
        increment(8);
        if (!owner) increment(13);
#if MADB_KN_OBSERVER_ROLE == 1
        else if (thread != threadToken()) increment(12);
#endif
        return;
    }
    increment(5);
    // Neither normal Runtime.close nor entry into a system cleanup hook proves
    // the env invalid. We deliberately do not synthesize an invalid-env count.
    increment(17);
    if (!env) increment(9);
    if (!owner) increment(10);
#if MADB_KN_OBSERVER_ROLE == 0
    else if (env && owner != env) increment(11);
#else
    else {
        if (env && owner != reinterpret_cast<uintptr_t>(env)) increment(11);
        if (thread != threadToken()) increment(12);
    }
#endif
}
uint64_t metric(int index) noexcept {
    switch (index) {
        case 0: return 70602;
        case 1: return MADB_KN_OBSERVER_ROLE;
        case 2: return MADB_KN_OBSERVER_API_COUNT;
        case 3: return MADB_KN_OBSERVER_ROLE == 0 ? 1 : 6;
#if MADB_KN_OBSERVER_ROLE == 0
        case 12: return Unknown; // bootstrap uses entry scope, not a single thread owner
#endif
        default: break;
    }
    if (index >= 4 && index < FixedFields) return totals[index - 4].load(std::memory_order_relaxed);
    if (index >= FixedFields && index < FixedFields + MADB_KN_OBSERVER_API_COUNT)
        return attempts[index - FixedFields].load(std::memory_order_relaxed);
    return Unknown;
}
}
#if MADB_KN_OBSERVER_ROLE == 0
extern "C" __attribute__((visibility("default"))) uint64_t madb_kn_bootstrap_napi_metric(int index) { return metric(index); }
extern "C" __attribute__((visibility("hidden"))) void madb_kn_bootstrap_napi_enter(madb_kn_napi_entry *entry, void *env) {
    entry->env = env; entry->thread = threadToken();
    const EntryLock lock;
    entry->previous = nullptr; entry->next = entries;
    if (entries) entries->previous = entry;
    entries = entry; increment(14);
}
extern "C" __attribute__((visibility("hidden"))) void madb_kn_bootstrap_napi_leave(madb_kn_napi_entry *entry) {
    const EntryLock lock;
    if (entry->previous) entry->previous->next = entry->next;
    else entries = entry->next;
    if (entry->next) entry->next->previous = entry->previous;
}
#else
extern "C" __attribute__((visibility("default"))) uint64_t madb_kn_provider_napi_metric(int index) { return metric(index); }
extern "C" __attribute__((visibility("hidden"))) void madb_kn_provider_napi_bind(void *env) {
    if (!env) { increment(16); return; }
    boundThread.store(threadToken(), std::memory_order_release);
    boundEnv.store(reinterpret_cast<uintptr_t>(env), std::memory_order_release);
    increment(15);
}
#endif
