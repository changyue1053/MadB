#include "protocol.hpp"
#include "provider_abi.h"
#include "provider_port.hpp"
#include <atomic>
#include <climits>
#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>
#include <vector>
using namespace madb::c07;
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x)) {                                                                                \
            std::cerr << __FILE__ << ":" << __LINE__ << " failed " #x << std::endl;                \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
#define SUCCESS(x) CHECK((x) == MP_OK)
using Factory = mp_status (*)(mp_provider *);
mp_snapshot snapshot(int32_t value, const std::string &text) {
    return {value, reinterpret_cast<const uint8_t *>(text.data()), text.size()};
}
mp_stats stats(mp_provider &p) {
    mp_stats result{};
    SUCCESS(p.stats(p.context, &result));
    return result;
}
void same_text(mp_snapshot s, const std::string &expected) {
    CHECK(s.size == expected.size());
    if (s.size)
        CHECK(!std::memcmp(s.text, expected.data(), s.size));
}
struct Observation {
    explicit Observation(mp_provider &p) : provider(&p) {}
    mp_provider *provider;
    int calls = 0, drops = 0;
    Status status = Status::NotReady;
    int32_t value = 0;
    std::string text;
};
Callback observe(Observation &observation) {
    return {&observation,
            [](void *ctx, Status status, const void *value) {
                auto &o = *static_cast<Observation *>(ctx);
                ++o.calls;
                o.status = status;
                if (value) {
                    mp_snapshot view{};
                    SUCCESS(o.provider->read_view(o.provider->context, value, &view));
                    o.value = view.value;
                    if (view.size)
                        o.text.assign(reinterpret_cast<const char *>(view.text), view.size);
                }
            },
            [](void *ctx) noexcept { ++static_cast<Observation *>(ctx)->drops; }};
}
void typed_core_wiring(mp_provider &p) {
    // Same genuine provider artifacts, now consumed through typed clone outcomes.
    auto port = madb::c07::probe::provider_port(p);
    Engine engine(port);
    Executor executor;
    CHECK(engine.acquire_executor(executor) == Status::Ok);
    std::string text("A\0\xF0\x9F\x98\x80", 6);
    uint64_t id = 0;
    CHECK(engine.start({41, reinterpret_cast<const unsigned char *>(text.data()), text.size()},
                       id) == Status::Ok);
    Work work;
    CHECK(engine.take_work(work) == Status::Ok);
    SUCCESS(p.increment(p.context, work.input()));
    CHECK(work.succeed() == Status::Ok);
    Observation result{p};
    ResultClaim claim;
    CHECK(engine.claim_result(id, claim) == Status::Ok);
    CHECK(claim.commit(observe(result)) == Status::Ok);
    ResultDelivery delivery;
    CHECK(engine.take_result(id, delivery) == Status::Ok);
    CHECK(delivery.invoke() == Status::Ok);
    CHECK(result.calls == 1 && result.drops == 1 && result.value == 42 && result.text == text);
    engine.close_operation(id);
    CHECK(!engine.operation_closed(id));
    work.finish();
    CHECK(engine.operation_closed(id));
    for (int mode :
         {MP_INJECT_COPY_ALLOC, MP_INJECT_COPY_INTERNAL, MP_INJECT_COPY_AFTER_ALLOC_INTERNAL}) {
        SUCCESS(p.inject(p.context, mode));
        auto status = engine.start(
            {1, reinterpret_cast<const unsigned char *>(text.data()), text.size()}, id);
        CHECK(status ==
              (mode == MP_INJECT_COPY_ALLOC ? Status::ResourceExhausted : Status::InternalFailure));
        CHECK(engine.stats().bytes == 0 && engine.stats().operations == 0 && stats(p).live == 0);
    }
    Observation event{p};
    uint64_t subscription = 0;
    CHECK(engine.observe(Stream::Event, observe(event), subscription) == Status::Ok);
    CHECK(engine.publish(Stream::Event, {7, reinterpret_cast<const unsigned char *>(text.data()),
                                         text.size()}) == Status::Ok);
    Delivery eventDelivery;
    CHECK(engine.take_delivery(subscription, eventDelivery) == Status::Ok);
    CHECK(eventDelivery.invoke() == Status::Ok);
    CHECK(event.calls == 1 && event.value == 7 && event.text == text);
    SUCCESS(p.inject(p.context, MP_INJECT_COPY_AFTER_ALLOC_INTERNAL));
    CHECK(engine.publish(Stream::Event, {8, reinterpret_cast<const unsigned char *>(text.data()),
                                         text.size()}) == Status::InternalFailure);
    CHECK(engine.stats().eventTickets == 0 && engine.stats().bytes == 0 && event.calls == 1);

    engine.stop(subscription);
    CHECK(event.drops == 1);
    engine.request_close();
    executor.finish();
    CHECK(engine.stats().closed);
    CHECK(stats(p).live == 0);
}
void exercise(const char *name, Factory create) {
    CHECK(create(nullptr) == MP_INVALID_ARGUMENT);
    mp_provider provider{};
    SUCCESS(create(&provider));
    auto &p = provider;
    CHECK(p.context && p.clone && p.read_view && p.increment && p.release && p.stats && p.inject &&
          p.destroy_context);
    std::string text("A\0\xE4\xB8\xAD\xF0\x9F\x98\x80", 9), expected = text;
    void *model = nullptr;
    SUCCESS(p.clone(p.context, snapshot(41, text), &model));
    CHECK(model);
    text[0] = 'Z';
    mp_snapshot view{};
    SUCCESS(p.read_view(p.context, model, &view));
    CHECK(view.value == 41);
    same_text(view, expected);
    SUCCESS(p.increment(p.context, model));
    SUCCESS(p.read_view(p.context, model, &view));
    CHECK(view.value == 42);
    same_text(view, expected);
    CHECK(p.destroy_context(p.context) == MP_BUSY);
    SUCCESS(p.release(p.context, model));
    SUCCESS(p.clone(p.context, snapshot(INT32_MAX, text), &model));
    CHECK(p.increment(p.context, model) == MP_DOMAIN_OVERFLOW);
    SUCCESS(p.read_view(p.context, model, &view));
    CHECK(view.value == INT32_MAX);
    same_text(view, text);
    SUCCESS(p.release(p.context, model));
    for (auto invalid : std::vector<std::string>{"\xED\xA0\x80", "\xF4\x90\x80\x80", "\xC0\x80",
                                                 std::string(1025, 'a')}) {
        model = &view;
        CHECK(p.clone(p.context, snapshot(0, invalid), &model) == MP_INVALID_ARGUMENT);
        CHECK(model == nullptr);
    }
    model = &view;
    CHECK(p.clone(p.context, {0, nullptr, 1}, &model) == MP_INVALID_ARGUMENT && model == nullptr);
    SUCCESS(p.clone(p.context, {0, nullptr, 0}, &model));
    SUCCESS(p.read_view(p.context, model, &view));
    CHECK(view.size == 0);
    SUCCESS(p.release(p.context, model));
    auto before = stats(p);
    SUCCESS(p.inject(p.context, MP_INJECT_COPY_ALLOC));
    model = &view;
    CHECK(p.clone(p.context, snapshot(1, text), &model) == MP_ALLOCATION_FAILURE && !model);
    SUCCESS(p.inject(p.context, MP_INJECT_COPY_INTERNAL));
    model = &view;
    CHECK(p.clone(p.context, snapshot(1, text), &model) == MP_INTERNAL_FAILURE && !model);
    SUCCESS(p.inject(p.context, MP_INJECT_COPY_AFTER_ALLOC_INTERNAL));
    model = &view;
    CHECK(p.clone(p.context, snapshot(1, text), &model) == MP_INTERNAL_FAILURE && !model);
    auto after = stats(p);
    CHECK(after.allocations == before.allocations + 1 && after.releases == before.releases + 1 &&
          after.live == 0);
    SUCCESS(p.clone(p.context, snapshot(11, text), &model));
    SUCCESS(p.inject(p.context, MP_INJECT_WORK_INTERNAL));
    CHECK(p.increment(p.context, model) == MP_INTERNAL_FAILURE);
    SUCCESS(p.read_view(p.context, model, &view));
    CHECK(view.value == 11);
    same_text(view, text);
    SUCCESS(p.release(p.context, model));
    CHECK(p.inject(p.context, 99) == MP_INVALID_ARGUMENT);
    std::vector<std::thread> threads;
    for (int t = 0; t < 4; ++t)
        threads.emplace_back([&] {
            for (int i = 0; i < 100; ++i) {
                void *local = nullptr;
                SUCCESS(p.clone(p.context, snapshot(i, text), &local));
                SUCCESS(p.increment(p.context, local));
                mp_snapshot borrowed{};
                SUCCESS(p.read_view(p.context, local, &borrowed));
                CHECK(borrowed.value == i + 1);
                same_text(borrowed, text);
                SUCCESS(p.release(p.context, local));
            }
        });
    for (auto &thread : threads)
        thread.join();
    typed_core_wiring(p);
    auto final = stats(p);
    CHECK(final.live == 0 && final.allocations == final.releases);
    SUCCESS(p.destroy_context(p.context));
    p = {};
    std::cout << "PASS " << name
              << " typed ABI, real model ownership, overflow, failure boundary, four-thread "
                 "stress, typed-core success/failure wiring; model allocations="
              << final.allocations << " releases=" << final.releases << std::endl;
}
int main() {
    mp_layout layout{};
    SUCCESS(mp_rust_layout(&layout));
    CHECK(layout.snapshot_size == sizeof(mp_snapshot));
    CHECK(layout.snapshot_align == alignof(mp_snapshot));
    CHECK(layout.value_offset == offsetof(mp_snapshot, value));
    CHECK(layout.text_offset == offsetof(mp_snapshot, text));
    CHECK(layout.length_offset == offsetof(mp_snapshot, size));
    CHECK(layout.provider_size == sizeof(mp_provider) &&
          layout.provider_align == alignof(mp_provider));
    std::cout << "PASS independent Rust repr(C) vs C header layout" << std::endl;
    exercise("C17", mp_c_create);
    exercise("C++17", mp_cpp_create);
    exercise("Rust", mp_rust_create);
    std::cout << "PASS 3 independent providers; no target/NAPI/profile certification" << std::endl;
}
