#include "worker.hpp"
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <iostream>
#include <new>
using namespace madb::c07;
using namespace std::chrono_literals;
struct Context {
    NativeWorker *worker = nullptr;
    std::mutex mutex;
    std::condition_variable cv;
    bool entered = false, allow = false;
    std::atomic<int> self{-1};
};
struct TlsOwner {
    Context *context = nullptr;
    ~TlsOwner() {
        if (!context)
            return;
        {
            std::unique_lock<std::mutex> l(context->mutex);
            context->entered = true;
            context->cv.notify_all();
            context->cv.wait(l, [&] { return context->allow; });
        }
        context->self = static_cast<int>(context->worker->join_and_ack());
    }
};
int main() {
    Context ctx;
    Engine engine({nullptr,
                   [](void *, Snapshot) noexcept {
                       auto *p = new (std::nothrow) int(1);
                       return CloneResult{p ? CloneStatus::Ok : CloneStatus::AllocationFailure, p};
                   },
                   [](void *, void *p) noexcept { delete static_cast<int *>(p); }});
    std::unique_ptr<NativeWorker> worker;
    if (NativeWorker::start(engine,
                            {&ctx,
                             [](void *p, void *) noexcept {
                                 thread_local TlsOwner owned;
                                 owned.context = static_cast<Context *>(p);
                                 return Status::Ok;
                             },
                             [](void *) noexcept {}},
                            worker) != Status::Ok)
        return 2;
    ctx.worker = worker.get();
    uint64_t id = 0;
    if (worker->submit({1, nullptr, 0}, id) != Status::Ok)
        return 3;
    auto deadline = std::chrono::steady_clock::now() + 5s;
    while (engine.winner(id) == Winner::Pending) {
        if (std::chrono::steady_clock::now() > deadline)
            return 4;
        std::this_thread::yield();
    }
    worker->request_stop();
    {
        std::unique_lock<std::mutex> l(ctx.mutex);
        if (!ctx.cv.wait_for(l, 5s, [&] { return ctx.entered; }))
            return 5;
    }
    std::atomic<bool> externalStarted{false}, externalReturned{false};
    Status joined = Status::InternalFailure;
    std::thread cleanup([&] {
        externalStarted = true;
        joined = worker->join_and_ack();
        externalReturned = true;
    });
    while (!externalStarted)
        std::this_thread::yield();
    std::this_thread::sleep_for(20ms);
    bool held = engine.stats().executors == 1 && !engine.stats().closed && !externalReturned;
    {
        std::lock_guard<std::mutex> l(ctx.mutex);
        ctx.allow = true;
        ctx.cv.notify_all();
    }
    cleanup.join();
    bool pass = held && ctx.self == static_cast<int>(Status::InvalidArgument) &&
                joined == Status::Ok && worker->joined() && engine.stats().closed &&
                engine.stats().executors == 0;
    std::cout << "tls_held_executor=" << held
              << " tls_self_rejected=" << (ctx.self == static_cast<int>(Status::InvalidArgument))
              << " external_join_ok=" << (joined == Status::Ok)
              << " final_closed=" << engine.stats().closed << "\n";
    return pass ? 0 : 1;
}
