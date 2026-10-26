#include "runtime.hpp"
#include "identity.generated.hpp"
#include <limits>
#include <new>
#include <stdexcept>
namespace gateway {
namespace {
std::array<Slot, LiveRuntimes> slots;
struct Historical {
    uint64_t epoch = 0;
    std::array<uint64_t, FrameSize> values{};
};
std::array<Historical, History> history;
size_t historyNext = 0;
uint64_t nextEpoch = 0;
std::mutex registryMutex;
void failure(Runtime &r) {
    ++r.audit.c[F::ResidualErrors];
}
bool physical(const Audit &a) {
    return a.c[F::MechanismTeardownStarted] && !a.c[F::ContextLive] && !a.c[F::ExplicitRefLive] &&
           !a.c[F::TsfnOwnerLive] && !a.c[F::HookLive] && !a.c[F::WakeLive] &&
           !a.c[F::InDispatch] && !a.c[F::BusinessEntries] && !a.c[F::JoinJobLive] &&
           !a.c[F::ProviderContextLive] && !a.c[F::CallbackContextsLive] &&
           !a.c[F::WaiterIndices] && !a.c[F::ScratchLive] && !a.c[F::ResidualErrors] &&
           a.c[F::ContextCreated] == a.c[F::ContextDestroyed] &&
           a.c[F::RefCreated] == a.c[F::RefDeleted] &&
           a.c[F::TsfnCreated] == a.c[F::TsfnFinalized] &&
           a.c[F::HookAdded] == a.c[F::HookRetired] &&
           a.c[F::WakeCreated] == a.c[F::WakeReleased] &&
           a.c[F::JoinQueued] == a.c[F::JoinCompleted] &&
           a.c[F::EnginesCreated] == a.c[F::EnginesDestroyed] &&
           a.c[F::ScratchAcquired] == a.c[F::ScratchReleased];
}
void unlink(const Owner &o) {
    Owner drop;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        drop = std::move(slots[o->slot].owner);
    }
}
struct DeleteRuntime {
    void operator()(Runtime *p) const {
        Audit *a = &p->audit;
        delete p;
        --a->c[F::ContextLive];
        ++a->c[F::ContextDestroyed];
    }
};
struct Wake {
    Owner owner;
    explicit Wake(Owner o) : owner(std::move(o)) {
        ++owner->audit.c[F::WakeCreated];
        ++owner->audit.c[F::WakeLive];
    }
};
struct DeleteWake {
    void operator()(Wake *p) const {
        Audit *a = &p->owner->audit;
        delete p;
        --a->c[F::WakeLive];
        ++a->c[F::WakeReleased];
    }
};
bool clearException(Runtime &r, napi_status s) {
    if (s != napi_pending_exception)
        return true;
    napi_value e = nullptr;
    if (napi_get_and_clear_last_exception(r.env, &e) != napi_ok) {
        failure(r);
        return false;
    }
    return true;
}
void notify(const Owner &o) {
    if (o->envClosing || o->shutting || !o->dispatcher)
        return;
    ++o->audit.c[F::InDispatch];
    napi_value fn = nullptr, receiver = nullptr, kind = nullptr, key = nullptr, value = nullptr,
               result = nullptr;
    auto s = napi_get_reference_value(o->env, o->dispatcher, &fn);
    if (s == napi_ok)
        s = napi_get_undefined(o->env, &receiver);
    if (s == napi_ok)
        s = napi_create_int32(o->env, 0, &kind);
    if (s == napi_ok)
        s = napi_create_string_utf8(o->env, "", 0, &key);
    if (s == napi_ok)
        s = napi_get_null(o->env, &value);
    napi_value args[] = {kind, key, value};
    if (s == napi_ok)
        s = napi_call_function(o->env, receiver, fn, 3, args, &result);
    if (s != napi_ok) {
        ++o->audit.c[F::WakeFailures];
        clearException(*o, s);
    }
    --o->audit.c[F::InDispatch];
}
void wakeJs(napi_env env, napi_value, void *, void *data) {
    std::unique_ptr<Wake, DeleteWake> ticket(static_cast<Wake *>(data));
    if (!ticket)
        return;
    Owner o = ticket->owner;
    o->queued = false;
    if (!env || o->envClosing || o->shutting)
        return;
    if (env != o->env || std::this_thread::get_id() != o->thread) {
        failure(*o);
        return;
    }
    napi_handle_scope scope = nullptr;
    if (napi_open_handle_scope(env, &scope) != napi_ok) {
        failure(*o);
        return;
    }
    notify(o);
    if (napi_close_handle_scope(env, scope) != napi_ok)
        failure(*o);
}
void finalized(napi_env, void *data, void *) {
    std::unique_ptr<Owner> p(static_cast<Owner *>(data));
    Audit *a = &(*p)->audit;
    p.reset();
    --a->c[F::TsfnOwnerLive];
    ++a->c[F::TsfnFinalized];
}
bool deleteRef(const Owner &o) {
    if (!o->dispatcher)
        return true;
    if (napi_delete_reference(o->env, o->dispatcher) != napi_ok) {
        failure(*o);
        return false;
    }
    o->dispatcher = nullptr;
    --o->audit.c[F::ExplicitRefLive];
    ++o->audit.c[F::RefDeleted];
    return true;
}
bool removeHook(const Owner &o) {
    if (!o->hook)
        return true;
    if (napi_remove_async_cleanup_hook(o->hook) != napi_ok) {
        failure(*o);
        return false;
    }
    o->hook = nullptr;
    delete o->hookOwner;
    o->hookOwner = nullptr;
    --o->audit.c[F::HookLive];
    ++o->audit.c[F::HookRetired];
    return true;
}
void teardown(const Owner &o) {
    if (o->engine || o->provider.context || o->audit.c[F::ResidualErrors] || o->join.active ||
        o->audit.c[F::BusinessEntries] || o->audit.c[F::InDispatch])
        return;
    o->shutting = true;
    o->audit.c[F::MechanismTeardownStarted] = 1;
    if (!deleteRef(o))
        return;
    if (o->tsfn) {
        if (napi_release_threadsafe_function(o->tsfn, napi_tsfn_abort) != napi_ok) {
            failure(*o);
            return;
        }
        o->tsfn = nullptr;
    }
    // After worker/Core/ref retirement, TSFN's independent pure-native holders
    // may finish later. The async hook must not wait for a platform phase it blocks.
    if (!removeHook(o))
        return;
    unlink(o);
}
void joinWork(uv_work_t *req) {
    auto *j = static_cast<JoinJob *>(req->data);
    j->result = j->owner->worker->join_and_ack();
}
void afterJoin(uv_work_t *req, int status) {
    auto *j = static_cast<JoinJob *>(req->data);
    Owner o = j->owner;
    if (status != 0 || j->result != Status::Ok) {
        failure(*o);
        return;
    }
    j->active = false;
    j->owner.reset();
    --o->audit.c[F::JoinJobLive];
    ++o->audit.c[F::JoinCompleted];
    o->audit.c[F::WorkerJoined] = 1;
    if (o->envClosing || o->abandonPending)
        discardObservations(o);
    progress(o);
    if (!o->envClosing && !o->shutting)
        wake(o);
}
void asyncCleanup(napi_async_cleanup_hook_handle, void *data) {
    Owner o = *static_cast<Owner *>(data);
    o->envClosing = true;
    o->runtimeClosing = true;
    o->audit.c[F::EnvExit] = 1;
    o->audit.c[F::RuntimeCloseRequested] = 1;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        slots[o->slot].envEnded = true;
        slots[o->slot].retireWithoutObserver = true;
    }
    requestEngineClose(o);
    discardObservations(o);
    deleteRef(o);
    progress(o);
}
} // namespace
std::string identity() {
    return GatewayIdentity;
}
bool observationId(napi_env env, napi_value v, uint64_t &id) {
    char bytes[24]{};
    size_t len = 0, n = 0, p = 0;
    if (napi_get_value_string_utf8(env, v, nullptr, 0, &len) != napi_ok || len >= sizeof(bytes) ||
        napi_get_value_string_utf8(env, v, bytes, sizeof(bytes), &n) != napi_ok || n != len ||
        !decimal(std::string_view(bytes, n), p, id) || p != n) {
        error(env, "InvalidArgument");
        return false;
    }
    return true;
}
Observation *observation(Runtime &r, uint64_t id) {
    for (auto &x : r.observations)
        if (x.id == id)
            return &x;
    return nullptr;
}
Resource *resource(Runtime &r, const Handle &h) {
    for (auto &x : r.resources)
        if (x.id == h.id && x.kind == h.kind)
            return &x;
    return nullptr;
}
Owner find(napi_env env, const Handle &h, char expected, bool requireOpen) {
    Owner o;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        for (auto &s : slots)
            if (s.epoch == h.runtime && !s.retired) {
                if (s.env != env || s.thread != std::this_thread::get_id()) {
                    error(env, "WrongEnv");
                    return {};
                }
                o = s.owner;
                break;
            }
    }
    if (!o) {
        error(env, "Closed");
        return {};
    }
    if (expected && h.kind != expected) {
        error(env, "InvalidHandle");
        return {};
    }
    if (h.kind != 'r' && (!o->engine || h.generation != o->generation)) {
        error(env, "InvalidHandleRetired");
        return {};
    }
    if ((h.kind == 'o' || h.kind == 's') && !resource(*o, h)) {
        error(env, "InvalidHandleRetired");
        return {};
    }
    if (requireOpen && (o->runtimeClosing || !o->engine || o->engine->stats().closing)) {
        error(env, "Closing");
        return {};
    }
    if (o->envClosing || o->shutting) {
        error(env, "Closed");
        return {};
    }
    return o;
}
void refresh(Runtime &r) {
    auto &a = r.audit.c;
    if (r.engine) {
        auto s = r.engine->stats();
        a[F::Operations] = s.operations;
        a[F::Subscriptions] = s.subscriptions;
        a[F::EventTickets] = s.eventTickets;
        a[F::StateTickets] = s.stateTickets;
        a[F::Sources] = s.sources;
        a[F::Entered] = s.entered;
        a[F::Bytes] = s.bytes;
        a[F::Waiters] = s.waiters;
        a[F::Admissions] = s.admissions;
        a[F::Work] = s.work;
        a[F::Executors] = s.executors;
        a[F::CoreClosing] = s.closing;
        a[F::CoreClosed] = s.closed;
    } else {
        for (size_t i = F::Operations; i <= F::Executors; ++i)
            a[i] = 0;
        a[F::CoreClosed] = r.generationRetired ? 1 : 0;
    }
    if (r.provider.context) {
        mp_stats s{};
        if (r.provider.stats(r.provider.context, &s) == MP_OK) {
            a[F::ProviderAllocations] = s.allocations;
            a[F::ProviderReleases] = s.releases;
            a[F::ProviderModelsLive] = s.live;
            a[F::ProviderWorkCalls] = s.work_calls;
        } else
            ++a[F::ResidualErrors];
    }
    a[F::RuntimeCloseRequested] = r.runtimeClosing;
}
void wake(const Owner &o) noexcept {
    if (o->envClosing || o->shutting)
        return;
    bool expected = false;
    if (!o->queued.compare_exchange_strong(expected, true))
        return;
    auto *t = new (std::nothrow) Wake(o);
    if (!t) {
        o->queued = false;
        ++o->audit.c[F::WakeFailures];
        return;
    }
    auto s = napi_call_threadsafe_function(o->tsfn, t, napi_tsfn_nonblocking);
    if (s == napi_ok)
        return;
    ++o->audit.c[F::WakeFailures];
    if (s == napi_queue_full || s == napi_closing) {
        o->queued = false;
        DeleteWake{}(t);
    } else
        ++o->audit.c[F::ResidualErrors]; // possible post-insertion failure retains ticket
}
void requestEngineClose(const Owner &o) {
    if (!o->engine)
        return;
    o->engine->request_close();
    if (o->worker && !o->worker->joined() && !o->join.active) {
        o->worker->request_stop();
        o->join.owner = o;
        o->join.request.data = &o->join;
        o->join.active = true;
        int s = uv_queue_work(o->loop, &o->join.request, joinWork, afterJoin);
        if (s != 0) {
            o->join.active = false;
            o->join.owner.reset();
            failure(*o);
            return;
        }
        ++o->audit.c[F::JoinQueued];
        ++o->audit.c[F::JoinJobLive];
    }
    refresh(*o);
    wake(o);
}
void retireGeneration(const Owner &o) {
    if (!o->engine)
        return;
    refresh(*o);
    const auto s = o->engine->stats();
    if (!o->engineAcked || !s.closed || s.waiters || o->audit.c[F::BusinessEntries] ||
        o->audit.c[F::CallbackContextsLive] || o->join.active || !o->worker ||
        !o->worker->joined() || o->audit.c[F::WaiterIndices])
        return;
    o->worker.reset();
    o->engine.reset();
    ++o->audit.c[F::EnginesDestroyed];
    if (o->provider.context) {
        if (o->provider.destroy_context(o->provider.context) != MP_OK) {
            failure(*o);
            return;
        }
        o->provider = {};
        --o->audit.c[F::ProviderContextLive];
    }
    o->generationRetired = true;
    for (auto &r : o->resources)
        r = {};
    refresh(*o);
}
bool closeNotifications(const Owner &o) {
    if (!o->engine)
        return true;
    auto dispatch = [&](const Handle &h, Status outcome) {
        if (o->envClosing)
            return true;
        ++o->audit.c[F::InDispatch];
        struct Leave {
            Runtime &r;
            ~Leave() {
                --r.audit.c[F::InDispatch];
            }
        } leave{*o};
        napi_value fn = nullptr, receiver = nullptr, kind = nullptr, key = nullptr, value = nullptr,
                   result = nullptr;
        const auto text = encode(h);
        auto s = copy(o->env, outcome, 0, reinterpret_cast<const unsigned char *>(""), 0, value);
        if (s == napi_ok)
            s = napi_get_reference_value(o->env, o->dispatcher, &fn);
        if (s == napi_ok)
            s = napi_get_undefined(o->env, &receiver);
        if (s == napi_ok)
            s = napi_create_int32(o->env, 2, &kind);
        if (s == napi_ok)
            s = napi_create_string_utf8(o->env, text.data(), text.size(), &key);
        napi_value argv[] = {kind, key, value};
        if (s == napi_ok)
            s = napi_call_function(o->env, receiver, fn, 3, argv, &result);
        if (s != napi_ok) {
            ++o->audit.c[F::WakeFailures];
            clearException(*o, s);
            return false;
        }
        return true;
    };
    // A bounded immutable close outcome remains here until the corresponding JS
    // wrapper has received the actual ACK. Core slot reuse alone never frees it.
    for (auto &r : o->resources)
        if (r.id) {
            const bool done = r.kind == 'o' ? o->engine->operation_closed(r.id)
                                            : o->engine->subscription_closed(r.id);
            if (done) {
                if (!dispatch({o->epoch, o->generation, r.id, r.kind}, r.closeOutcome))
                    return false;
                r = {};
            }
        }
    auto s = o->engine->stats();
    bool children = false;
    for (const auto &r : o->resources)
        children |= r.id != 0;
    if (!o->engineAcked && s.closed && !s.waiters && !children && o->worker &&
        o->worker->joined() && !o->join.active && !o->audit.c[F::BusinessEntries] &&
        !o->audit.c[F::InDispatch] && !o->audit.c[F::CallbackContextsLive]) {
        if (!dispatch({o->epoch, o->generation, 0, 'e'}, Status::Ok))
            return false;
        o->engineAcked = true;
    }
    return true;
}
bool progress(const Owner &o, bool notify) {
    if (o->envClosing || o->abandonPending)
        discardObservations(o);
    if ((o->envClosing || notify) && !closeNotifications(o))
        return false;
    retireGeneration(o);
    refresh(*o);
    if (o->runtimeClosing && !o->engine && !o->audit.c[F::BusinessEntries] &&
        !o->audit.c[F::ScratchLive])
        teardown(o);
    return true;
}
void closeRuntime(const Owner &o, bool abandonPending) {
    o->abandonPending = o->abandonPending || abandonPending;
    o->runtimeClosing = true;
    o->audit.c[F::RuntimeCloseRequested] = 1;
    requestEngineClose(o);
    progress(o);
}
napi_value pollFrame(napi_env env, const Handle &h, bool advance) {
    Owner o;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        for (auto &s : slots)
            if (s.epoch == h.runtime && !s.retired) {
                if (h.kind != 'r' || s.env != env || s.thread != std::this_thread::get_id())
                    return error(env, "WrongEnv");
                o = s.owner;
                break;
            }
    }
    if (o) {
        if (advance && !o->shutting && !progress(o, true))
            return error(env, "CloseAcknowledgementFailure");
        refresh(*o);
    }
    return frame(env, h, false);
}
napi_value frame(napi_env env, const Handle &h, bool consume) {
    std::array<uint64_t, FrameSize> values{};
    size_t index = LiveRuntimes;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        for (size_t i = 0; i < LiveRuntimes; ++i)
            if (slots[i].epoch == h.runtime && !slots[i].retired) {
                index = i;
                break;
            }
        if (index == LiveRuntimes || h.kind != 'r')
            return error(env, "InvalidHandleRetired");
        auto &s = slots[index];
        if (s.env != env || s.thread != std::this_thread::get_id())
            return error(env, "WrongEnv");
        const bool done = physical(s.audit);
        if (consume && !done)
            return error(env, "NotReady");
        for (size_t i = 0; i < FrameSize; ++i)
            values[i] = s.audit.c[i];
        values[F::Format] = 1;
        values[F::Phase] = done                                  ? 3
                           : values[F::MechanismTeardownStarted] ? 2
                           : values[F::RuntimeCloseRequested]    ? 1
                                                                 : 0;
        if (consume)
            values[F::ReceiptConsumed] = 1;
    }
    napi_value result = nullptr;
    if (napi_create_array_with_length(env, FrameSize, &result) != napi_ok)
        return error(env, "InternalFailure");
    for (size_t i = 0; i < FrameSize; ++i) {
        napi_value v = nullptr;
        auto key = std::to_string(i);
        if (napi_create_double(env, static_cast<double>(values[i]), &v) != napi_ok)
            return error(env, "InternalFailure");
        napi_property_descriptor field{key.c_str(), nullptr, nullptr,         nullptr,
                                       nullptr,     v,       napi_enumerable, nullptr};
        if (napi_define_properties(env, result, 1, &field) != napi_ok)
            return error(env, "InternalFailure");
    }
    if (consume) {
        std::lock_guard<std::mutex> l(registryMutex);
        auto &s = slots[index];
        if (s.epoch != h.runtime || s.retired || !physical(s.audit))
            return error(env, "InvalidHandleRetired");
        s.audit.c[F::ReceiptConsumed] = 1;
        s.retired = true;
    }
    return result;
}
napi_value openRuntime(napi_env env, napi_value expected, napi_value dispatcher) {
    std::array<char, 224> id{};
    size_t len = 0, actual = 0;
    napi_valuetype type;
    constexpr std::string_view prefix = "madb-arkts/1:";
    if (napi_get_value_string_utf8(env, expected, nullptr, 0, &actual) != napi_ok ||
        actual != prefix.size() + 194 ||
        napi_get_value_string_utf8(env, expected, id.data(), id.size(), &len) != napi_ok ||
        len != actual)
        return error(env, "InvalidArgument");
    std::string_view input(id.data(), len);
    if (input.substr(0, prefix.size()) != prefix)
        return error(env, "InvalidArgument");
    std::array<std::string_view, 3> parts{};
    for (size_t part = 0; part < 3; ++part) {
        const size_t start = prefix.size() + 65 * part;
        parts[part] = input.substr(start, 64);
        for (char c : parts[part])
            if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')))
                return error(env, "InvalidArgument");
        if (part < 2 && input[start + 64] != ':')
            return error(env, "InvalidArgument");
    }
    if (parts[0] != GatewayAbi)
        return error(env, "AbiMismatch");
    if (parts[1] != GatewayProfile)
        return error(env, "ProfileMismatch");
    if (parts[2] != GatewayBuild)
        return error(env, "ArtifactMismatch");
    if (napi_typeof(env, dispatcher, &type) != napi_ok || type != napi_function)
        return error(env, "InvalidArgument");
    size_t index = LiveRuntimes;
    uint64_t epoch = 0;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        for (auto &s : slots)
            if (!s.retired && s.env == env && !(s.retireWithoutObserver && physical(s.audit)))
                return error(env, "RuntimeAlreadyOpen");
        for (size_t i = 0; i < LiveRuntimes; ++i)
            if (slots[i].retired || (slots[i].retireWithoutObserver && physical(slots[i].audit))) {
                index = i;
                break;
            }
        if (index == LiveRuntimes || nextEpoch == std::numeric_limits<uint64_t>::max())
            return error(env, "ResourceExhausted");
        auto &s = slots[index];
        if (s.epoch) {
            auto &old = history[historyNext++ % History];
            old.epoch = s.epoch;
            for (size_t i = 0; i < FrameSize; ++i)
                old.values[i] = s.audit.c[i];
        }
        for (auto &v : s.audit.c)
            v = 0;
        s.epoch = epoch = ++nextEpoch;
        s.env = env;
        s.thread = std::this_thread::get_id();
        s.retired = false;
        s.envEnded = false;
        s.retireWithoutObserver = false;
    }
    struct Reservation {
        size_t index;
        bool adopted = false;
        ~Reservation() {
            if (!adopted) {
                std::lock_guard<std::mutex> l(registryMutex);
                slots[index].retired = true;
            }
        }
    } reservation{index};
    napi_value result = string(env, encode({epoch, 0, 0, 'r'}));
    if (!result)
        return nullptr;
    Owner o;
    try {
        o = Owner(new Runtime(index, epoch, env, slots[index].audit), DeleteRuntime{});
    } catch (...) {
        std::lock_guard<std::mutex> l(registryMutex);
        slots[index].retired = true;
        return error(env, "ResourceExhausted");
    }
    o->self = o;
    {
        std::lock_guard<std::mutex> l(registryMutex);
        slots[index].owner = o;
        reservation.adopted = true;
    }
    auto rollback = [&](const char *code) {
        {
            std::lock_guard<std::mutex> l(registryMutex);
            slots[index].retireWithoutObserver = true;
        }
        o->runtimeClosing = true;
        progress(o);
        return error(env, code);
    };
    if (napi_get_uv_event_loop(env, &o->loop) != napi_ok)
        return rollback("InternalFailure");
    if (napi_create_reference(env, dispatcher, 1, &o->dispatcher) != napi_ok)
        return rollback("ResourceExhausted");
    ++o->audit.c[F::ExplicitRefLive];
    ++o->audit.c[F::RefCreated];
    napi_value name = nullptr;
    if (napi_create_string_utf8(env, "madb-c07-runtime", NAPI_AUTO_LENGTH, &name) != napi_ok)
        return rollback("InternalFailure");
    auto *hold = new (std::nothrow) Owner(o);
    if (!hold)
        return rollback("ResourceExhausted");
    if (napi_create_threadsafe_function(env, nullptr, nullptr, name, 1, 1, hold, finalized, nullptr,
                                        wakeJs, &o->tsfn) != napi_ok) {
        delete hold;
        return rollback("ResourceExhausted");
    }
    ++o->audit.c[F::TsfnOwnerLive];
    ++o->audit.c[F::TsfnCreated];
    o->hookOwner = new (std::nothrow) Owner(o);
    if (!o->hookOwner)
        return rollback("ResourceExhausted");
    if (napi_add_async_cleanup_hook(env, asyncCleanup, o->hookOwner, &o->hook) != napi_ok) {
        delete o->hookOwner;
        o->hookOwner = nullptr;
        return rollback("ResourceExhausted");
    }
    ++o->audit.c[F::HookLive];
    ++o->audit.c[F::HookAdded];
    return result;
}
} // namespace gateway
