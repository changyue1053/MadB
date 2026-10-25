#include "provider_abi.h"
#include "validate_snapshot.h"
#include <atomic>
#include <limits>
#include <memory>
#include <new>
#include <stdexcept>
#include <string>
namespace {
struct Context {
    std::atomic<uint64_t> clones{0}, allocations{0}, releases{0}, live{0}, work{0};
    std::atomic<int32_t> injection{0};
};
struct Model {
    int32_t value;
    std::string text;
};
struct ModelDeleter {
    Context *owner;
    void operator()(Model *value) const noexcept {
        delete value; // String and object storage are gone before release is counted.
        ++owner->releases;
        if (owner->live.fetch_sub(1) == 0)
            std::terminate();
    }
};
using OwnedModel = std::unique_ptr<Model, ModelDeleter>;
template <class F> mp_status boundary(F &&body) noexcept {
    try {
        return body();
    } catch (const std::bad_alloc &) {
        return MP_ALLOCATION_FAILURE;
    } catch (...) {
        return MP_INTERNAL_FAILURE;
    }
}
mp_status clone_model(void *opaque, mp_snapshot source, void **out) noexcept {
    if (!out)
        return MP_INVALID_ARGUMENT;
    *out = nullptr;
    if (!opaque || !mp_valid_snapshot(source))
        return MP_INVALID_ARGUMENT;
    return boundary([&]() -> mp_status {
        auto &ctx = *static_cast<Context *>(opaque);
        ++ctx.clones;
        auto injection = ctx.injection.exchange(MP_INJECT_NONE);
        if (injection == MP_INJECT_COPY_ALLOC)
            throw std::bad_alloc{};
        if (injection == MP_INJECT_COPY_INTERNAL)
            throw std::runtime_error("controlled copy exception");
        auto *raw = new Model();
        ++ctx.allocations;
        ++ctx.live;
        OwnedModel model(raw, ModelDeleter{&ctx});
        model->value = source.value;
        if (source.size)
            model->text.assign(reinterpret_cast<const char *>(source.text), source.size);
        if (injection == MP_INJECT_COPY_AFTER_ALLOC_INTERNAL)
            throw std::runtime_error("controlled exception after model allocation");
        *out = model.release();
        return MP_OK;
    });
}
mp_status read_model(void *ctx, const void *opaque, mp_snapshot *out) noexcept {
    if (!ctx || !opaque || !out)
        return MP_INVALID_ARGUMENT;
    auto &model = *static_cast<const Model *>(opaque);
    *out = {model.value, reinterpret_cast<const uint8_t *>(model.text.data()), model.text.size()};
    return MP_OK;
}
mp_status increment_model(void *opaque, void *value) noexcept {
    if (!opaque || !value)
        return MP_INVALID_ARGUMENT;
    return boundary([&]() -> mp_status {
        auto &ctx = *static_cast<Context *>(opaque);
        auto &model = *static_cast<Model *>(value);
        ++ctx.work;
        if (ctx.injection.exchange(MP_INJECT_NONE) == MP_INJECT_WORK_INTERNAL)
            throw std::runtime_error("controlled work exception");
        if (model.value == std::numeric_limits<int32_t>::max())
            return MP_DOMAIN_OVERFLOW;
        ++model.value;
        return MP_OK;
    });
}
mp_status release_model(void *opaque, void *model) noexcept {
    if (!opaque || !model)
        return MP_INVALID_ARGUMENT;
    OwnedModel owned(static_cast<Model *>(model), ModelDeleter{static_cast<Context *>(opaque)});
    owned.reset();
    return MP_OK;
}
mp_status read_stats(void *opaque, mp_stats *out) noexcept {
    if (!opaque || !out)
        return MP_INVALID_ARGUMENT;
    auto &ctx = *static_cast<Context *>(opaque);
    *out = {ctx.clones.load(), ctx.allocations.load(), ctx.releases.load(), ctx.live.load(),
            ctx.work.load()};
    return MP_OK;
}
mp_status inject(void *opaque, int32_t mode) noexcept {
    if (!opaque || mode < MP_INJECT_NONE || mode > MP_INJECT_COPY_AFTER_ALLOC_INTERNAL)
        return MP_INVALID_ARGUMENT;
    static_cast<Context *>(opaque)->injection.store(mode);
    return MP_OK;
}
mp_status destroy_context(void *opaque) noexcept {
    if (!opaque)
        return MP_INVALID_ARGUMENT;
    auto *ctx = static_cast<Context *>(opaque);
    if (ctx->live.load())
        return MP_BUSY;
    delete ctx;
    return MP_OK;
}
} // namespace
extern "C" mp_status mp_cpp_create(mp_provider *out) noexcept {
    if (!out)
        return MP_INVALID_ARGUMENT;
    *out = {};
    return boundary([&]() -> mp_status {
        auto ctx = std::make_unique<Context>();
        *out = {ctx.release(), clone_model, read_model, increment_model,
                release_model, read_stats,  inject,     destroy_context};
        return MP_OK;
    });
}
