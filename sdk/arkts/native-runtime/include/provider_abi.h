#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
#define MP_NOEXCEPT noexcept
extern "C" {
#else
#define MP_NOEXCEPT
#endif
/* Private prototype ABI, not a released SDK contract. Valid provenance pointers
 * and caller-held context/DSO lifetime are required. No STL/Box layout crosses. */
typedef int32_t mp_status;
enum {
    MP_OK = 0,
    MP_INVALID_ARGUMENT = 1,
    MP_ALLOCATION_FAILURE = 2,
    MP_DOMAIN_OVERFLOW = 3,
    MP_INTERNAL_FAILURE = 4,
    MP_BUSY = 5
};
typedef struct {
    int32_t value;
    const uint8_t *text;
    size_t size;
} mp_snapshot;
typedef struct {
    uint64_t clone_calls, allocations, releases, live, work_calls;
} mp_stats;
enum {
    MP_INJECT_NONE = 0,
    MP_INJECT_COPY_ALLOC = 1,
    MP_INJECT_COPY_INTERNAL = 2,
    MP_INJECT_WORK_INTERNAL = 3,
    MP_INJECT_COPY_AFTER_ALLOC_INTERNAL = 4
};
typedef struct mp_provider {
    void *context;
    /* On failure *out == NULL. On success caller owns exactly one paired release. */
    mp_status (*clone)(void *, mp_snapshot, void **out);
    /* Borrowed view valid only while model/context live and before mutation. */
    mp_status (*read_view)(void *, const void *, mp_snapshot *out);
    /* Unique mutable borrow; text unchanged. Failure preserves input model. */
    mp_status (*increment)(void *, void *);
    /* Consumes one genuine own-provider model. Does not accept foreign pointers. */
    mp_status (*release)(void *, void *);
    /* Test support: per-instance counters and next-call injected failure only. */
    mp_status (*stats)(void *, mp_stats *out);
    mp_status (*inject)(void *, int32_t mode);
    /* Fails Busy without consuming context if any model remains. */
    mp_status (*destroy_context)(void *);
} mp_provider;
mp_status mp_c_create(mp_provider *out) MP_NOEXCEPT;
mp_status mp_cpp_create(mp_provider *out) MP_NOEXCEPT;
mp_status mp_rust_create(mp_provider *out) MP_NOEXCEPT;
/* Independent Rust repr(C) layout observation, compared with C offsetof/sizeof. */
typedef struct {
    size_t snapshot_size, snapshot_align, value_offset, text_offset, length_offset, provider_size,
        provider_align;
} mp_layout;
mp_status mp_rust_layout(mp_layout *out) MP_NOEXCEPT;
#ifdef __cplusplus
}
#endif
