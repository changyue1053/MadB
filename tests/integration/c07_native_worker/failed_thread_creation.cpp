#include "worker.hpp"
#include <dlfcn.h>
#include <iostream>
using namespace madb::c07;
int main() {
    auto failures =
        reinterpret_cast<int (*)()>(dlsym(RTLD_DEFAULT, "probe_pthread_create_failures"));
    if (!failures)
        return 2;
    Engine engine({nullptr,
                   [](void *, Snapshot) noexcept {
                       return CloneResult{CloneStatus::InternalFailure, nullptr};
                   },
                   [](void *, void *) noexcept {}});
    std::unique_ptr<NativeWorker> worker;
    WorkerPorts ports{nullptr, [](void *, void *) noexcept { return Status::Ok; },
                      [](void *) noexcept {}};
    auto status = NativeWorker::start(engine, ports, worker);
    auto s = engine.stats();
    bool rolled =
        status != Status::Ok && !worker && !s.executors && !s.work && !s.operations && !s.bytes;
    auto retry = NativeWorker::start(engine, ports, worker);
    engine.request_close();
    bool pass =
        rolled && failures() == 1 && retry == Status::AlreadyConsumed && engine.stats().closed;
    std::cout << "actual_pthread_EAGAIN=" << failures() << " status=" << static_cast<int>(status)
              << " resources_rolled_back=" << rolled
              << " retry_one_shot=" << (retry == Status::AlreadyConsumed)
              << " final_closed=" << engine.stats().closed << "\n";
    return pass ? 0 : 1;
}
