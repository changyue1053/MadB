#pragma once
#include "codec.hpp"
#include "frame.generated.hpp"
#include "provider_port.hpp"
#include "worker.hpp"
#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <thread>
#include <uv.h>
namespace gateway {
using namespace madb::c07;
using F = Frame;
constexpr size_t LiveRuntimes = 16, History = 16, FrameSize = 54;
struct Runtime;
using Owner = std::shared_ptr<Runtime>;
struct Audit {
    std::array<std::atomic<uint64_t>, FrameSize> c{};
};
struct Slot {
    uint64_t epoch = 0;
    napi_env env = nullptr;
    std::thread::id thread;
    bool retired = true, envEnded = false, retireWithoutObserver = false;
    Audit audit;
    Owner owner;
};
struct Observation {
    uint64_t id = 0;
    Handle target;
    bool result = false, entered = false;
    Status outcome = Status::Ok;
    CloseWaiter close;
};
struct Resource {
    uint64_t id = 0;
    char kind = 0;
    bool callbackAlive = false;
    Status closeOutcome = Status::Ok;
};
struct CopyTarget {
    uint64_t observation = 0;
    napi_value value = nullptr;
    napi_status status = napi_ok;
};
struct JoinJob {
    uv_work_t request{};
    Owner owner;
    Status result = Status::InternalFailure;
    bool active = false;
};
struct Runtime {
    size_t slot = 0;
    uint64_t epoch = 0, generation = 0, nextObservation = 0;
    napi_env env = nullptr;
    std::thread::id thread = std::this_thread::get_id();
    Audit &audit;
    std::weak_ptr<Runtime> self;
    napi_ref dispatcher = nullptr;
    napi_threadsafe_function tsfn = nullptr;
    napi_async_cleanup_hook_handle hook = nullptr;
    Owner *hookOwner = nullptr;
    uv_loop_t *loop = nullptr;
    JoinJob join;
    std::atomic<bool> envClosing{false}, shutting{false}, queued{false};
    bool runtimeClosing = false, scratch = false, generationRetired = true, engineAcked = false;
    // Once a failed facade hands off completions, delivery cannot be restored.
    bool abandonPending = false;
    mp_provider provider{};
    std::unique_ptr<Engine> engine;
    std::unique_ptr<NativeWorker> worker;
    std::array<Observation, 32> observations;
    std::array<Resource, 16> resources;
    CopyTarget *copyTarget = nullptr;
    Runtime(size_t s, uint64_t e, napi_env n, Audit &a) : slot(s), epoch(e), env(n), audit(a) {
        ++audit.c[F::ContextCreated];
        ++audit.c[F::ContextLive];
    }
};
struct Entry {
    Owner owner;
    explicit Entry(Owner o) : owner(std::move(o)) {
        ++owner->audit.c[F::BusinessEntries];
    }
    ~Entry() {
        --owner->audit.c[F::BusinessEntries];
    }
};
struct CallbackContext {
    Runtime *owner;
    Handle target;
    uint64_t observation = 0;
    bool listener = false;
};
void callbackDrop(void *) noexcept;
void callbackCall(void *, Status, const void *);
Callback callback(Runtime &, Handle, uint64_t, bool);
Owner find(napi_env, const Handle &, char expected = 0, bool open = false);
bool observationId(napi_env, napi_value, uint64_t &);
Observation *observation(Runtime &, uint64_t);
Resource *resource(Runtime &, const Handle &);
void refresh(Runtime &);
void wake(const Owner &) noexcept;
void requestEngineClose(const Owner &);
bool progress(const Owner &, bool notify = false);
bool closeNotifications(const Owner &);
void retireGeneration(const Owner &);
void discardObservations(const Owner &);
napi_value frame(napi_env, const Handle &, bool consume = false);
napi_value pollFrame(napi_env, const Handle &, bool advance);
napi_value openRuntime(napi_env, napi_value, napi_value);
void closeRuntime(const Owner &, bool abandonPending);
std::string identity();
} // namespace gateway
