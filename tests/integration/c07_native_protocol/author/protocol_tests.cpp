#include "c_provider.h"
#include "protocol.hpp"
#include <array>
#include <atomic>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <iostream>
#include <limits>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
using namespace madb::c07;
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x)) {                                                                                \
            std::cerr << __FILE__ << ':' << __LINE__ << " failed: " #x << std::endl;               \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
#define OK(x) CHECK((x) == Status::Ok)
Snapshot snapshot(int64_t value = 1, const std::string &text = "abc") {
    return {value, reinterpret_cast<const unsigned char *>(text.data()), text.size()};
}
// Call sites keep their string alive throughout the synchronous call. Tests that
// retain a Publication explicitly retain their input string through allocate.
Provider port(c_provider *provider) {
    return {provider,
            [](void *p, Snapshot s) noexcept {
                return c_provider_clone(static_cast<c_provider *>(p), s.value, s.text, s.size);
            },
            [](void *p, void *value) noexcept {
                c_provider_destroy(static_cast<c_provider *>(p), value);
            }};
}
struct Rig {
    c_provider *provider = c_provider_create();
    Engine engine{port(provider)};
    Executor executor;
    Rig() { OK(engine.acquire_executor(executor)); }
    ~Rig() {
        engine.request_close();
        executor.finish();
        auto s = engine.stats();
        CHECK(s.closed && !s.operations && !s.subscriptions && !s.bytes && !s.work && !s.waiters);
        CHECK(c_provider_allocations(provider) == c_provider_frees(provider));
        c_provider_drop(provider);
    }
};
struct CallbackState {
    int calls = 0, drops = 0;
    std::vector<int32_t> values;
    std::vector<Status> outcomes;
    std::function<void(Status, const void *)> on_call;
    std::function<void()> on_drop;
};
struct CallbackBox {
    CallbackState *state;
};
Callback callback(CallbackState &state) {
    return {new CallbackBox{&state},
            [](void *context, Status status, const void *value) {
                auto &s = *static_cast<CallbackBox *>(context)->state;
                ++s.calls;
                s.outcomes.push_back(status);
                if (value)
                    s.values.push_back(c_payload_value(value));
                if (s.on_call)
                    s.on_call(status, value);
            },
            [](void *context) noexcept {
                auto *box = static_cast<CallbackBox *>(context);
                auto *s = box->state;
                ++s->drops;
                delete box;
                if (s->on_drop)
                    s->on_drop();
            }};
}
void close_sub(Engine &engine, uint64_t id) {
    engine.stop(id);
    CHECK(engine.subscription_closed(id));
}
void consume(Engine &engine, uint64_t id) {
    Delivery d;
    OK(engine.take_delivery(id, d));
    OK(d.invoke());
}
void finish_success(Engine &engine) {
    Work w;
    OK(engine.take_work(w));
    CHECK(c_provider_increment(w.input()));
    OK(w.succeed());
    w.finish();
}
void snapshot_validation() {
    Rig r;
    std::string text("a\0\xF0\x9F\x98\x80", 6);
    uint64_t id = 0;
    OK(r.engine.start(snapshot(41, text), id));
    Work w;
    OK(r.engine.take_work(w));
    CHECK(c_payload_size(w.input()) == 6 &&
          !std::memcmp(c_payload_text(w.input()), text.data(), 6));
    CHECK(c_provider_increment(w.input()));
    OK(w.succeed());
    w.finish();
    CallbackState s;
    ResultClaim claim;
    OK(r.engine.claim_result(id, claim));
    OK(claim.commit(callback(s)));
    ResultDelivery result;
    OK(r.engine.take_result(id, result));
    OK(result.invoke());
    CHECK(s.values == std::vector<int32_t>{42});
    r.engine.close_operation(id);
    for (const auto &invalid : std::vector<std::string>{
             "\xC0\x80", "\xED\xA0\x80", "\xF4\x90\x80\x80", "\xE2\x82", std::string(1025, 'a')}) {
        CHECK(r.engine.start(snapshot(0, invalid), id) == Status::InvalidArgument);
    }
    CHECK(r.engine.start(snapshot(int64_t(INT32_MAX) + 1), id) == Status::InvalidArgument);
    CHECK(r.engine.start(snapshot(int64_t(INT32_MIN) - 1), id) == Status::InvalidArgument);
    CHECK(!valid_snapshot({0, nullptr, 1}));
    CHECK(c_provider_calls(r.provider) == 1);
}
void terminal_and_work_are_separate() {
    Rig r;
    uint64_t id = 0;
    OK(r.engine.start(snapshot(), id));
    Work w;
    OK(r.engine.take_work(w));
    CallbackState s;
    ResultClaim claim;
    OK(r.engine.claim_result(id, claim));
    OK(claim.commit(callback(s)));
    OK(r.engine.cancel(id));
    CHECK(w.cancelled());
    CHECK(w.succeed() == Status::Cancelled);
    ResultDelivery result;
    OK(r.engine.take_result(id, result));
    CHECK(result.invoke() == Status::Cancelled);
    r.engine.close_operation(id);
    CHECK(!r.engine.operation_closed(id));
    CHECK(r.engine.stats().work == 1);
    w.finish();
    CHECK(r.engine.operation_closed(id));
    CHECK(s.calls == 1 && s.drops == 1);
}
void successful_terminal_survives_cancel_and_close() {
    Rig r;
    uint64_t id = 0;
    OK(r.engine.start(snapshot(), id));
    Work w;
    OK(r.engine.take_work(w));
    CHECK(c_provider_increment(w.input()));
    OK(w.succeed());
    OK(r.engine.cancel(id));
    CHECK(r.engine.winner(id) == Winner::Success);
    ResultClaim claim;
    OK(r.engine.claim_result(id, claim));
    r.engine.close_operation(id);
    CallbackState s;
    OK(claim.commit(callback(s)));
    ResultDelivery result;
    OK(r.engine.take_result(id, result));
    CHECK(!r.engine.operation_closed(id));
    OK(result.invoke());
    CHECK(s.values == std::vector<int32_t>{2});
    CHECK(!r.engine.operation_closed(id));
    w.finish();
    CHECK(r.engine.operation_closed(id));
}
void unclaimed_and_reserved_rollback() {
    Rig r;
    uint64_t id = 0;
    OK(r.engine.start(snapshot(), id));
    finish_success(r.engine);
    r.engine.close_operation(id);
    CHECK(r.engine.operation_closed(id));
    CHECK(!c_provider_live(r.provider));
    OK(r.engine.start(snapshot(), id));
    finish_success(r.engine);
    ResultClaim first, second;
    OK(r.engine.claim_result(id, first));
    CHECK(r.engine.claim_result(id, second) == Status::AlreadyConsumed);
    first.rollback();
    CHECK(r.engine.stats().waiters == 0);
    OK(r.engine.claim_result(id, second));
    r.engine.close_operation(id);
    CHECK(!r.engine.operation_closed(id));
    second.rollback();
    CHECK(r.engine.operation_closed(id));
    CHECK(!r.engine.stats().waiters);
}
void operation_slots_and_single_worker() {
    Rig r;
    std::array<uint64_t, 8> ids{};
    for (auto &id : ids)
        OK(r.engine.start(snapshot(), id));
    uint64_t rejected = 0;
    CHECK(r.engine.start(snapshot(), rejected) == Status::ResourceExhausted);
    Work first, second;
    OK(r.engine.take_work(first));
    CHECK(r.engine.take_work(second) == Status::NotReady);
    OK(r.engine.cancel(ids[1]));
    r.engine.close_operation(ids[1]);
    CHECK(r.engine.operation_closed(ids[1]));
    OK(r.engine.start(snapshot(), rejected)); // Only explicitly closed slot can be reused.
    first.finish();
    r.engine.request_close();
    CHECK(r.engine.stats().work == 0);
}
void state_latest_and_entered_stop() {
    Rig r;
    CallbackState s;
    uint64_t id = 0;
    OK(r.engine.observe(Stream::State, callback(s), id));
    Delivery empty;
    CHECK(r.engine.take_delivery(id, empty) == Status::NotReady);
    OK(r.engine.publish(Stream::State, snapshot(1)));
    OK(r.engine.publish(Stream::State, snapshot(2)));
    CHECK(r.engine.stats().stateTickets == 1 && c_provider_live(r.provider) == 1);
    s.on_call = [&](Status, const void *) {
        CHECK(r.engine.stats().entered == 1 && r.engine.stats().stateTickets == 1);
        OK(r.engine.publish(Stream::State, snapshot(3)));
        CHECK(r.engine.stats().stateTickets == 2);
        r.engine.stop(id);
        CHECK(!r.engine.subscription_closed(id));
        CHECK(s.drops == 0);
        CHECK(r.engine.stats().stateTickets ==
              1); // Pending released; entered retains its own payload.
    };
    consume(r.engine, id);
    CHECK(s.values == std::vector<int32_t>{2});
    CHECK(s.drops == 1);
    CHECK(r.engine.subscription_closed(id));
}
void acquired_delivery_is_gated() {
    Rig r;
    CallbackState s;
    uint64_t id = 0;
    OK(r.engine.observe(Stream::Event, callback(s), id));
    OK(r.engine.publish(Stream::Event, snapshot()));
    Delivery d;
    OK(r.engine.take_delivery(id, d));
    r.engine.stop(id);
    CHECK(!r.engine.subscription_closed(id));
    CHECK(s.drops == 0);
    CHECK(d.invoke() == Status::Closing);
    CHECK(s.calls == 0 && s.drops == 1);
    CHECK(r.engine.subscription_closed(id));
}
void source_before_enqueue_and_order() {
    Rig r;
    CallbackState s;
    uint64_t id = 0;
    OK(r.engine.observe(Stream::Event, callback(s), id));
    const std::string text = "abc";
    Publication first, second;
    OK(r.engine.reserve_publish(Stream::Event, snapshot(1, text), first));
    OK(r.engine.reserve_publish(Stream::Event, snapshot(2, text), second));
    OK(second.allocate(snapshot(2, text)));
    OK(second.commit());
    Delivery d;
    CHECK(r.engine.take_delivery(id, d) == Status::NotReady);
    OK(first.allocate(snapshot(1, text)));
    OK(first.commit());
    consume(r.engine, id);
    consume(r.engine, id);
    CHECK(s.values == (std::vector<int32_t>{1, 2}));
    Publication held;
    OK(r.engine.reserve_publish(Stream::Event, snapshot(3, text), held));
    r.engine.stop(id);
    CHECK(!r.engine.subscription_closed(id) && s.drops == 0);
    CHECK(r.engine.stats().sources == 1 && r.engine.stats().bytes == 3);
    OK(held.allocate(snapshot(3, text)));
    CHECK(held.commit() == Status::Closing);
    CHECK(r.engine.subscription_closed(id) && s.drops == 1 && !r.engine.stats().sources);
}
void event_atomic_fanout_and_item_saturation() {
    Rig r;
    std::array<CallbackState, 3> states;
    std::array<uint64_t, 3> ids{};
    for (size_t i = 0; i < 3; ++i)
        OK(r.engine.observe(Stream::Event, callback(states[i]), ids[i]));
    OK(r.engine.publish(Stream::Event, snapshot(1)));
    OK(r.engine.publish(Stream::Event, snapshot(2)));
    CHECK(r.engine.stats().eventTickets == 6);
    CHECK(r.engine.publish(Stream::Event, snapshot(3)) == Status::ResourceExhausted);
    for (auto id : ids) {
        consume(r.engine, id);
        consume(r.engine, id);
    }
    for (auto &s : states)
        CHECK(s.values == (std::vector<int32_t>{1, 2}));
    for (auto id : ids)
        close_sub(r.engine, id);
    CallbackState single;
    uint64_t id = 0;
    OK(r.engine.observe(Stream::Event, callback(single), id));
    for (int i = 0; i < 8; ++i)
        OK(r.engine.publish(Stream::Event, snapshot(i)));
    single.on_call = [&](Status, const void *) {
        CHECK(r.engine.stats().eventTickets == 8 && r.engine.stats().entered == 1);
        CHECK(r.engine.publish(Stream::Event, snapshot(9)) == Status::ResourceExhausted);
        r.engine.stop(id);
        CHECK(r.engine.stats().eventTickets == 1);
    };
    consume(r.engine, id);
    CHECK(r.engine.subscription_closed(id));
}
void byte_budget_and_state_staging() {
    Rig r;
    CallbackState events;
    uint64_t event = 0;
    OK(r.engine.observe(Stream::Event, callback(events), event));
    const std::string large(1024, 'x');
    for (int i = 0; i < 4; ++i)
        OK(r.engine.publish(Stream::Event, snapshot(i, large)));
    CHECK(r.engine.stats().bytes == 4096 && r.engine.stats().eventTickets == 4);
    CHECK(r.engine.publish(Stream::Event, snapshot(5)) == Status::ResourceExhausted);
    close_sub(r.engine, event);
    std::array<CallbackState, 4> states;
    std::array<uint64_t, 4> ids{};
    for (size_t i = 0; i < 4; ++i)
        OK(r.engine.observe(Stream::State, callback(states[i]), ids[i]));
    OK(r.engine.publish(Stream::State, snapshot(7, large)));
    CHECK(r.engine.stats().bytes == 4096);
    CHECK(r.engine.publish(Stream::State, snapshot(8, large)) == Status::ResourceExhausted);
    for (size_t i = 0; i < 4; ++i) {
        consume(r.engine, ids[i]);
        CHECK(states[i].values == std::vector<int32_t>{7});
        close_sub(r.engine, ids[i]);
    }
}
void partial_clone_failure_is_atomic() {
    Rig r;
    std::array<CallbackState, 3> states;
    std::array<uint64_t, 3> ids{};
    for (size_t i = 0; i < 3; ++i)
        OK(r.engine.observe(Stream::State, callback(states[i]), ids[i]));
    OK(r.engine.publish(Stream::State, snapshot(1)));
    c_provider_fail_at(r.provider, c_provider_calls(r.provider) + 2);
    CHECK(r.engine.publish(Stream::State, snapshot(2)) == Status::ResourceExhausted);
    CHECK(c_provider_allocations(r.provider) == 4 && c_provider_frees(r.provider) == 1);
    CHECK(r.engine.stats().stateTickets == 3 && r.engine.stats().bytes == 9 &&
          !r.engine.stats().sources && !r.engine.stats().admissions);
    for (size_t i = 0; i < 3; ++i) {
        consume(r.engine, ids[i]);
        CHECK(states[i].values == std::vector<int32_t>{1});
        close_sub(r.engine, ids[i]);
    }
    uint64_t rejected = 0;
    c_provider_fail_at(r.provider, c_provider_calls(r.provider) + 1);
    CHECK(r.engine.start(snapshot(), rejected) == Status::ResourceExhausted);
    CHECK(!r.engine.stats().operations && !r.engine.stats().work && !r.engine.stats().bytes);
}
void waiter_full_rollback_and_close_control() {
    Rig r;
    uint64_t first = 0, second = 0;
    OK(r.engine.start(snapshot(), first));
    OK(r.engine.start(snapshot(), second));
    Work held;
    OK(r.engine.take_work(held));
    std::array<CloseWaiter, 32> waiters;
    std::array<CallbackState, 32> callbacks;
    for (size_t i = 0; i < 32; ++i)
        OK(r.engine.watch_operation_close(first, callback(callbacks[i]), waiters[i]));
    CHECK(r.engine.stats().waiters == 32);
    ResultClaim claim;
    CHECK(r.engine.claim_result(second, claim) == Status::ResourceExhausted);
    waiters[0].discard();
    CHECK(callbacks[0].drops == 1);
    OK(r.engine.claim_result(second, claim));
    claim.rollback();
    CHECK(r.engine.stats().waiters == 31);
    CallbackState last, rejected;
    CloseWaiter one, failed;
    OK(r.engine.watch_operation_close(first, callback(last), one));
    CHECK(r.engine.watch_close(callback(rejected), failed) == Status::ResourceExhausted);
    CHECK(r.engine.stats().closing && !r.engine.stats().closed && rejected.drops == 1);
    held.finish();
    r.executor.finish();
    CHECK(r.engine.stats().closed && r.engine.stats().waiters == 32);
    for (size_t i = 1; i < 32; ++i) {
        CHECK(waiters[i].ready());
        OK(waiters[i].invoke());
    }
    CHECK(one.ready());
    OK(one.invoke());
    CHECK(!r.engine.stats().waiters);
}
void sub_quota_and_callback_failure() {
    Rig r;
    std::array<CallbackState, 9> states;
    std::array<uint64_t, 8> ids{};
    for (size_t i = 0; i < 8; ++i)
        OK(r.engine.observe(i % 2 ? Stream::Event : Stream::State, callback(states[i]), ids[i]));
    uint64_t ninth = 0;
    CHECK(r.engine.observe(Stream::Event, callback(states[8]), ninth) == Status::ResourceExhausted);
    CHECK(states[8].drops == 1);
    for (auto id : ids)
        close_sub(r.engine, id);
    CallbackState failure, close;
    uint64_t id = 0;
    failure.on_call = [](Status, const void *) { throw std::runtime_error("listener failure"); };
    OK(r.engine.observe(Stream::Event, callback(failure), id));
    OK(r.engine.publish(Stream::Event, snapshot()));
    Delivery d;
    OK(r.engine.take_delivery(id, d));
    CHECK(d.invoke() == Status::CallbackFailure);
    CHECK(r.engine.subscription_closed(id) && failure.drops == 1);
    CloseWaiter waiter;
    OK(r.engine.watch_subscription_close(id, callback(close), waiter));
    CHECK(waiter.invoke() == Status::CallbackFailure && close.calls == 1 && close.drops == 1);
}
void external_destructors_run_without_protocol_lock() {
    Rig r;
    size_t released = 0;
    struct Context {
        Rig *r;
        size_t *count;
    } context{&r, &released};
    c_provider_hooks(
        r.provider,
        [](void *ctx) {
            auto *c = static_cast<Context *>(ctx);
            CHECK(c->r->engine.stats().bytes >= 3);
        },
        [](void *ctx) {
            auto *c = static_cast<Context *>(ctx);
            CHECK(c->r->engine.stats().bytes >= 3);
            ++*c->count;
        },
        &context);
    CallbackState s;
    uint64_t id = 0;
    s.on_drop = [&] {
        CHECK(r.engine.stats().subscriptions == 1);
        CHECK(!r.engine.stats().bytes);
    };
    OK(r.engine.observe(Stream::Event, callback(s), id));
    OK(r.engine.publish(Stream::Event, snapshot()));
    close_sub(r.engine, id);
    CHECK(released == 1 && s.drops == 1);
    c_provider_hooks(r.provider, nullptr, nullptr, nullptr);
}
void concurrent_cancel_claim() {
    for (int i = 0; i < 100; ++i) {
        Rig r;
        uint64_t id = 0;
        OK(r.engine.start(snapshot(), id));
        Work w;
        OK(r.engine.take_work(w));
        std::atomic<bool> go{false};
        std::thread cancel([&] {
            while (!go.load(std::memory_order_acquire))
                std::this_thread::yield();
            OK(r.engine.cancel(id));
        });
        go.store(true, std::memory_order_release);
        auto status = w.succeed();
        cancel.join();
        auto winner = r.engine.winner(id);
        CHECK(winner == Winner::Success || winner == Winner::Cancelled);
        CHECK((winner == Winner::Success && status == Status::Ok) ||
              (winner == Winner::Cancelled && status == Status::Cancelled));
        w.finish();
        r.engine.close_operation(id);
    }
}
void allocator_reentrant_close() {
    Rig r;
    c_provider_hooks(
        r.provider, [](void *context) { static_cast<Engine *>(context)->request_close(); }, nullptr,
        &r.engine);
    uint64_t id = 0;
    CHECK(r.engine.start(snapshot(), id) == Status::Closing);
    CHECK(!r.engine.stats().operations && !r.engine.stats().bytes && !r.engine.stats().admissions);
    CHECK(c_provider_allocations(r.provider) == 1 && c_provider_frees(r.provider) == 1);
    c_provider_hooks(r.provider, nullptr, nullptr, nullptr);
}
void retained_close_error_survives_slot_reuse() {
    Rig r;
    CallbackState failing, ack, replacement;
    uint64_t old = 0, current = 0;
    CloseWaiter waiter;
    failing.on_call = [&](Status, const void *) {
        OK(r.engine.watch_subscription_close(old, callback(ack), waiter));
        CHECK(!waiter.ready() && r.engine.stats().waiters == 1);
        throw std::runtime_error("listener failure after close observer registration");
    };
    OK(r.engine.observe(Stream::Event, callback(failing), old));
    OK(r.engine.publish(Stream::Event, snapshot()));
    Delivery delivery;
    OK(r.engine.take_delivery(old, delivery));
    CHECK(delivery.invoke() == Status::CallbackFailure);
    CHECK(waiter.ready() && r.engine.stats().waiters == 1);
    OK(r.engine.observe(Stream::Event, callback(replacement), current));
    CHECK(current != old);
    r.engine.stop(old);
    CHECK(!r.engine.subscription_closed(current));
    CHECK(waiter.invoke() == Status::CallbackFailure);
    CHECK(ack.outcomes == std::vector<Status>{Status::CallbackFailure});
    OK(r.engine.publish(Stream::Event, snapshot(99)));
    consume(r.engine, current);
    CHECK(replacement.values == std::vector<int32_t>{99});
    close_sub(r.engine, current);
}
void ready_records_stay_bounded_and_noop_is_empty() {
    Rig r;
    uint64_t first = 0, second = 0;
    OK(r.engine.start(snapshot(), first));
    Work work;
    OK(r.engine.take_work(work));
    std::array<CloseWaiter, 32> observers;
    std::array<CallbackState, 32> states;
    for (size_t i = 0; i < 32; ++i)
        OK(r.engine.watch_operation_close(first, callback(states[i]), observers[i]));
    work.finish();
    CHECK(r.engine.operation_closed(first) && r.engine.stats().waiters == 32);
    OK(r.engine.start(snapshot(), second));
    CHECK(second != first);
    OK(r.engine.take_work(work));
    CHECK(r.engine.cancel(first) == Status::Closed);
    CHECK(r.engine.winner(second) == Winner::Pending);
    CallbackState rejected;
    CloseWaiter failed;
    CHECK(r.engine.watch_operation_close(second, callback(rejected), failed) ==
          Status::ResourceExhausted);
    CHECK(rejected.drops == 1 && r.engine.stats().waiters == 32);
    states[0].on_call = [&](Status, const void *) { CHECK(r.engine.stats().waiters == 32); };
    states[0].on_drop = [&] { CHECK(r.engine.stats().waiters == 32); };
    OK(observers[0].invoke());
    CHECK(r.engine.stats().waiters == 31);
    for (size_t i = 1; i < 32; ++i)
        observers[i].discard();
    work.finish();
    CallbackState immediate;
    CloseWaiter valueOnly;
    OK(r.engine.watch_operation_close(second, callback(immediate), valueOnly));
    CHECK(immediate.calls == 1 && immediate.drops == 1 && valueOnly.ready() &&
          !r.engine.stats().waiters);
    OK(valueOnly.invoke());
    CHECK(immediate.calls == 1);
    std::array<Publication, 100> noops;
    for (auto &publication : noops)
        OK(r.engine.reserve_publish(Stream::Event, snapshot(), publication));
    CHECK(!r.engine.stats().admissions && !r.engine.stats().bytes && !r.engine.stats().sources);
    r.engine.request_close();
    r.executor.finish();
    CHECK(r.engine.stats().closed);
    for (auto &publication : noops)
        OK(publication.commit());
}
void domain_failure_owns_no_business_in_protocol() {
    Rig r;
    uint64_t id = 0;
    OK(r.engine.start(snapshot(INT32_MAX), id));
    Work work;
    OK(r.engine.take_work(work));
    CHECK(!c_provider_increment(work.input()));
    OK(work.fail());
    OK(r.engine.cancel(id));
    CHECK(r.engine.winner(id) == Winner::DomainFailure && r.engine.stats().work == 1 &&
          !r.engine.stats().bytes);
    CallbackState state;
    ResultClaim claim;
    OK(r.engine.claim_result(id, claim));
    OK(claim.commit(callback(state)));
    ResultDelivery result;
    OK(r.engine.take_result(id, result));
    CHECK(result.invoke() == Status::DomainFailure);
    CHECK(state.values.empty() && state.calls == 1 && state.drops == 1);
    r.engine.close_operation(id);
    CHECK(!r.engine.operation_closed(id));
    work.finish();
    CHECK(r.engine.operation_closed(id));
}
void state_pre_entry_remains_replaceable() {
    Rig r;
    CallbackState state;
    uint64_t id = 0;
    OK(r.engine.observe(Stream::State, callback(state), id));
    OK(r.engine.publish(Stream::State, snapshot(1)));
    Delivery delivery;
    OK(r.engine.take_delivery(id, delivery));
    CHECK(r.engine.stats().stateTickets == 1 && !r.engine.stats().entered);
    OK(r.engine.publish(Stream::State, snapshot(2)));
    CHECK(r.engine.stats().stateTickets == 1 && !r.engine.stats().entered);
    OK(delivery.invoke());
    CHECK(state.values == std::vector<int32_t>{2});
    OK(r.engine.publish(Stream::State, snapshot(3)));
    OK(r.engine.take_delivery(id, delivery));
    r.engine.stop(id);
    CHECK(!r.engine.subscription_closed(id) && state.drops == 0);
    CHECK(!r.engine.stats().stateTickets && !r.engine.stats().bytes);
    CHECK(delivery.invoke() == Status::Closing);
    CHECK(state.calls == 1 && state.drops == 1);
    CHECK(r.engine.subscription_closed(id));
}
int main() {
    const std::pair<const char *, void (*)()> cases[] = {
        {"retained_close_error_survives_slot_reuse", retained_close_error_survives_slot_reuse},
        {"ready_records_stay_bounded_and_noop_is_empty",
         ready_records_stay_bounded_and_noop_is_empty},
        {"domain_failure_owns_no_business_in_protocol",
         domain_failure_owns_no_business_in_protocol},
        {"state_pre_entry_remains_replaceable", state_pre_entry_remains_replaceable},
        {"snapshot_validation", snapshot_validation},
        {"terminal_and_work_are_separate", terminal_and_work_are_separate},
        {"successful_terminal_survives_cancel_and_close",
         successful_terminal_survives_cancel_and_close},
        {"unclaimed_and_reserved_rollback", unclaimed_and_reserved_rollback},
        {"operation_slots_and_single_worker", operation_slots_and_single_worker},
        {"state_latest_and_entered_stop", state_latest_and_entered_stop},
        {"acquired_delivery_is_gated", acquired_delivery_is_gated},
        {"source_before_enqueue_and_order", source_before_enqueue_and_order},
        {"event_atomic_fanout_and_item_saturation", event_atomic_fanout_and_item_saturation},
        {"byte_budget_and_state_staging", byte_budget_and_state_staging},
        {"partial_clone_failure_is_atomic", partial_clone_failure_is_atomic},
        {"waiter_full_rollback_and_close_control", waiter_full_rollback_and_close_control},
        {"sub_quota_and_callback_failure", sub_quota_and_callback_failure},
        {"external_destructors_run_without_protocol_lock",
         external_destructors_run_without_protocol_lock},
        {"concurrent_cancel_claim", concurrent_cancel_claim},
        {"allocator_reentrant_close", allocator_reentrant_close},
    };
    for (auto entry : cases) {
        entry.second();
        std::cout << "PASS " << entry.first << std::endl;
    }
    std::cout << "PASS " << std::size(cases) << " cases (including 100 cancellation races)"
              << std::endl;
}
