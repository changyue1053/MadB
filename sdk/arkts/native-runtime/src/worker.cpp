#include "worker.hpp"
#include <new>

namespace madb::c07 {
namespace {
// Thread identifiers may be reused after a join. This trivial TLS marker denotes
// the actual executing business thread, including its TLS-destructor phase.
thread_local const NativeWorker *currentWorker = nullptr;
}
NativeWorker::NativeWorker(Engine &engine, WorkerPorts ports)
    : engine_(engine), ports_(ports), owner_(std::this_thread::get_id()) {}

Status NativeWorker::start(Engine &engine, WorkerPorts ports,
                           std::unique_ptr<NativeWorker> &out) noexcept {
    if (out || !ports.run || !ports.ready)
        return Status::InvalidArgument;
    try {
        auto candidate = std::unique_ptr<NativeWorker>(new NativeWorker(engine, ports));
        auto status = engine.acquire_executor(candidate->executor_);
        if (status != Status::Ok)
            return status;
        // On construction failure no thread exists; the acquired lease is safe
        // to roll back through Executor's destructor.
        candidate->thread_ = std::thread(&NativeWorker::loop, candidate.get());
        out = std::move(candidate);
        return Status::Ok;
    } catch (const std::bad_alloc &) {
        return Status::ResourceExhausted;
    } catch (...) {
        return Status::InternalFailure;
    }
}

NativeWorker::~NativeWorker() {
    // The env owner must retain this driver until its explicit cleanup job has
    // joined. Silently detaching or blocking an env destructor is forbidden.
    if (thread_.joinable())
        std::terminate();
}

Status NativeWorker::submit(Snapshot input, uint64_t &id) {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (stopping_)
            return Status::Closing;
    }
    auto status = engine_.start(input, id);
    if (status == Status::Ok) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            pendingWake_ = true;
        }
        changed_.notify_one();
    }
    return status;
}

void NativeWorker::request_stop() {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        stopping_ = true;
    }
    engine_.request_close();
    changed_.notify_one();
}

void NativeWorker::loop() noexcept {
    currentWorker = this;
    for (;;) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (stopping_)
                break;
            // Clear before inspecting Core so a submission after that inspection
            // cannot lose its wake. Multiple queued jobs need only one hint.
            pendingWake_ = false;
        }
        Work work;
        auto status = engine_.take_work(work);
        if (status == Status::Ok) {
            if (!work.cancelled()) {
                auto outcome = ports_.run(ports_.context, work.input());
                if (outcome == Status::Ok)
                    work.succeed();
                else if (outcome == Status::DomainFailure)
                    work.fail();
                // All other private-port results remain InternalFailure; finish
                // preserves a terminal cancellation already claimed by the caller.
            }
            work.finish();
            ports_.ready(ports_.context);
            continue;
        }
        if (status == Status::Closing || status == Status::Closed)
            break;
        if (status != Status::NotReady) {
            engine_.request_close();
            ports_.ready(ports_.context);
            break;
        }
        std::unique_lock<std::mutex> lock(mutex_);
        changed_.wait(lock, [&] { return stopping_ || pendingWake_; });
    }
}

Status NativeWorker::join_and_ack() noexcept {
    try {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (joined_)
                return Status::Ok;
        }
        // Check before the serialized join: another cleanup caller may hold it
        // while waiting for this very business thread. TLS avoids stale IDs.
        // The owning env thread must remain alive through cleanup-job admission
        // and completion; the platform async cleanup handle enforces that scope.
        if (currentWorker == this || std::this_thread::get_id() == owner_)
            return Status::InvalidArgument;
        std::lock_guard<std::mutex> singleJoin(joinMutex_);
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (!stopping_)
                return Status::InvalidArgument;
            if (joined_)
                return Status::Ok;
        }
        thread_.join();
        // std::thread::join synchronizes with thread completion, including TLS
        // destructors. Only this real witness returns the protocol's lease.
        executor_.finish();
        {
            std::lock_guard<std::mutex> lock(mutex_);
            joined_ = true;
        }
        return Status::Ok;
    } catch (...) {
        return Status::InternalFailure;
    }
}

bool NativeWorker::joined() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return joined_;
}
} // namespace madb::c07
