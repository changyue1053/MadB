#include "c_provider.h"
#include "protocol.hpp"
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <functional>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
using namespace madb::c07;
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x))                                                                                  \
            throw std::runtime_error(std::string(__func__) + ":" + std::to_string(__LINE__) +      \
                                     ": " #x);                                                     \
    } while (0)
CloneResult clone(void *c, Snapshot s) noexcept {
    auto *owned = cp_clone(static_cast<cp_context *>(c), s.value, s.text, s.size);
    return {owned ? CloneStatus::Ok : CloneStatus::AllocationFailure, owned};
}
void destroy(void *c, void *p) noexcept { cp_destroy(static_cast<cp_context *>(c), p); }
struct Context {
    cp_context *p = cp_create();
    ~Context() { cp_delete(p); }
};
struct Fixture {
    Context context;
    Engine e{Provider{context.p, clone, destroy}};
    void balanced(size_t expected) {
        auto c = cp_read(context.p);
        CHECK(c.allocated == expected && c.released == expected && c.live == 0 &&
              c.bad_release == 0);
    }
};
Snapshot snap(int64_t value, const std::string &text) {
    return {value, reinterpret_cast<const unsigned char *>(text.data()), text.size()};
}
struct Gate {
    std::mutex mutex;
    std::condition_variable cv;
    bool entered = false, open = false;
    bool arrive() {
        std::unique_lock<std::mutex> lock(mutex);
        entered = true;
        cv.notify_all();
        return cv.wait_for(lock, std::chrono::seconds(3), [&] { return open; });
    }
    void wait() {
        std::unique_lock<std::mutex> lock(mutex);
        CHECK(cv.wait_for(lock, std::chrono::seconds(3), [&] { return entered; }));
    }
    void release() {
        std::lock_guard<std::mutex> lock(mutex);
        open = true;
        cv.notify_all();
    }
};
struct Joining {
    Gate *gate;
    std::thread thread;
    ~Joining() {
        gate->release();
        if (thread.joinable())
            thread.join();
    }
    void join() {
        gate->release();
        thread.join();
    }
};
struct Seen {
    Status status;
    bool hasPayload;
    int64_t value;
    std::string text;
};
struct TraceData {
    std::mutex mutex;
    std::vector<Seen> calls;
    std::atomic<int> created{0}, destroyed{0};
};
void record(TraceData &t, Status status, const void *payload) {
    int64_t value = 0;
    const unsigned char *text = nullptr;
    size_t size = 0;
    bool has = payload != nullptr;
    if (has)
        CHECK(cp_value(payload, &value, &text, &size));
    std::lock_guard<std::mutex> lock(t.mutex);
    t.calls.push_back(
        {status, has, value,
         has ? std::string(reinterpret_cast<const char *>(text), size) : std::string()});
}
struct Trace {
    std::shared_ptr<TraceData> state = std::make_shared<TraceData>();
    std::atomic<int> &created = state->created;
    std::atomic<int> &destroyed = state->destroyed;
    size_t count() {
        std::lock_guard<std::mutex> lock(state->mutex);
        return state->calls.size();
    }
    Seen at(size_t n) {
        std::lock_guard<std::mutex> lock(state->mutex);
        return state->calls.at(n);
    }
};
struct CallbackContext {
    std::shared_ptr<TraceData> trace;
    Gate *gate;
    bool throwing;
    Engine *query;
};
Callback callback(Trace &trace, Gate *gate = nullptr, bool throwing = false,
                  Engine *query = nullptr) {
    ++trace.created;
    return Callback(
        new CallbackContext{trace.state, gate, throwing, query},
        [](void *raw, Status status, const void *payload) {
            auto &c = *static_cast<CallbackContext *>(raw);
            if (c.query)
                (void)c.query->stats();
            record(*c.trace, status, payload);
            if (c.gate)
                CHECK(c.gate->arrive());
            if (c.throwing)
                throw std::runtime_error("intentional callback failure");
        },
        [](void *raw) noexcept {
            auto *c = static_cast<CallbackContext *>(raw);
            if (c->query)
                (void)c->query->stats();
            ++c->trace->destroyed;
            delete c;
        });
}
void finish_engine(Fixture &f, Executor &ex) {
    f.e.request_close();
    ex.finish();
    CHECK(f.e.stats().closed);
}
void vector_and_c_algorithm() {
    Fixture f;
    uint64_t id = 0;
    const std::string text("A\0B", 3);
    CHECK(f.e.start(snap(3, text), id) == Status::Closed);
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    const unsigned char invalid[] = {0xc0, 0x80};
    CHECK(f.e.start({3, invalid, 2}, id) == Status::InvalidArgument);
    CHECK(cp_read(f.context.p).attempts == 0);
    CHECK(f.e.start(snap(3, text), id) == Status::Ok);
    ResultClaim claim;
    CHECK(f.e.claim_result(id, claim) == Status::Ok);
    Trace results;
    CHECK(claim.commit(callback(results, nullptr, false, &f.e)) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    CHECK(cp_apply(work.input()) == 1);
    CHECK(work.succeed() == Status::Ok);
    ResultDelivery result;
    CHECK(f.e.take_result(id, result) == Status::Ok);
    CHECK(result.invoke() == Status::Ok);
    CHECK(results.count() == 1 && results.at(0).value == 10 && results.at(0).text == text);
    CHECK(results.destroyed == 1);
    f.e.close_operation(id);
    CHECK(!f.e.operation_closed(id));
    work.finish();
    CHECK(f.e.operation_closed(id));
    finish_engine(f, ex);
    f.balanced(1);
}
void terminal_before_work_exit() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t id = 0;
    CHECK(f.e.start(snap(11, "x"), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    Trace resultTrace, closeTrace;
    ResultClaim claim;
    CHECK(f.e.claim_result(id, claim) == Status::Ok);
    CHECK(claim.commit(callback(resultTrace)) == Status::Ok);
    CHECK(work.succeed() == Status::Ok);
    ResultDelivery result;
    CHECK(f.e.take_result(id, result) == Status::Ok);
    CHECK(result.invoke() == Status::Ok);
    CHECK(resultTrace.at(0).value == 11);
    CloseWaiter waiter;
    CHECK(f.e.watch_operation_close(id, callback(closeTrace), waiter) == Status::Ok);
    CHECK(!waiter.ready() && closeTrace.count() == 0);
    CHECK(waiter.invoke() == Status::NotReady);
    work.finish();
    CHECK(waiter.ready());
    CHECK(waiter.invoke() == Status::Ok);
    CHECK(closeTrace.count() == 1 && closeTrace.destroyed == 1);
    finish_engine(f, ex);
    f.balanced(1);
}
void cancel_and_late_result() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t id;
    CHECK(f.e.start(snap(23, "late"), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    Trace t;
    ResultClaim claim;
    CHECK(f.e.claim_result(id, claim) == Status::Ok);
    CHECK(claim.commit(callback(t)) == Status::Ok);
    CHECK(f.e.cancel(id) == Status::Ok);
    CHECK(work.cancelled());
    ResultDelivery result;
    CHECK(f.e.take_result(id, result) == Status::Ok);
    CHECK(result.invoke() == Status::Cancelled);
    CHECK(t.count() == 1 && !t.at(0).hasPayload);
    CHECK(cp_read(f.context.p).live == 1);
    CHECK(cp_apply(work.input()));
    CHECK(work.succeed() == Status::Cancelled);
    CHECK(cp_read(f.context.p).live == 0);
    CHECK(f.e.winner(id) == Winner::Cancelled);
    work.finish();
    finish_engine(f, ex);
    f.balanced(1);
}
void failure_outcomes() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    for (bool explicitFailure : {true, false}) {
        uint64_t id;
        CHECK(f.e.start(snap(9, "domain"), id) == Status::Ok);
        Work work;
        CHECK(f.e.take_work(work) == Status::Ok);
        Trace t;
        ResultClaim claim;
        CHECK(f.e.claim_result(id, claim) == Status::Ok);
        CHECK(claim.commit(callback(t)) == Status::Ok);
        if (explicitFailure)
            CHECK(work.fail() == Status::Ok);
        work.finish();
        ResultDelivery result;
        CHECK(f.e.take_result(id, result) == Status::Ok);
        CHECK(result.invoke() ==
              (explicitFailure ? Status::DomainFailure : Status::InternalFailure));
        CHECK(t.count() == 1 && !t.at(0).hasPayload);
        f.e.close_operation(id);
        CHECK(f.e.operation_closed(id));
    }
    finish_engine(f, ex);
    f.balanced(2);
}
void result_claim_rollback() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t id;
    CHECK(f.e.start(snap(7, "rollback"), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    CHECK(work.succeed() == Status::Ok);
    work.finish();
    ResultClaim first;
    CHECK(f.e.claim_result(id, first) == Status::Ok);
    first.rollback();
    ResultClaim second;
    CHECK(f.e.claim_result(id, second) == Status::Ok);
    Trace closeTrace;
    CloseWaiter close;
    CHECK(f.e.watch_close(callback(closeTrace), close) == Status::Ok);
    CHECK(!close.ready() && cp_read(f.context.p).live == 1);
    second.rollback();
    CHECK(cp_read(f.context.p).live == 0);
    ex.finish();
    CHECK(close.ready());
    CHECK(close.invoke() == Status::Ok);
    CHECK(closeTrace.count() == 1);
    f.balanced(1);
}
void result_claim_commit_across_close() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t id;
    CHECK(f.e.start(snap(31, "commit"), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    CHECK(work.succeed() == Status::Ok);
    ResultClaim claim;
    CHECK(f.e.claim_result(id, claim) == Status::Ok);
    f.e.request_close();
    Trace t;
    CHECK(claim.commit(callback(t)) == Status::Ok);
    ResultDelivery result;
    CHECK(f.e.take_result(id, result) == Status::Ok);
    CHECK(result.invoke() == Status::Ok);
    CHECK(t.count() == 1 && t.at(0).value == 31);
    CHECK(!f.e.stats().closed);
    work.finish();
    ex.finish();
    CHECK(f.e.stats().closed);
    f.balanced(1);
}
void atomic_fanout_clone_failure() {
    Fixture f;
    Trace a, b, c;
    uint64_t ids[3];
    CHECK(f.e.observe(Stream::Event, callback(a), ids[0]) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(b), ids[1]) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(c), ids[2]) == Status::Ok);
    cp_fail_after(f.context.p, 1);
    CHECK(f.e.publish(Stream::Event, snap(5, "abc")) == Status::ResourceExhausted);
    for (auto id : ids) {
        Delivery d;
        CHECK(f.e.take_delivery(id, d) == Status::NotReady);
    }
    CHECK(cp_read(f.context.p).allocated == 1 && cp_read(f.context.p).released == 1);
    std::string text = "hello";
    CHECK(f.e.publish(Stream::Event, snap(42, text)) == Status::Ok);
    text = "xxxxx";
    for (auto id : ids) {
        Delivery d;
        CHECK(f.e.take_delivery(id, d) == Status::Ok);
        CHECK(d.invoke() == Status::Ok);
    }
    for (auto *t : {&a, &b, &c})
        CHECK(t->count() == 1 && t->at(0).value == 42 && t->at(0).text == "hello");
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(4);
}
void atomic_fanout_stop_during_source() {
    Fixture f;
    Trace a, b;
    uint64_t ia, ib;
    CHECK(f.e.observe(Stream::Event, callback(a), ia) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(b), ib) == Status::Ok);
    Publication publication;
    CHECK(f.e.reserve_publish(Stream::Event, snap(71, "source"), publication) == Status::Ok);
    CHECK(publication.allocate(snap(71, "source")) == Status::Ok);
    f.e.stop(ib);
    CHECK(b.destroyed == 0 && !f.e.subscription_closed(ib));
    CHECK(publication.commit() == Status::Closing);
    Delivery d;
    CHECK(f.e.take_delivery(ia, d) == Status::NotReady);
    CHECK(a.count() == 0 && b.count() == 0 && b.destroyed == 1);
    f.e.request_close();
    CHECK(a.destroyed == 1 && f.e.stats().closed);
    f.balanced(2);
}
void event_order_across_sources() {
    Fixture f;
    Trace t;
    uint64_t id;
    CHECK(f.e.observe(Stream::Event, callback(t), id) == Status::Ok);
    Publication early, late;
    CHECK(f.e.reserve_publish(Stream::Event, snap(101, "first"), early) == Status::Ok);
    CHECK(f.e.reserve_publish(Stream::Event, snap(202, "second"), late) == Status::Ok);
    CHECK(early.allocate(snap(101, "first")) == Status::Ok);
    CHECK(late.allocate(snap(202, "second")) == Status::Ok);
    CHECK(late.commit() == Status::Ok);
    Delivery d;
    CHECK(f.e.take_delivery(id, d) == Status::NotReady);
    CHECK(early.commit() == Status::Ok);
    CHECK(f.e.take_delivery(id, d) == Status::Ok);
    CHECK(d.invoke() == Status::Ok);
    CHECK(f.e.take_delivery(id, d) == Status::Ok);
    CHECK(d.invoke() == Status::Ok);
    CHECK(t.at(0).value == 101 && t.at(1).value == 202);
    f.e.request_close();
    f.balanced(2);
}
void entered_event_budget_and_stop() {
    Fixture f;
    Trace a, b, closeTrace;
    Gate gate;
    uint64_t ia, ib;
    CHECK(f.e.observe(Stream::Event, callback(a, &gate, false, &f.e), ia) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(b), ib) == Status::Ok);
    for (int i = 1; i <= 4; ++i)
        CHECK(f.e.publish(Stream::Event, snap(i, "e")) == Status::Ok);
    Delivery entered;
    CHECK(f.e.take_delivery(ia, entered) == Status::Ok);
    std::atomic<Status> status{Status::NotReady};
    Joining run{&gate, std::thread([&] { status = entered.invoke(); })};
    gate.wait();
    CHECK(f.e.publish(Stream::Event, snap(9, "e")) == Status::ResourceExhausted);
    CHECK(cp_read(f.context.p).allocated == 8);
    CloseWaiter close;
    CHECK(f.e.watch_subscription_close(ia, callback(closeTrace), close) == Status::Ok);
    CHECK(!close.ready() && a.destroyed == 0);
    f.e.stop(ib);
    CHECK(b.destroyed == 1 && b.count() == 0);
    run.join();
    CHECK(status == Status::Ok && a.count() == 1 && a.destroyed == 1);
    CHECK(close.ready());
    CHECK(close.invoke() == Status::Ok);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(8);
}
void stop_before_enter() {
    Fixture f;
    Trace t;
    uint64_t id;
    CHECK(f.e.observe(Stream::Event, callback(t), id) == Status::Ok);
    CHECK(f.e.publish(Stream::Event, snap(1, "late")) == Status::Ok);
    Delivery ticket;
    CHECK(f.e.take_delivery(id, ticket) == Status::Ok);
    f.e.stop(id);
    CHECK(t.destroyed == 0 && !f.e.subscription_closed(id));
    CHECK(ticket.invoke() == Status::Closing);
    CHECK(t.count() == 0 && t.destroyed == 1);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(1);
}
void state_latest_with_entered() {
    Fixture f;
    Trace t;
    Gate gate;
    uint64_t id;
    CHECK(f.e.observe(Stream::State, callback(t, &gate), id) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(1, "old")) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(2, "entered")) == Status::Ok);
    CHECK(cp_read(f.context.p).released == 1);
    Delivery entered;
    CHECK(f.e.take_delivery(id, entered) == Status::Ok);
    Joining run{&gate, std::thread([&] { CHECK(entered.invoke() == Status::Ok); })};
    gate.wait();
    CHECK(f.e.publish(Stream::State, snap(3, "replace")) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(4, "latest")) == Status::Ok);
    CHECK(cp_read(f.context.p).released == 2);
    Delivery extra;
    CHECK(f.e.take_delivery(id, extra) == Status::NotReady);
    CHECK(f.e.stats().stateTickets == 2);
    run.join();
    CHECK(f.e.take_delivery(id, extra) == Status::Ok);
    CHECK(extra.invoke() == Status::Ok);
    CHECK(t.count() == 2 && t.at(0).value == 2 && t.at(1).value == 4);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(4);
}
void staging_counts_bytes() {
    Fixture f;
    Trace a, b;
    uint64_t ia, ib;
    CHECK(f.e.observe(Stream::Event, callback(a), ia) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(b), ib) == Status::Ok);
    std::string kb(1024, 'a');
    CHECK(f.e.publish(Stream::Event, snap(1, kb)) == Status::Ok);
    Publication staged;
    CHECK(f.e.reserve_publish(Stream::Event, snap(2, kb), staged) == Status::Ok);
    Publication rejected;
    CHECK(f.e.reserve_publish(Stream::Event, snap(3, "x"), rejected) == Status::ResourceExhausted);
    CHECK(cp_read(f.context.p).allocated == 2);
    staged.abandon();
    CHECK(f.e.publish(Stream::Event, snap(4, kb)) == Status::Ok);
    CHECK(f.e.publish(Stream::Event, snap(5, "x")) == Status::ResourceExhausted);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(4);
}
void operation_and_subscription_limits() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t ids[8], extra = 0;
    for (auto &id : ids)
        CHECK(f.e.start(snap(1, ""), id) == Status::Ok);
    CHECK(f.e.start(snap(1, ""), extra) == Status::ResourceExhausted);
    CHECK(cp_read(f.context.p).allocated == 8);
    for (auto id : ids)
        f.e.close_operation(id);
    Trace subs;
    for (auto &id : ids)
        CHECK(f.e.observe(Stream::Event, callback(subs, nullptr, false, &f.e), id) == Status::Ok);
    CHECK(f.e.observe(Stream::Event, callback(subs, nullptr, false, &f.e), extra) ==
          Status::ResourceExhausted);
    CHECK(subs.created == 9 && subs.destroyed == 1);
    finish_engine(f, ex);
    CHECK(subs.destroyed == 9);
    f.balanced(8);
}
void full_close_independent_control() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t ids[8];
    for (auto &id : ids)
        CHECK(f.e.start(snap(6, ""), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    Trace subs, closed;
    for (auto &id : ids)
        CHECK(f.e.observe(Stream::Event, callback(subs), id) == Status::Ok);
    std::string text(512, 'q');
    CHECK(f.e.publish(Stream::Event, snap(99, text)) == Status::Ok);
    CHECK(f.e.publish(Stream::Event, snap(100, "")) == Status::ResourceExhausted);
    CloseWaiter close;
    CHECK(f.e.watch_close(callback(closed), close) == Status::Ok);
    CHECK(!close.ready());
    CHECK(f.e.start(snap(1, ""), ids[0]) == Status::Closing);
    CHECK(f.e.publish(Stream::Event, snap(1, "")) == Status::Closing);
    CHECK(subs.destroyed == 8 && subs.count() == 0);
    CHECK(work.cancelled());
    work.finish();
    CHECK(!close.ready());
    ex.finish();
    CHECK(close.ready());
    CHECK(close.invoke() == Status::Ok);
    CHECK(closed.count() == 1 && closed.destroyed == 1);
    f.balanced(16);
}
void waiter_budget_and_ready_receipt() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    Trace t;
    std::vector<CloseWaiter> held(32);
    for (auto &waiter : held)
        CHECK(f.e.watch_close(callback(t, nullptr, false, &f.e), waiter) == Status::Ok);
    CloseWaiter excess;
    CHECK(f.e.watch_close(callback(t), excess) == Status::ResourceExhausted);
    CHECK(t.count() == 0 && t.destroyed == 1);
    ex.finish();
    for (auto &waiter : held)
        CHECK(waiter.ready());
    CloseWaiter immediate;
    CHECK(f.e.watch_close(callback(t), immediate) == Status::Ok);
    CHECK(immediate.ready() && t.count() == 1 && t.destroyed == 2);
    CHECK(immediate.invoke() == Status::Ok && t.count() == 1);
    for (auto &waiter : held)
        CHECK(waiter.invoke() == Status::Ok);
    CHECK(t.count() == 33 && t.created == 34 && t.destroyed == 34);
    CHECK(f.e.stats().waiters == 0);
    f.balanced(0);
}
void shared_waiter_budget_claim_rollback() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    ResultClaim claims[8];
    uint64_t ids[8];
    for (int i = 0; i < 8; ++i) {
        CHECK(f.e.start(snap(1, ""), ids[i]) == Status::Ok);
        CHECK(f.e.claim_result(ids[i], claims[i]) == Status::Ok);
    }
    Trace t;
    CloseWaiter held[24];
    for (auto &w : held)
        CHECK(f.e.watch_close(callback(t), w) == Status::Ok);
    CloseWaiter excess;
    CHECK(f.e.watch_close(callback(t), excess) == Status::ResourceExhausted);
    for (auto &c : claims)
        c.rollback();
    ex.finish();
    for (auto &w : held) {
        CHECK(w.ready());
        CHECK(w.invoke() == Status::Ok);
    }
    CHECK(t.count() == 24 && t.destroyed == 25 && f.e.stats().waiters == 0);
    f.balanced(8);
}
void callback_failure_survives_slot_reuse() {
    Fixture f;
    Trace throwing, closed, replacement;
    Gate gate;
    uint64_t id, newId;
    CHECK(f.e.observe(Stream::Event, callback(throwing, &gate, true, &f.e), id) == Status::Ok);
    CHECK(f.e.publish(Stream::Event, snap(1, "x")) == Status::Ok);
    Delivery d;
    CHECK(f.e.take_delivery(id, d) == Status::Ok);
    std::atomic<Status> result{Status::NotReady};
    Joining run{&gate, std::thread([&] { result = d.invoke(); })};
    gate.wait();
    CloseWaiter close;
    CHECK(f.e.watch_subscription_close(id, callback(closed), close) == Status::Ok);
    CHECK(!close.ready());
    run.join();
    CHECK(result == Status::CallbackFailure);
    CHECK(f.e.observe(Stream::Event, callback(replacement), newId) == Status::Ok);
    CHECK(newId != id);
    CHECK(close.ready());
    CHECK(close.invoke() == Status::CallbackFailure);
    CHECK(closed.count() == 1 && closed.at(0).status == Status::CallbackFailure);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(1);
}
void empty_publication_has_no_owner() {
    Fixture f;
    std::vector<Publication> empty(64);
    for (auto &p : empty) {
        CHECK(f.e.reserve_publish(Stream::Event, snap(4, "unused"), p) == Status::Ok);
        CHECK(p.allocate(snap(4, "unused")) == Status::Ok);
    }
    f.e.request_close();
    CHECK(f.e.stats().closed && cp_read(f.context.p).attempts == 0);
    for (auto &p : empty)
        CHECK(p.commit() == Status::Ok);
    f.balanced(0);
}
struct PortAudit {
    Engine *engine;
    Gate *gate = nullptr;
    std::atomic<int> clones{0}, drops{0};
    std::atomic<bool> correct{true};
};
void port_observer(void *raw, int action) {
    auto &p = *static_cast<PortAudit *>(raw);
    auto stats = p.engine->stats();
    if (stats.bytes != 128)
        p.correct = false;
    if (action == 1) {
        ++p.clones;
        if (p.gate && !p.gate->arrive())
            p.correct = false;
    } else {
        ++p.drops;
    }
}
void provider_ports_reenter_and_remain_charged() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    PortAudit ports{&f.e};
    cp_hook(f.context.p, port_observer, &ports);
    uint64_t id;
    std::string text(128, 'c');
    CHECK(f.e.start(snap(4, text), id) == Status::Ok);
    f.e.close_operation(id);
    CHECK(ports.correct && ports.clones == 1 && ports.drops == 1);
    cp_hook(f.context.p, nullptr, nullptr);
    finish_engine(f, ex);
    f.balanced(1);
}
void source_lease_survives_stop_before_clone() {
    Fixture f;
    Trace callbacks, closed;
    Gate gate;
    PortAudit ports{&f.e, &gate};
    cp_hook(f.context.p, port_observer, &ports);
    uint64_t id;
    CHECK(f.e.observe(Stream::Event, callback(callbacks), id) == Status::Ok);
    std::string text(128, 's');
    Publication publication;
    CHECK(f.e.reserve_publish(Stream::Event, snap(8, text), publication) == Status::Ok);
    std::atomic<Status> allocated{Status::NotReady}, committed{Status::NotReady};
    Joining task{&gate, std::thread([&] {
                     allocated = publication.allocate(snap(8, text));
                     committed = publication.commit();
                 })};
    gate.wait();
    CloseWaiter waiter;
    CHECK(f.e.watch_subscription_close(id, callback(closed), waiter) == Status::Ok);
    CHECK(!waiter.ready() && callbacks.destroyed == 0 && cp_read(f.context.p).allocated == 0);
    task.join();
    CHECK(allocated == Status::Ok && committed == Status::Closing);
    CHECK(waiter.ready() && waiter.invoke() == Status::Ok);
    CHECK(callbacks.count() == 0 && callbacks.destroyed == 1 && ports.correct &&
          ports.clones == 1 && ports.drops == 1);
    cp_hook(f.context.p, nullptr, nullptr);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(1);
}
void state_fanout_failure_preserves_previous() {
    Fixture f;
    Trace a, b;
    uint64_t ia, ib;
    CHECK(f.e.observe(Stream::State, callback(a), ia) == Status::Ok);
    CHECK(f.e.observe(Stream::State, callback(b), ib) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(17, "kept")) == Status::Ok);
    cp_fail_after(f.context.p, 1);
    CHECK(f.e.publish(Stream::State, snap(19, "failed")) == Status::ResourceExhausted);
    CHECK(cp_read(f.context.p).allocated == 3 && cp_read(f.context.p).released == 1);
    for (auto id : {ia, ib}) {
        Delivery d;
        CHECK(f.e.take_delivery(id, d) == Status::Ok);
        CHECK(d.invoke() == Status::Ok);
    }
    CHECK(a.count() == 1 && b.count() == 1 && a.at(0).value == 17 && b.at(0).value == 17 &&
          a.at(0).text == "kept" && b.at(0).text == "kept");
    f.e.request_close();
    f.balanced(3);
}
void success_winner_survives_late_cancel() {
    Fixture f;
    Executor ex;
    CHECK(f.e.acquire_executor(ex) == Status::Ok);
    uint64_t id;
    CHECK(f.e.start(snap(53, "winner"), id) == Status::Ok);
    Work work;
    CHECK(f.e.take_work(work) == Status::Ok);
    CHECK(work.succeed() == Status::Ok);
    CHECK(f.e.cancel(id) == Status::Ok);
    CHECK(f.e.winner(id) == Winner::Success && !work.cancelled());
    Trace trace;
    ResultClaim claim;
    CHECK(f.e.claim_result(id, claim) == Status::Ok);
    CHECK(claim.commit(callback(trace)) == Status::Ok);
    ResultDelivery result;
    CHECK(f.e.take_result(id, result) == Status::Ok);
    CHECK(result.invoke() == Status::Ok && trace.count() == 1 && trace.at(0).value == 53);
    work.finish();
    finish_engine(f, ex);
    f.balanced(1);
}
void state_preentry_observes_latest_only() {
    Fixture f;
    Trace t;
    uint64_t id;
    CHECK(f.e.observe(Stream::State, callback(t), id) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(1, "superseded")) == Status::Ok);
    Delivery ticket;
    CHECK(f.e.take_delivery(id, ticket) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(2, "latest")) == Status::Ok);
    const bool onePending = f.e.stats().stateTickets == 1;
    const bool oldReleased = cp_read(f.context.p).released == 1;
    CHECK(ticket.invoke() == Status::Ok);
    CHECK(t.count() == 1 && t.at(0).value == 2 && t.at(0).text == "latest");
    CHECK(onePending && oldReleased);
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(2);
}
void state_preentry_stop_releases_only_after_lease() {
    Fixture f;
    Trace t;
    uint64_t id;
    CHECK(f.e.observe(Stream::State, callback(t), id) == Status::Ok);
    CHECK(f.e.publish(Stream::State, snap(1, "pending")) == Status::Ok);
    Delivery ticket;
    CHECK(f.e.take_delivery(id, ticket) == Status::Ok);
    f.e.stop(id);
    CHECK(t.destroyed == 0 && !f.e.subscription_closed(id));
    CHECK(ticket.invoke() == Status::Closing);
    CHECK(t.count() == 0 && t.destroyed == 1 && f.e.subscription_closed(id));
    f.e.request_close();
    CHECK(f.e.stats().closed);
    f.balanced(1);
}
int main(int argc, char **argv) {
    const std::vector<std::pair<std::string, std::function<void()>>> tests = {
        {"vector_and_c_algorithm", vector_and_c_algorithm},
        {"terminal_before_work_exit", terminal_before_work_exit},
        {"cancel_and_late_result", cancel_and_late_result},
        {"failure_outcomes", failure_outcomes},
        {"result_claim_rollback", result_claim_rollback},
        {"result_claim_commit_across_close", result_claim_commit_across_close},
        {"atomic_fanout_clone_failure", atomic_fanout_clone_failure},
        {"atomic_fanout_stop_during_source", atomic_fanout_stop_during_source},
        {"event_order_across_sources", event_order_across_sources},
        {"entered_event_budget_and_stop", entered_event_budget_and_stop},
        {"stop_before_enter", stop_before_enter},
        {"state_latest_with_entered", state_latest_with_entered},
        {"staging_counts_bytes", staging_counts_bytes},
        {"operation_and_subscription_limits", operation_and_subscription_limits},
        {"full_close_independent_control", full_close_independent_control},
        {"waiter_budget_and_ready_receipt", waiter_budget_and_ready_receipt},
        {"shared_waiter_budget_claim_rollback", shared_waiter_budget_claim_rollback},
        {"callback_failure_survives_slot_reuse", callback_failure_survives_slot_reuse},
        {"empty_publication_has_no_owner", empty_publication_has_no_owner},
        {"provider_ports_reenter_and_remain_charged", provider_ports_reenter_and_remain_charged},
        {"source_lease_survives_stop_before_clone", source_lease_survives_stop_before_clone},
        {"state_fanout_failure_preserves_previous", state_fanout_failure_preserves_previous},
        {"success_winner_survives_late_cancel", success_winner_survives_late_cancel},
        {"state_preentry_observes_latest_only", state_preentry_observes_latest_only},
        {"state_preentry_stop_releases_only_after_lease",
         state_preentry_stop_releases_only_after_lease}};
    if (argc == 2 && std::string(argv[1]) == "--list") {
        for (auto &test : tests)
            std::cout << test.first << "\n";
        return 0;
    }
    for (auto &test : tests)
        if (argc == 2 && test.first == argv[1]) {
            try {
                test.second();
                std::cout << "PASS " << test.first << "\n";
                return 0;
            } catch (const std::exception &error) {
                std::cerr << "FAIL " << error.what() << "\n";
                return 1;
            }
        }
    std::cerr << "one explicit case name required\n";
    return 2;
}
