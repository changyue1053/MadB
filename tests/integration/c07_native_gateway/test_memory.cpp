#define MADB_NAPI_OBSERVER_NO_WRAPPERS
#include "support.hpp"
#include <cassert>
#include <iostream>
#include <vector>
using namespace gateway;
using namespace madb::c07;
#define check(v)                                                                                   \
    do {                                                                                           \
        if (!(v)) {                                                                                \
            std::cerr << "memory check line " << __LINE__ << ": " #v << "\n";                      \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
int64_t at(const MemorySnapshot &s, MemoryField field) {
    check(s.valid);
    return s.values[static_cast<size_t>(field)];
}
struct Fixture {
    Audit audit;
    Runtime runtime{0, 1, nullptr, audit};
    Fixture() {
        audit.memory.reset();
        check(GatewayProviderCreate(&runtime.provider) == MP_OK);
        runtime.engine = std::make_unique<Engine>(testProvider(runtime));
    }
    ~Fixture() {
        runtime.engine->request_close();
        runtime.engine.reset();
        check(runtime.provider.destroy_context(runtime.provider.context) == MP_OK);
        auto s = audit.memory.snapshot();
        check(at(s, MemoryField::CoreRetainedCurrent) == 0);
        check(at(s, MemoryField::PublicationStagingCurrent) == 0);
        check(at(s, MemoryField::ProviderObjectCurrent) == 0);
        check(at(s, MemoryField::ProviderTextCurrent) == 0);
        check(at(s, MemoryField::ProviderStorageAcquired) ==
              at(s, MemoryField::ProviderStorageReleased));
    }
    MemorySnapshot read() {
        return audit.memory.snapshot();
    }
};
Snapshot input(size_t size, int value = 1) {
    static std::array<unsigned char, 1024> text{};
    text.fill('a');
    return {value, text.data(), size};
}
std::vector<uint64_t> observe(Fixture &f) {
    std::vector<uint64_t> ids;
    for (int i = 0; i < 4; ++i) {
        uint64_t id = 0;
        check(f.runtime.engine->observe(
                  Stream::State,
                  Callback(
                      nullptr, [](void *, Status, const void *) {}, [](void *) noexcept {}),
                  id) == Status::Ok);
        ids.push_back(id);
    }
    return ids;
}
void drain(Fixture &f, const std::vector<uint64_t> &ids) {
    for (auto id : ids) {
        Delivery d;
        check(f.runtime.engine->take_delivery(id, d) == Status::Ok);
        check(d.invoke() == Status::Ok);
    }
}
void full_refusal() {
    Fixture f;
    auto ids = observe(f);
    check(f.runtime.engine->publish(Stream::State, input(1024)) == Status::Ok);
    auto before = f.read();
    check(at(before, MemoryField::CoreRetainedCurrent) == 4096);
    check(at(before, MemoryField::CoreRetainedPeak) == 4096);
    check(at(before, MemoryField::PublicationStagingCurrent) == 0);
    check(at(before, MemoryField::PublicationStagingPeak) == 4096);
    check(at(before, MemoryField::ProviderObjectCurrent) ==
          4 * at(before, MemoryField::ModelObjectSize));
    check(at(before, MemoryField::ProviderTextCurrent) >= 4096);
    check(f.runtime.engine->publish(Stream::State, input(1024, 2)) == Status::ResourceExhausted);
    check(f.read().values == before.values); // Failed admission created no fake storage event.
    drain(f, ids);
    check(at(f.read(), MemoryField::CoreRetainedCurrent) == 0);
    check(f.runtime.engine->publish(Stream::State, input(1024, 2)) == Status::Ok);
    check(at(f.read(), MemoryField::CoreRetainedPeak) == 4096);
    drain(f, ids);
}
void legal_overlap() {
    Fixture f;
    auto ids = observe(f);
    check(f.runtime.engine->publish(Stream::State, input(256)) == Status::Ok);
    auto first = f.read();
    check(f.runtime.engine->publish(Stream::State, input(256, 2)) == Status::Ok);
    auto second = f.read();
    check(at(second, MemoryField::CoreRetainedCurrent) == 1024);
    check(at(second, MemoryField::CoreRetainedPeak) == 2048);
    check(at(second, MemoryField::PublicationStagingPeak) == 1024);
    check(at(second, MemoryField::PublicationStagingCurrent) == 0);
    check(at(second, MemoryField::ProviderStoragePeak) >
          at(first, MemoryField::ProviderStoragePeak));
    drain(f, ids);
}
void independent_model() {
    Fixture f;
    Executor executor;
    check(f.runtime.engine->acquire_executor(executor) == Status::Ok);
    for (int i = 0; i < 4; ++i) {
        uint64_t id = 0;
        check(f.runtime.engine->start(input(1024), id) == Status::Ok);
    }
    auto before = f.read();
    void *model = nullptr;
    auto in = input(1024);
    check(testRoundtripClone(f.runtime, {static_cast<int32_t>(in.value), in.text, in.size},
                             &model) == MP_OK);
    auto during = f.read();
    check(at(during, MemoryField::CoreRetainedCurrent) == 4096);
    check(at(during, MemoryField::CoreRetainedPeak) == 4096);
    check(at(during, MemoryField::ProviderStoragePeak) >
          at(before, MemoryField::ProviderStoragePeak));
    check(at(during, MemoryField::ProviderObjectCurrent) ==
          5 * at(during, MemoryField::ModelObjectSize));
    check(f.runtime.provider.release(f.runtime.provider.context, model) == MP_OK);
    f.runtime.engine->request_close();
    executor.finish();
}
void failed_allocated_staging() {
    Fixture f;
    observe(f);
    f.runtime.test.cloneNth = 2;
    f.runtime.test.cloneMode = 4;
    check(f.runtime.engine->publish(Stream::State, input(256)) == Status::InternalFailure);
    auto s = f.read();
    check(at(s, MemoryField::CoreRetainedCurrent) == 0);
    check(at(s, MemoryField::PublicationStagingCurrent) == 0);
    check(at(s, MemoryField::PublicationStagingPeak) == 512);
    check(at(s, MemoryField::ProviderStorageAcquired) ==
          at(s, MemoryField::ProviderStorageReleased));
}
void provider_failure_and_empty() {
    Fixture f;
    void *model = nullptr;
    auto value = input(256);
    check(f.runtime.provider.inject(f.runtime.provider.context,
                                    MP_INJECT_COPY_AFTER_ALLOC_INTERNAL) == MP_OK);
    check(f.runtime.provider.clone(f.runtime.provider.context, {1, value.text, value.size},
                                   &model) == MP_INTERNAL_FAILURE);
    check(model == nullptr);
    auto failed = f.read();
    check(at(failed, MemoryField::ProviderObjectCurrent) == 0 &&
          at(failed, MemoryField::ProviderTextCurrent) == 0);
    check(at(failed, MemoryField::ProviderStorageAcquired) ==
          at(failed, MemoryField::ProviderStorageReleased));
    check(at(failed, MemoryField::ProviderStoragePeak) > at(failed, MemoryField::ModelObjectSize));
    check(f.runtime.provider.clone(f.runtime.provider.context, {1, nullptr, 0}, &model) == MP_OK);
    auto empty = f.read();
    check(at(empty, MemoryField::ProviderObjectCurrent) == at(empty, MemoryField::ModelObjectSize));
    check(at(empty, MemoryField::ProviderTextCurrent) ==
          0); // Empty/inline storage is not a second allocation.
    check(f.runtime.provider.release(f.runtime.provider.context, model) == MP_OK);
}
void accounting_failures() {
    MemoryState s;
    s.reset();
    s.staging(1, false);
    check(!s.snapshot().valid);
    s.reset();
    s.provider(9007199254740992ULL, 0, 1);
    check(!s.snapshot().valid);
    s.reset();
    s.provider(32, 0, 0);
    s.provider(16, 0, 0);
    check(!s.snapshot().valid);
}
struct napi_value__ {
    double number = 0;
    std::u16string text;
};
extern "C" napi_status napi_get_value_double(napi_env, napi_value v, double *out) {
    *out = v->number;
    return napi_ok;
}
extern "C" napi_status napi_get_value_string_utf16(napi_env, napi_value v, char16_t *out,
                                                   size_t capacity, size_t *length) {
    *length = v->text.size();
    if (out) {
        check(capacity > *length);
        std::copy(v->text.begin(), v->text.end(), out);
        out[*length] = 0;
    }
    return napi_ok;
}
extern "C" napi_status napi_throw_error(napi_env, const char *, const char *) {
    return napi_ok;
}
void actual_codec_scopes() {
    MemoryState state;
    state.reset();
    napi_value__ number;
    number.number = 7;
    napi_value__ text;
    text.text.assign(1024, u'a');
    for (int i = 0; i < 2; ++i) {
        MemoryInputScope scope(state, sizeof(Input));
        Input value;
        if (i)
            text.text.assign(1, char16_t{0xD800});
        check(snapshot(nullptr, &number, &text, value) == (i == 0));
        auto s = state.snapshot();
        check(at(s, MemoryField::InputObjectCurrent) == sizeof(Input));
        check(at(s, MemoryField::Utf16ArrayCurrent) == 0);
        check(at(s, MemoryField::Utf8BackingCurrent) ==
              static_cast<int64_t>(value.text.capacity() + 1));
    }
    auto s = state.snapshot();
    check(at(s, MemoryField::InputObjectCurrent) == 0);
    check(at(s, MemoryField::Utf8BackingCurrent) == 0);
    check(at(s, MemoryField::Utf8BackingAcquired) == at(s, MemoryField::Utf8BackingReleased));
    check(at(s, MemoryField::InputScopesEntered) == 2 &&
          at(s, MemoryField::InputScopesExited) == 2);
    check(at(s, MemoryField::Utf16ScopesEntered) == 2 &&
          at(s, MemoryField::Utf16ScopesExited) == 2);
    check(at(s, MemoryField::Utf16ArrayPeak) == sizeof(std::array<char16_t, 1025>));
}
int main() {
    full_refusal();
    legal_overlap();
    independent_model();
    failed_allocated_staging();
    provider_failure_and_empty();
    accounting_failures();
    actual_codec_scopes();
    std::cout << "7 real host memory checks passed; codec NAPI stubs, no target claim\n";
}
