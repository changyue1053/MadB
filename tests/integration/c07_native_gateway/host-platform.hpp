#pragma once
#include <cstdint>
#include <pthread.h>
// Host-only thread-id port; no Node-API function is replaced or called.
inline uint64_t gettid() {
    uint64_t id = 0;
    pthread_threadid_np(nullptr, &id);
    return id;
}
