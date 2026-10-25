#include "support.hpp"
#include <cassert>
#include <chrono>
#include <iostream>
using namespace gateway;
using namespace madb::c07;
namespace {
#define require(value)                                                                             \
    do {                                                                                           \
        if (!(value)) {                                                                            \
            std::cerr << "failed line " << __LINE__ << ": " #value << "\n";                        \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
template <class P> void eventually(P predicate) {
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (!predicate()) {
        require(std::chrono::steady_clock::now() < deadline);
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}
int64_t trace(Audit &a, T i) {
    std::lock_guard<std::mutex> l(a.testMutex);
    return a.test[i];
}
Snapshot input() {
    static const unsigned char text[] = "real-model";
    return {7, text, sizeof(text) - 1};
}
mp_stats stats(Runtime &r) {
    mp_stats s{};
    require(r.provider.stats(r.provider.context, &s) == MP_OK);
    return s;
}
struct Fixture {
    Audit audit;
    Runtime runtime{0, 1, nullptr, audit};
    Fixture() {
        require(GatewayProviderCreate(&runtime.provider) == MP_OK);
        runtime.engine = std::make_unique<Engine>(testProvider(runtime));
    }
    ~Fixture() {
        if (runtime.worker) {
            require(runtime.worker->joined());
            runtime.worker.reset();
        }
        runtime.engine.reset();
        require(stats(runtime).live == 0);
        require(runtime.provider.destroy_context(runtime.provider.context) == MP_OK);
        runtime.provider = {};
    }
};
void fault(Fixture &f, int nth, int mode) {
    f.runtime.test.cloneNth = nth;
    f.runtime.test.cloneMode = mode;
}
Status run(void *p, void *model) noexcept {
    auto &r = *static_cast<Runtime *>(p);
    mp_status status;
    {
        std::lock_guard<std::mutex> l(r.test.mutex);
        status = r.provider.increment(r.provider.context, model);
    }
    testGate(r, 1);
    return status == MP_OK ? Status::Ok : Status::InternalFailure;
}
void ready(void *) noexcept {}
void start(Fixture &f, int phase) {
    auto &r = f.runtime;
    r.test.gatePhase = phase;
    r.test.gateArmed = true;
    require(NativeWorker::start(*r.engine, {&r, run, ready}, r.worker) == Status::Ok);
}
void arrived(Fixture &f) {
    eventually([&] {
        std::lock_guard<std::mutex> l(f.runtime.test.mutex);
        return f.runtime.test.gateArrived;
    });
}
void release(Fixture &f) {
    auto &r = f.runtime;
    {
        std::lock_guard<std::mutex> l(r.test.mutex);
        r.test.gateReleased = true;
    }
    r.test.changed.notify_all();
}
void finish(Fixture &f) {
    auto &r = f.runtime;
    r.worker->request_stop();
    std::thread cleanup([&] { require(r.worker->join_and_ack() == Status::Ok); });
    cleanup.join();
    require(r.worker->joined());
    require(trace(f.audit, T::WorkerTlsExited) == 1);
    require(trace(f.audit, T::LoopExitedSequence) < trace(f.audit, T::TlsExitedSequence));
    require(trace(f.audit, T::TlsExitedSequence) < trace(f.audit, T::JoinReturnedSequence));
    require(trace(f.audit, T::JoinReturnedSequence) < trace(f.audit, T::ExecutorReleasedSequence));
}
void own_error_core() {
    Fixture f;
    Executor executor;
    require(f.runtime.engine->acquire_executor(executor) == Status::Ok);
    fault(f, 1, 4);
    uint64_t id = 0;
    require(f.runtime.engine->start(input(), id) == Status::InternalFailure);
    auto s = stats(f.runtime);
    require(s.allocations == 1 && s.releases == 1 && s.live == 0);
    require(f.runtime.engine->stats().operations == 0 && f.runtime.engine->stats().bytes == 0);
}
void own_error_roundtrip() {
    Fixture f;
    fault(f, 1, 4);
    void *owned = nullptr;
    auto s = input();
    require(testRoundtripClone(f.runtime, {7, s.text, s.size}, &owned) == MP_INTERNAL_FAILURE);
    require(owned != nullptr);
    require(stats(f.runtime).live == 1);
    require(f.runtime.provider.release(f.runtime.provider.context, owned) == MP_OK);
    require(stats(f.runtime).releases == 1);
}
void provider_errors() {
    for (int mode : {1, 2}) {
        Fixture f;
        Executor executor;
        require(f.runtime.engine->acquire_executor(executor) == Status::Ok);
        fault(f, 1, mode);
        uint64_t id = 0;
        require(f.runtime.engine->start(input(), id) ==
                (mode == 1 ? Status::ResourceExhausted : Status::InternalFailure));
        auto s = stats(f.runtime);
        require(s.allocations == 0 && s.releases == 0 && s.live == 0);
        require(trace(f.audit, T::CloneFaultHits) == 1);
    }
}
void phase1_close_release() {
    Fixture f;
    start(f, 1);
    auto &r = f.runtime;
    uint64_t id = 0;
    require(r.worker->submit(input(), id) == Status::Ok);
    testSubmitted(r, id);
    arrived(f);
    require(r.engine->winner(id) == Winner::Pending);
    require(r.engine->stats().work == 1);
    r.worker->request_stop();
    std::atomic<bool> joined{false};
    std::thread cleanup([&] {
        require(r.worker->join_and_ack() == Status::Ok);
        joined = true;
    });
    eventually([&] { return trace(f.audit, T::JoinStarted) == 1; });
    require(!joined);
    require(r.engine->winner(id) == Winner::Cancelled);
    release(f);
    cleanup.join();
    require(joined);
    require(r.test.gateResumed && !r.test.gateArmed);
    require(trace(f.audit, T::WorkerTlsExited) == 1);
}
void phase2_first_terminal() {
    Fixture f;
    start(f, 2);
    auto &r = f.runtime;
    uint64_t id = 0;
    require(r.worker->submit(input(), id) == Status::Ok);
    testSubmitted(r, id);
    arrived(f);
    require(r.engine->winner(id) == Winner::Success && r.engine->stats().work == 1);
    require(r.engine->cancel(id) == Status::Ok);
    require(r.engine->winner(id) == Winner::Success);
    release(f);
    eventually([&] { return r.engine->stats().work == 0; });
    require(!r.test.gateArmed);
    uint64_t other = 0;
    require(r.worker->submit(input(), other) == Status::Ok);
    eventually([&] { return r.engine->stats().work == 0; });
    require(r.test.gateOperation == id);
    finish(f);
}
void publication_closing() {
    Fixture f;
    auto &r = f.runtime;
    int callbacks = 0;
    uint64_t id = 0;
    auto cb = Callback(
        &callbacks, [](void *p, Status, const void *) { ++*static_cast<int *>(p); },
        [](void *) noexcept {});
    require(r.engine->observe(Stream::Event, std::move(cb), id) == Status::Ok);
    require(r.engine->reserve_publish(Stream::Event, input(), r.test.publication) == Status::Ok);
    require(r.test.publication.allocate(input()) == Status::Ok);
    r.test.sourceHeld = true;
    require(r.engine->stats().sources == 1);
    r.engine->stop(id);
    require(!r.engine->subscription_closed(id));
    require(r.test.publication.commit() == Status::Closing);
    r.test.sourceHeld = false;
    require(r.engine->stats().sources == 0 && stats(r).live == 0 && callbacks == 0);
}
void unwind_actual_owners() {
    Fixture f;
    auto &r = f.runtime;
    uint64_t sub = 0;
    auto cb = Callback(
        nullptr, [](void *, Status, const void *) {}, [](void *) noexcept {});
    require(r.engine->observe(Stream::State, std::move(cb), sub) == Status::Ok);
    require(r.engine->reserve_publish(Stream::State, input(), r.test.publication) == Status::Ok);
    require(r.test.publication.allocate(input()) == Status::Ok);
    r.test.sourceHeld = true;
    start(f, 1);
    uint64_t id = 0;
    require(r.worker->submit(input(), id) == Status::Ok);
    testSubmitted(r, id);
    arrived(f);
    r.worker->request_stop();
    r.envClosing = true;
    testUnwind(r);
    finish(f);
    require(!r.test.sourceHeld && r.engine->stats().sources == 0 && stats(r).live == 0);
    require(trace(f.audit, T::SourceAbandoned) == 1);
}
} // namespace
int main() {
    std::cerr << "own_error_core\n";
    own_error_core();
    std::cerr << "own_error_roundtrip\n";
    own_error_roundtrip();
    std::cerr << "provider_errors\n";
    provider_errors();
    std::cerr << "phase1_close_release\n";
    phase1_close_release();
    std::cerr << "phase2_first_terminal\n";
    phase2_first_terminal();
    std::cerr << "publication_closing\n";
    publication_closing();
    std::cerr << "unwind_actual_owners\n";
    unwind_actual_owners();
    std::cout << "8 native companion checks passed; host-only Core/provider/work hooks, no "
                 "NAPI/target claim\n";
}
