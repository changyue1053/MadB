// Actual gateway/Core/provider/worker ownership under a host Node-API return harness.
// The stubs below do not establish platform cleanup/TSFN or ArkTS behavior.
#define MADB_NAPI_OBSERVER_NO_WRAPPERS
#include "api.cpp"
#include "runtime.cpp"
#include <chrono>
#include <cstring>
#include <iostream>
#include <vector>
struct napi_value__ {
    std::string text;
    double number = 0;
    bool numeric = false;
};
struct napi_callback_info__ {
    std::vector<napi_value> values;
};
static std::vector<std::unique_ptr<napi_value__>> values;
static std::string lastError;
static bool failUndefined = false;
static uv_work_t *queuedJob = nullptr;
static uv_work_cb queuedWork = nullptr;
static uv_after_work_cb queuedAfter = nullptr;
static napi_value value(const std::string &text = "") {
    values.emplace_back(std::make_unique<napi_value__>());
    values.back()->text = text;
    return values.back().get();
}
extern "C" {
void napi_module_register(napi_module *) {}
napi_status napi_throw_error(napi_env, const char *code, const char *) {
    lastError = code;
    return napi_ok;
}
napi_status napi_get_cb_info(napi_env, napi_callback_info info, size_t *n, napi_value *out,
                             napi_value *, void **) {
    const auto count = std::min(*n, info->values.size());
    for (size_t i = 0; i < count; ++i)
        out[i] = info->values[i];
    *n = info->values.size();
    return napi_ok;
}
napi_status napi_get_value_string_utf8(napi_env, napi_value v, char *out, size_t capacity,
                                       size_t *length) {
    *length = v->text.size();
    if (out) {
        if (capacity <= *length)
            return napi_generic_failure;
        std::memcpy(out, v->text.c_str(), *length + 1);
    }
    return napi_ok;
}
napi_status napi_get_value_double(napi_env, napi_value v, double *out) {
    if (!v->numeric)
        return napi_number_expected;
    *out = v->number;
    return napi_ok;
}
napi_status napi_get_undefined(napi_env, napi_value *out) {
    if (failUndefined)
        return napi_generic_failure;
    *out = value();
    return napi_ok;
}
napi_status napi_call_threadsafe_function(napi_threadsafe_function, void *,
                                          napi_threadsafe_function_call_mode) {
    return napi_closing;
}
int uv_queue_work(uv_loop_t *, uv_work_t *request, uv_work_cb work, uv_after_work_cb after) {
    if (queuedJob)
        std::abort();
    queuedJob = request;
    queuedWork = work;
    queuedAfter = after;
    return 0;
}
}
#define require(v)                                                                                 \
    do {                                                                                           \
        if (!(v)) {                                                                                \
            std::cerr << "failed " << __LINE__ << ": " #v "\n";                                    \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
using namespace gateway;
using namespace madb::c07;
template <class F> static void eventually(F predicate) {
    auto end = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (!predicate()) {
        require(std::chrono::steady_clock::now() < end);
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}
struct Fixture {
    napi_env env = reinterpret_cast<napi_env>(uintptr_t{1});
    Owner owner;
    Handle runtime;
    Executor synchronousExecutor;
    bool synchronousStarted = false;
    Fixture() {
        auto &slot = slots[0];
        for (auto &c : slot.audit.c)
            c = 0;
        slot.audit.napi.reset(env, std::this_thread::get_id());
        slot.epoch = ++nextEpoch;
        slot.env = env;
        slot.thread = std::this_thread::get_id();
        slot.retired = false;
        owner = std::make_shared<Runtime>(0, slot.epoch, env, slot.audit);
        slot.owner = owner;
        owner->self = owner;
        owner->generation = 1;
        owner->generationRetired = false;
        require(GatewayProviderCreate(&owner->provider) == MP_OK);
        owner->engine = std::make_unique<Engine>(testProvider(*owner));
        runtime = {slot.epoch, 0, 0, 'r'};
    }
    ~Fixture() {
        failUndefined = false;
        require(!queuedJob);
        require(!owner->worker || owner->worker->joined());
        owner->worker.reset();
        require(owner->engine->stats().waiters == 0);
        owner->engine.reset();
        mp_stats s{};
        require(owner->provider.stats(owner->provider.context, &s) == MP_OK);
        require(s.live == 0 && s.allocations == s.releases);
        require(owner->provider.destroy_context(owner->provider.context) == MP_OK);
        owner->provider = {};
        slots[0].owner.reset();
        owner.reset();
        slots[0].retired = true;
    }
    napi_value close(double mode, bool numeric = true) {
        auto m = value();
        m->number = mode;
        m->numeric = numeric;
        napi_callback_info__ args{{value(encode(runtime)), m}};
        napi_observer::Scope authority(napi_observer::Mode::Entry, env);
        auto result = js_requestRuntimeClose(env, &args);
        if (result && synchronousStarted)
            synchronousExecutor.finish();
        return result;
    }
    uint64_t start(bool running = false) {
        static const unsigned char text[] = "actual-owned-model";
        if (running) {
            owner->test.gatePhase = 1;
            owner->test.gateArmed = true;
            require(NativeWorker::start(*owner->engine, {owner.get(), gateway::run, gateway::ready},
                                        owner->worker) == Status::Ok);
        }
        uint64_t id = 0;
        auto input = Snapshot{41, text, sizeof(text) - 1};
        if (owner->worker) {
            require(owner->worker->submit(input, id) == Status::Ok);
        } else {
            // Synchronous protocol fixture: no OS executor is launched here.
            // The separate gated-worker case proves real exit/join ordering.
            if (!synchronousStarted) {
                require(owner->engine->acquire_executor(synchronousExecutor) == Status::Ok);
                synchronousStarted = true;
            }
            require(owner->engine->start(input, id) == Status::Ok);
        }
        if (running)
            eventually([&] {
                std::lock_guard<std::mutex> l(owner->test.mutex);
                return owner->test.gateArrived;
            });
        auto *r = emptyResource(*owner);
        require(r);
        r->id = id;
        r->kind = 'o';
        return id;
    }
    Observation &claim(uint64_t id, uint64_t observationId) {
        auto *x = emptyObservation(*owner);
        require(x);
        x->id = observationId;
        x->target = {owner->epoch, 1, id, 'o'};
        x->result = true;
        ResultClaim claim;
        require(owner->engine->claim_result(id, claim) == Status::Ok);
        require(claim.commit(callback(*owner, x->target, observationId, false)) == Status::Ok);
        ++owner->audit.c[F::WaiterIndices];
        return *x;
    }
    void finishJoin() {
        require(queuedJob);
        {
            std::lock_guard<std::mutex> l(owner->test.mutex);
            owner->test.gateReleased = true;
            owner->test.changed.notify_all();
        }
        auto *job = queuedJob;
        auto work = queuedWork;
        auto after = queuedAfter;
        queuedJob = nullptr;
        std::thread cleanup([&] { work(job); });
        cleanup.join();
        after(job, 0);
        require(owner->worker->joined());
    }
};
static void strict_entry_before_side_effects() {
    Fixture f;
    for (double mode : {-1., 2., 0.5, std::numeric_limits<double>::infinity(),
                        std::numeric_limits<double>::quiet_NaN()}) {
        require(!f.close(mode));
        require(lastError == "InvalidArgument");
        require(!f.owner->runtimeClosing && !f.owner->abandonPending &&
                !f.owner->engine->stats().closing);
    }
    require(!f.close(1, false));
    require(lastError == "InvalidArgument");
    failUndefined = true;
    require(!f.close(1));
    require(!f.owner->runtimeClosing && !f.owner->abandonPending);
    failUndefined = false;
}
static void normal_claim_stays_deliverable() {
    Fixture f;
    auto id = f.start();
    auto &x = f.claim(id, 1);
    require(f.close(0));
    require(!f.owner->abandonPending && x.id == 1);
    require(f.owner->engine->stats().waiters == 1 && !f.owner->engine->stats().closed);
    ResultDelivery result;
    require(f.owner->engine->take_result(id, result) == Status::Ok);
    require(result.invoke() == Status::Cancelled);
    clear(x, *f.owner);
    require(f.owner->engine->stats().waiters == 0 && f.owner->engine->stats().closed);
}
static void failed_handoff_releases_all_and_cannot_downgrade() {
    Fixture f;
    for (int i = 1; i <= 3; ++i)
        f.claim(f.start(), i);
    auto *closeObserver = emptyObservation(*f.owner);
    require(closeObserver);
    closeObserver->id = 4;
    closeObserver->target = {f.owner->epoch, 1, 0, 'e'};
    closeObserver->result = false;
    require(f.owner->engine->watch_close(callback(*f.owner, closeObserver->target, 4, false),
                                         closeObserver->close) == Status::Ok);
    ++f.owner->audit.c[F::WaiterIndices];
    require(f.close(0));
    require(f.owner->engine->stats().waiters == 4);
    require(f.close(1));
    require(f.owner->abandonPending);
    require(f.owner->engine->stats().waiters == 0 && f.owner->audit.c[F::WaiterIndices] == 0);
    require(f.owner->audit.c[F::CallbackContextsLive] == 0 && f.owner->engine->stats().closed);
    require(f.close(0));
    require(f.owner->abandonPending);
}
static void entered_token_keeps_its_original_owner() {
    Fixture f;
    auto id = f.start();
    auto &x = f.claim(id, 1);
    f.owner->engine->close_operation(id);
    ResultDelivery active;
    require(f.owner->engine->take_result(id, active) == Status::Ok);
    x.entered = true;
    require(f.close(1));
    require(x.id == 1 && f.owner->engine->stats().waiters == 1);
    require(!f.owner->engine->stats().closed && f.owner->audit.c[F::CallbackContextsLive] == 1);
    active.discard();
    clear(x, *f.owner);
    require(progress(f.owner));
    require(f.owner->engine->stats().waiters == 0 && f.owner->engine->stats().closed);
}
static void not_ready_retains_real_claim() {
    Fixture f;
    f.claim(f.start(), 1);
    discardObservations(f.owner);
    require(f.owner->engine->stats().waiters == 1 && f.owner->audit.c[F::WaiterIndices] == 1);
    require(f.close(1));
    require(f.owner->engine->stats().waiters == 0);
}
static void no_new_claims_after_handoff() {
    Fixture f;
    auto id = f.start();
    auto &x = f.claim(id, 1);
    f.owner->engine->close_operation(id);
    ResultDelivery active;
    require(f.owner->engine->take_result(id, active) == Status::Ok);
    x.entered = true;
    require(f.close(1));
    napi_callback_info__ args{{value(encode(x.target)), value("2")}};
    napi_observer::Scope authority(napi_observer::Mode::Entry, f.env);
    require(!js_watchClose(f.env, &args) && lastError == "Closing");
    require(!js_claimResult(f.env, &args) && lastError == "Closing");
    require(f.owner->engine->stats().waiters == 1 &&
            f.owner->audit.c[F::CallbackContextsLive] == 1);
    active.discard();
    clear(x, *f.owner);
}
static void work_and_executor_require_actual_exit_join() {
    Fixture f;
    auto id = f.start(true);
    f.claim(id, 1);
    require(f.close(1));
    require(f.owner->engine->stats().waiters == 0);
    require(f.owner->engine->stats().work == 1 && f.owner->engine->stats().executors == 1);
    require(!f.owner->engine->stats().closed && !f.owner->worker->joined());
    require(f.owner->audit.c[F::MechanismTeardownStarted] == 0);
    f.finishJoin();
    require(f.owner->engine->stats().closed && f.owner->engine->stats().work == 0 &&
            f.owner->engine->stats().executors == 0);
    require(f.owner->audit.c[F::JoinCompleted] == 1 && f.owner->audit.c[F::JoinJobLive] == 0);
}
static void close_callback_reentry_keeps_entered_owner() {
    Fixture f;
    f.start(); // Hold the synchronous executor so watch_close owns a pending record.
    auto *x = emptyObservation(*f.owner);
    require(x);
    x->id = 1;
    x->target = {f.owner->epoch, 1, 0, 'e'};
    struct State {
        Fixture *fixture;
        Observation *observation;
        int calls = 0, drops = 0;
    } state{&f, x};
    require(f.owner->engine->watch_close(
                Callback(
                    &state,
                    [](void *p, Status, const void *) {
                        auto &s = *static_cast<State *>(p);
                        ++s.calls;
                        require(s.fixture->close(1));
                        require(s.observation->id == 1 && s.observation->entered);
                        require(s.fixture->owner->engine->stats().waiters == 1 && s.drops == 0);
                    },
                    [](void *p) noexcept { ++static_cast<State *>(p)->drops; }),
                x->close) == Status::Ok);
    ++f.owner->audit.c[F::WaiterIndices];
    require(f.close(0));
    require(x->close.ready());
    x->entered = true;
    require(x->close.invoke() == Status::Ok);
    require(state.calls == 1 && state.drops == 1);
    clear(*x, *f.owner);
    require(f.owner->engine->stats().waiters == 0 && f.owner->audit.c[F::WaiterIndices] == 0);
}
int main() {
    strict_entry_before_side_effects();
    normal_claim_stays_deliverable();
    failed_handoff_releases_all_and_cannot_downgrade();
    entered_token_keeps_its_original_owner();
    not_ready_retains_real_claim();
    no_new_claims_after_handoff();
    work_and_executor_require_actual_exit_join();
    close_callback_reentry_keeps_entered_owner();
    std::cout << "8 failed-handoff gateway checks passed; actual Core/provider/worker, host NAPI "
                 "return harness only\n";
}
