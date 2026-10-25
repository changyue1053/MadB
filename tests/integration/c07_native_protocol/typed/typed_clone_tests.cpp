#include "c_provider.h"
#include "protocol.hpp"
#include <cstdlib>
#include <iostream>
#include <string>
#include <vector>
using namespace madb::c07;
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x)) {                                                                                \
            std::cerr << __LINE__ << " failed " #x << std::endl;                                   \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
struct TestProvider {
    c_provider *allocations = c_provider_create();
    Engine *engine = nullptr;
    CloneStatus failure = CloneStatus::Ok;
    size_t calls = 0, failAt = 1;
    bool withPayload = true, closeDuringClone = false;
    size_t destroys = 0;
    ~TestProvider() {
        CHECK(c_provider_allocations(allocations) == c_provider_frees(allocations));
        c_provider_drop(allocations);
    }
};
Provider port(TestProvider &p) {
    return {&p,
            [](void *ctx, Snapshot s) noexcept {
                auto &p = *static_cast<TestProvider *>(ctx);
                bool fail = ++p.calls == p.failAt;
                void *payload = (!fail || p.withPayload)
                                    ? c_provider_clone(p.allocations, s.value, s.text, s.size)
                                    : nullptr;
                if (!fail || p.withPayload)
                    CHECK(payload);
                if (fail && p.closeDuringClone)
                    p.engine->request_close();
                return CloneResult{fail ? p.failure : CloneStatus::Ok, payload};
            },
            [](void *ctx, void *payload) noexcept {
                auto &p = *static_cast<TestProvider *>(ctx);
                CHECK(p.engine->stats().bytes >=
                      c_payload_size(payload)); // Lock-free provider entry and permit still held.
                ++p.destroys;
                c_provider_destroy(p.allocations, payload);
            }};
}
Snapshot input(int64_t value = 1) {
    static const unsigned char text[] = {'a', 'b', 'c'};
    return {value, text, 3};
}
void start_failure(CloneStatus failure, bool payload, Status expected, bool close) {
    TestProvider provider;
    provider.failure = failure;
    provider.withPayload = payload;
    provider.closeDuringClone = close;
    Engine engine(port(provider));
    provider.engine = &engine;
    Executor executor;
    CHECK(engine.acquire_executor(executor) == Status::Ok);
    uint64_t id = 0;
    CHECK(engine.start(input(), id) == expected);
    CHECK(id == 0);
    auto stats = engine.stats();
    CHECK(!stats.operations && !stats.bytes && !stats.work && !stats.admissions);
    CHECK(provider.destroys == (payload ? 1u : 0u));
    CHECK(!c_provider_live(provider.allocations));
    engine.request_close();
    executor.finish();
    CHECK(engine.stats().closed);
}
struct Observer {
    std::vector<int32_t> values;
    size_t drops = 0;
};
Callback callback(Observer &observer) {
    return {&observer,
            [](void *ctx, Status status, const void *value) {
                CHECK(status == Status::Ok);
                static_cast<Observer *>(ctx)->values.push_back(c_payload_value(value));
            },
            [](void *ctx) noexcept { ++static_cast<Observer *>(ctx)->drops; }};
}
void fanout_failure_owned() {
    TestProvider provider;
    provider.failAt = 0;
    Engine engine(port(provider));
    provider.engine = &engine;
    Executor executor;
    CHECK(engine.acquire_executor(executor) == Status::Ok);
    Observer observations[3];
    uint64_t ids[3]{};
    for (int i = 0; i < 3; ++i)
        CHECK(engine.observe(Stream::State, callback(observations[i]), ids[i]) == Status::Ok);
    CHECK(engine.publish(Stream::State, input(1)) == Status::Ok);
    provider.failAt = provider.calls + 2;
    provider.failure = CloneStatus::InternalFailure;
    provider.withPayload = true;
    CHECK(engine.publish(Stream::State, input(2)) == Status::InternalFailure);
    CHECK(c_provider_allocations(provider.allocations) == 5 &&
          c_provider_frees(provider.allocations) == 2 &&
          c_provider_live(provider.allocations) == 3);
    auto stats = engine.stats();
    CHECK(stats.bytes == 9 && stats.stateTickets == 3 && !stats.sources && !stats.admissions);
    for (int i = 0; i < 3; ++i) {
        Delivery d;
        CHECK(engine.take_delivery(ids[i], d) == Status::Ok);
        CHECK(d.invoke() == Status::Ok);
        CHECK(observations[i].values == std::vector<int32_t>{1});
        engine.stop(ids[i]);
        CHECK(observations[i].drops == 1);
    }
    engine.request_close();
    executor.finish();
    CHECK(engine.stats().closed);
}
int main() {
    for (bool owned : {false, true}) {
        start_failure(CloneStatus::InvalidArgument, owned, Status::InvalidArgument, false);
        start_failure(CloneStatus::AllocationFailure, owned, Status::ResourceExhausted, false);
        start_failure(CloneStatus::InternalFailure, owned, Status::InternalFailure, false);
        start_failure(static_cast<CloneStatus>(99), owned, Status::InternalFailure, false);
    }
    start_failure(CloneStatus::Ok, false, Status::InternalFailure, false);
    start_failure(CloneStatus::InternalFailure, true, Status::InternalFailure, true);
    fanout_failure_owned();
    std::cout << "PASS 11 typed clone rollback cases; real owned-failure release; existing 20 "
                 "regressions run separately"
              << std::endl;
}
