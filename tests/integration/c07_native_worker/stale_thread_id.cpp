#include "worker.hpp"
#include <condition_variable>
#include <iostream>
#include <new>
#include <vector>
using namespace madb::c07;
struct State {
    std::mutex mutex;
    std::condition_variable cv;
    bool ran = false;
    std::thread::id business;
};
int main() {
    State state;
    Engine engine({nullptr,
                   [](void *, Snapshot) noexcept {
                       auto *p = new (std::nothrow) int(1);
                       return CloneResult{p ? CloneStatus::Ok : CloneStatus::AllocationFailure, p};
                   },
                   [](void *, void *p) noexcept { delete static_cast<int *>(p); }});
    std::unique_ptr<NativeWorker> worker;
    auto start = NativeWorker::start(engine,
                                     {&state,
                                      [](void *p, void *) noexcept {
                                          auto &s = *static_cast<State *>(p);
                                          std::lock_guard<std::mutex> l(s.mutex);
                                          s.business = std::this_thread::get_id();
                                          s.ran = true;
                                          s.cv.notify_all();
                                          return Status::Ok;
                                      },
                                      [](void *) noexcept {}},
                                     worker);
    if (start != Status::Ok)
        return 2;
    uint64_t id = 0;
    if (worker->submit({1, nullptr, 0}, id) != Status::Ok)
        return 3;
    {
        std::unique_lock<std::mutex> l(state.mutex);
        state.cv.wait(l, [&] { return state.ran; });
    }
    worker->request_stop();
    Status first = Status::InternalFailure;
    std::thread cleanup([&] { first = worker->join_and_ack(); });
    cleanup.join();
    if (first != Status::Ok || !worker->joined())
        return 4;
    std::mutex mutex;
    std::condition_variable cv;
    bool go = false;
    constexpr size_t n = 16;
    std::vector<std::thread> threads;
    std::vector<Status> statuses(n, Status::InternalFailure);
    std::vector<std::thread::id> ids(n);
    for (size_t i = 0; i < n; ++i)
        threads.emplace_back([&, i] {
            ids[i] = std::this_thread::get_id();
            {
                std::unique_lock<std::mutex> l(mutex);
                cv.wait(l, [&] { return go; });
            }
            statuses[i] = worker->join_and_ack();
        });
    {
        std::lock_guard<std::mutex> l(mutex);
        go = true;
        cv.notify_all();
    }
    for (auto &t : threads)
        t.join();
    bool failed = false;
    for (size_t i = 0; i < n; ++i) {
        if (statuses[i] != Status::Ok) {
            failed = true;
            std::cout << "already_joined=" << worker->joined()
                      << " same_old_business_id=" << (ids[i] == state.business)
                      << " result=" << static_cast<int>(statuses[i]) << "\n";
        }
    }
    std::cout << "first_join=Ok final_closed=" << engine.stats().closed
              << " repeated_join_all_ok=" << !failed << "\n";
    return failed ? 1 : 0;
}
