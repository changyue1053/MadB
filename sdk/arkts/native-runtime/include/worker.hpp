#pragma once
#include "protocol.hpp"
#include <condition_variable>
#include <memory>
#include <mutex>
#include <thread>

namespace madb::c07 {
// Private, explicit one-thread driver. Context/Engine/provider/DSO must outlive
// join_and_ack and every callback. The constructing env thread remains alive
// through cleanup and driver destruction (an async cleanup handle retains that
// platform scope). Neither port may enter env-only NAPI directly.
struct WorkerPorts {
    void *context = nullptr;
    Status (*run)(void *, void *) noexcept = nullptr;
    void (*ready)(void *) noexcept = nullptr;
};

class NativeWorker final {
  public:
    static Status start(Engine &, WorkerPorts, std::unique_ptr<NativeWorker> &) noexcept;
    ~NativeWorker();
    NativeWorker(const NativeWorker &) = delete;
    NativeWorker &operator=(const NativeWorker &) = delete;
    Status submit(Snapshot, uint64_t &);
    void request_stop();
    // Blocking operation reserved for the externally owned cleanup executor.
    // Rejects the constructing owner thread and the business thread. Requesting
    // stop or observing a returned run function does not release Executor.
    Status join_and_ack() noexcept;
    bool joined() const;

  private:
    NativeWorker(Engine &, WorkerPorts);
    void loop() noexcept;
    Engine &engine_;
    WorkerPorts ports_;
    const std::thread::id owner_;
    Executor executor_;
    std::thread thread_;
    mutable std::mutex mutex_;
    std::mutex joinMutex_;
    std::condition_variable changed_;
    bool pendingWake_ = false;
    bool stopping_ = false;
    bool joined_ = false;
};
} // namespace madb::c07
