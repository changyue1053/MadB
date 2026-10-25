#include "provider_abi.h"
#include "validate_snapshot.h"
#include <limits.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
typedef struct {
    _Atomic uint64_t clone_calls, allocations, releases, live, work_calls;
    _Atomic int32_t injection;
} c_context;
typedef struct {
    int32_t value;
    size_t size;
    uint8_t text[];
} c_model;
static mp_status release_model(void *, void *);
static mp_status clone_model(void *opaque, mp_snapshot source, void **out) {
    if (!out)
        return MP_INVALID_ARGUMENT;
    *out = NULL;
    if (!opaque || !mp_valid_snapshot(source))
        return MP_INVALID_ARGUMENT;
    c_context *ctx = opaque;
    atomic_fetch_add(&ctx->clone_calls, 1);
    int32_t mode = atomic_exchange(&ctx->injection, MP_INJECT_NONE);
    if (mode == MP_INJECT_COPY_ALLOC)
        return MP_ALLOCATION_FAILURE;
    if (mode == MP_INJECT_COPY_INTERNAL)
        return MP_INTERNAL_FAILURE;
    c_model *model = malloc(sizeof(*model) + source.size);
    if (!model)
        return MP_ALLOCATION_FAILURE;
    model->value = source.value;
    model->size = source.size;
    if (source.size)
        memcpy(model->text, source.text, source.size);
    atomic_fetch_add(&ctx->allocations, 1);
    atomic_fetch_add(&ctx->live, 1);
    if (mode == MP_INJECT_COPY_AFTER_ALLOC_INTERNAL) {
        release_model(ctx, model);
        return MP_INTERNAL_FAILURE;
    }
    *out = model;
    return MP_OK;
}
static mp_status read_model(void *ctx, const void *opaque, mp_snapshot *out) {
    if (!ctx || !opaque || !out)
        return MP_INVALID_ARGUMENT;
    const c_model *model = opaque;
    *out = (mp_snapshot){model->value, model->text, model->size};
    return MP_OK;
}
static mp_status increment_model(void *opaque, void *value) {
    if (!opaque || !value)
        return MP_INVALID_ARGUMENT;
    c_context *ctx = opaque;
    c_model *model = value;
    atomic_fetch_add(&ctx->work_calls, 1);
    if (atomic_exchange(&ctx->injection, MP_INJECT_NONE) == MP_INJECT_WORK_INTERNAL)
        return MP_INTERNAL_FAILURE;
    if (model->value == INT32_MAX)
        return MP_DOMAIN_OVERFLOW;
    ++model->value;
    return MP_OK;
}
static mp_status release_model(void *opaque, void *model) {
    if (!opaque || !model)
        return MP_INVALID_ARGUMENT;
    c_context *ctx = opaque;
    free(model);
    atomic_fetch_add(&ctx->releases, 1);
    if (atomic_fetch_sub(&ctx->live, 1) == 0)
        abort();
    return MP_OK;
}
static mp_status read_stats(void *opaque, mp_stats *out) {
    if (!opaque || !out)
        return MP_INVALID_ARGUMENT;
    c_context *ctx = opaque;
    *out = (mp_stats){atomic_load(&ctx->clone_calls), atomic_load(&ctx->allocations),
                      atomic_load(&ctx->releases), atomic_load(&ctx->live),
                      atomic_load(&ctx->work_calls)};
    return MP_OK;
}
static mp_status inject(void *opaque, int32_t mode) {
    if (!opaque || mode < MP_INJECT_NONE || mode > MP_INJECT_COPY_AFTER_ALLOC_INTERNAL)
        return MP_INVALID_ARGUMENT;
    atomic_store(&((c_context *)opaque)->injection, mode);
    return MP_OK;
}
static mp_status destroy_context(void *opaque) {
    if (!opaque)
        return MP_INVALID_ARGUMENT;
    c_context *ctx = opaque;
    if (atomic_load(&ctx->live))
        return MP_BUSY;
    free(ctx);
    return MP_OK;
}
mp_status mp_c_create(mp_provider *out) {
    if (!out)
        return MP_INVALID_ARGUMENT;
    *out = (mp_provider){0};
    c_context *ctx = calloc(1, sizeof(*ctx));
    if (!ctx)
        return MP_ALLOCATION_FAILURE;
    *out = (mp_provider){ctx,           clone_model, read_model, increment_model,
                         release_model, read_stats,  inject,     destroy_context};
    return MP_OK;
}
