#include "provider_abi.h"
#include "worker.hpp"
#include <atomic>
#include <chrono>
#include <climits>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

using namespace madb::c07;
using namespace std::chrono_literals;
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x))                                                                                  \
            throw std::runtime_error(#x);                                                          \
    } while (false)

template <class F> void until(F predicate) {
    auto deadline = std::chrono::steady_clock::now() + 5s;
    while (!predicate()) {
        if (std::chrono::steady_clock::now() >= deadline)
            throw std::runtime_error("test barrier deadline");
        std::this_thread::sleep_for(1ms);
    }
}

struct Barrier {
    std::mutex mutex;
    std::condition_variable changed;
    bool enabled = false, entered = false, allow = false;
    void wait() {
        std::unique_lock<std::mutex> lock(mutex);
        entered = true;
        changed.notify_all();
        if (enabled)
            changed.wait(lock, [&] { return allow; });
    }
    void release() {
        std::lock_guard<std::mutex> lock(mutex);
        allow = true;
        changed.notify_all();
    }
    void await_entry() {
        std::unique_lock<std::mutex> lock(mutex);
        CHECK(changed.wait_for(lock, 5s, [&] { return entered; }));
    }
};
struct TlsOwner {
    Barrier *exit = nullptr;
    ~TlsOwner() {
        if (exit)
            exit->wait();
    }
};
struct Session {
    mp_provider provider{};
    Barrier work, tlsExit;
    std::atomic<int> calls{0}, hints{0};
    std::atomic<bool> selfJoin{false};
    std::atomic<int> selfJoinStatus{-1};
    NativeWorker *worker = nullptr;
    Status forced = Status::Ok;
};
CloneResult clone_model(void *pointer, Snapshot input) noexcept {
    auto &session = *static_cast<Session *>(pointer);
    void *owned = nullptr;
    if (input.value < INT32_MIN || input.value > INT32_MAX)
        return {CloneStatus::InvalidArgument, nullptr};
    auto status =
        session.provider.clone(session.provider.context,
                               {static_cast<int32_t>(input.value), input.text, input.size}, &owned);
    switch (status) {
    case MP_OK:
        return {CloneStatus::Ok, owned};
    case MP_INVALID_ARGUMENT:
        return {CloneStatus::InvalidArgument, owned};
    case MP_ALLOCATION_FAILURE:
        return {CloneStatus::AllocationFailure, owned};
    default:
        return {CloneStatus::InternalFailure, owned};
    }
}
void release_model(void *pointer, void *model) noexcept {
    auto &session = *static_cast<Session *>(pointer);
    if (session.provider.release(session.provider.context, model) != MP_OK)
        std::terminate();
}
Status run_model(void *pointer, void *model) noexcept {
    auto &session = *static_cast<Session *>(pointer);
    thread_local TlsOwner tls;
    tls.exit = &session.tlsExit;
    ++session.calls;
    session.work.wait();
    if (session.selfJoin)
        session.selfJoinStatus = static_cast<int>(session.worker->join_and_ack());
    if (session.forced != Status::Ok)
        return session.forced;
    auto status = session.provider.increment(session.provider.context, model);
    return status == MP_OK                ? Status::Ok
           : status == MP_DOMAIN_OVERFLOW ? Status::DomainFailure
                                          : Status::InternalFailure;
}
void ready(void *pointer) noexcept { ++static_cast<Session *>(pointer)->hints; }

struct Fixture {
    Session session;
    Engine engine{{&session, clone_model, release_model}};
    std::unique_ptr<NativeWorker> worker;
    Fixture() { CHECK(mp_c_create(&session.provider) == MP_OK); }
    void start() {
        CHECK(NativeWorker::start(engine, {&session, run_model, ready}, worker) == Status::Ok);
        session.worker = worker.get();
    }
    void stop_join() {
        worker->request_stop();
        Status result = Status::InternalFailure;
        std::thread cleanup([&] { result = worker->join_and_ack(); });
        cleanup.join();
        CHECK(result == Status::Ok && worker->joined());
        CHECK(engine.stats().executors == 0);
    }
    ~Fixture() {
        session.work.release();
        session.tlsExit.release();
        if (worker && !worker->joined()) {
            worker->request_stop();
            std::thread cleanup([&] { worker->join_and_ack(); });
            cleanup.join();
        }
        engine.request_close();
        if (session.provider.context &&
            session.provider.destroy_context(session.provider.context) != MP_OK)
            std::terminate();
    }
    uint64_t submit(int value) {
        uint64_t id = 0;
        static const unsigned char text[] = {'h', 0, 'i'};
        CHECK(worker->submit({value, text, sizeof(text)}, id) == Status::Ok);
        return id;
    }
};
struct Received {
    Session *session;
    Status status = Status::NotReady;
    int value = 0;
    std::string text{};
    int calls = 0;
};
void result_callback(void *pointer, Status status, const void *model) {
    auto &received = *static_cast<Received *>(pointer);
    ++received.calls;
    received.status = status;
    if (status == Status::Ok) {
        mp_snapshot view{};
        CHECK(received.session->provider.read_view(received.session->provider.context, model,
                                                   &view) == MP_OK);
        received.value = view.value;
        received.text.assign(reinterpret_cast<const char *>(view.text), view.size);
    }
}
void consume(Fixture &f, uint64_t id, Received &received) {
    ResultClaim claim;
    CHECK(f.engine.claim_result(id, claim) == Status::Ok);
    CHECK(claim.commit({&received, result_callback, nullptr}) == Status::Ok);
    until([&] { return f.engine.winner(id) != Winner::Pending; });
    ResultDelivery delivery;
    CHECK(f.engine.take_result(id, delivery) == Status::Ok);
    const auto delivered = delivery.invoke();
    CHECK(delivered == received.status && delivered != Status::NotReady);
    CHECK(received.calls == 1);
    f.engine.close_operation(id);
    until([&] { return f.engine.operation_closed(id); });
}

void success_and_idle_wake() {
    Fixture f;
    f.start();
    for (int i = 0; i != 100; ++i) {
        auto id = f.submit(i);
        Received result{&f.session};
        consume(f, id, result);
        CHECK(result.status == Status::Ok && result.value == i + 1);
        CHECK(result.text == std::string("h\0i", 3));
    }
    f.stop_join();
    CHECK(f.session.calls == 100 && f.session.hints == 100);
    mp_stats stats{};
    CHECK(f.session.provider.stats(f.session.provider.context, &stats) == MP_OK);
    CHECK(stats.allocations == 100 && stats.releases == 100 && stats.live == 0);
}
void domain_failure() {
    Fixture f;
    f.start();
    auto id = f.submit(INT32_MAX);
    Received r{&f.session};
    consume(f, id, r);
    CHECK(r.status == Status::DomainFailure);
    f.stop_join();
}
void internal_failure() {
    Fixture f;
    f.session.forced = Status::InternalFailure;
    f.start();
    auto id = f.submit(1);
    Received r{&f.session};
    consume(f, id, r);
    CHECK(r.status == Status::InternalFailure);
    f.stop_join();
}
void running_and_queued_cancel() {
    Fixture f;
    f.session.work.enabled = true;
    f.start();
    auto running = f.submit(1);
    f.session.work.await_entry();
    auto queued = f.submit(2);
    CHECK(f.engine.cancel(queued) == Status::Ok);
    CHECK(f.engine.cancel(running) == Status::Ok);
    CHECK(f.engine.stats().work == 1);
    f.session.work.release();
    Received one{&f.session}, two{&f.session};
    consume(f, running, one);
    consume(f, queued, two);
    CHECK(one.status == Status::Cancelled && two.status == Status::Cancelled);
    f.stop_join();
    CHECK(f.session.calls == 1);
}
void tls_exit_is_not_join() {
    Fixture f;
    f.session.tlsExit.enabled = true;
    f.start();
    auto id = f.submit(1);
    Received r{&f.session};
    consume(f, id, r);
    f.worker->request_stop();
    f.session.tlsExit.await_entry();
    std::atomic<bool> returned{false};
    Status status = Status::InternalFailure;
    std::thread cleanup([&] {
        status = f.worker->join_and_ack();
        returned = true;
    });
    CHECK(f.engine.stats().work == 0 && f.engine.stats().executors == 1);
    CHECK(!f.engine.stats().closed && !f.worker->joined() && !returned);
    f.session.tlsExit.release();
    cleanup.join();
    CHECK(returned && status == Status::Ok && f.worker->joined());
    CHECK(f.engine.stats().closed && f.engine.stats().executors == 0);
}
void full_close_and_join() {
    Fixture f;
    f.session.work.enabled = true;
    f.start();
    f.submit(1);
    f.session.work.await_entry();
    for (int i = 0; i < 7; ++i)
        f.submit(i);
    uint64_t rejected = 0;
    CHECK(f.worker->submit({1, nullptr, 0}, rejected) == Status::ResourceExhausted);
    f.worker->request_stop();
    CHECK(f.worker->submit({1, nullptr, 0}, rejected) == Status::Closing);
    CHECK(f.engine.stats().work == 1 && f.engine.stats().executors == 1);
    f.session.work.release();
    f.stop_join();
    CHECK(f.engine.stats().closed && f.engine.stats().bytes == 0);
    CHECK(f.session.calls == 1);
}
void owner_join_rejected() {
    Fixture f;
    f.start();
    f.worker->request_stop();
    CHECK(f.worker->join_and_ack() == Status::InvalidArgument);
    CHECK(f.engine.stats().executors == 1);
    f.stop_join();
}
void self_join_before_external_lock() {
    Fixture f;
    f.session.work.enabled = true;
    f.session.selfJoin = true;
    f.start();
    f.submit(1);
    f.session.work.await_entry();
    f.worker->request_stop();
    std::atomic<bool> started{false};
    Status status = Status::InternalFailure;
    std::thread cleanup([&] {
        started = true;
        status = f.worker->join_and_ack();
    });
    until([&] { return started.load(); });
    f.session.work.release();
    cleanup.join();
    CHECK(status == Status::Ok);
    CHECK(f.session.selfJoinStatus == static_cast<int>(Status::InvalidArgument));
}
void duplicate_executor_and_join() {
    Fixture f;
    f.start();
    std::unique_ptr<NativeWorker> second;
    CHECK(NativeWorker::start(f.engine, {&f.session, run_model, ready}, second) ==
          Status::AlreadyConsumed);
    CHECK(!second);
    f.worker->request_stop();
    Status a = Status::InternalFailure, b = a;
    std::thread one([&] { a = f.worker->join_and_ack(); });
    std::thread two([&] { b = f.worker->join_and_ack(); });
    one.join();
    two.join();
    CHECK(a == Status::Ok && b == Status::Ok && f.engine.stats().executors == 0);
}
void join_before_stop_rejected() {
    Fixture f;
    f.start();
    Status s = Status::Ok;
    std::thread cleanup([&] { s = f.worker->join_and_ack(); });
    cleanup.join();
    CHECK(s == Status::InvalidArgument && f.engine.stats().executors == 1);
    f.stop_join();
}

int main() {
    const std::pair<const char *, void (*)()> cases[] = {
        {"success_and_idle_wake", success_and_idle_wake},
        {"domain_failure", domain_failure},
        {"internal_failure", internal_failure},
        {"running_and_queued_cancel", running_and_queued_cancel},
        {"tls_exit_is_not_join", tls_exit_is_not_join},
        {"full_close_and_join", full_close_and_join},
        {"owner_join_rejected", owner_join_rejected},
        {"self_join_before_external_lock", self_join_before_external_lock},
        {"duplicate_executor_and_join", duplicate_executor_and_join},
        {"join_before_stop_rejected", join_before_stop_rejected}};
    for (const auto &test : cases) {
        try {
            test.second();
            std::cout << "PASS " << test.first << '\n';
        } catch (const std::exception &e) {
            std::cerr << "FAIL " << test.first << ": " << e.what() << '\n';
            return 1;
        }
    }
}
