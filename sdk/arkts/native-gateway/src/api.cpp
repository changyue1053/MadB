#include "runtime.hpp"
#include <limits>
#include <stdexcept>
namespace gateway {
namespace {
struct ListenerFailure {};
struct Scratch {
    Runtime &r;
    explicit Scratch(Runtime &v) : r(v) {
        r.scratch = true;
        ++r.audit.c[F::ScratchLive];
        ++r.audit.c[F::ScratchAcquired];
    }
    ~Scratch() {
        r.scratch = false;
        --r.audit.c[F::ScratchLive];
        ++r.audit.c[F::ScratchReleased];
    }
};
Status providerStatus(mp_status s) {
    return s == MP_OK                   ? Status::Ok
           : s == MP_INVALID_ARGUMENT   ? Status::InvalidArgument
           : s == MP_ALLOCATION_FAILURE ? Status::ResourceExhausted
           : s == MP_DOMAIN_OVERFLOW    ? Status::DomainFailure
                                        : Status::InternalFailure;
}
Status run(void *p, void *model) noexcept {
    auto *r = static_cast<Runtime *>(p);
    return providerStatus(r->provider.increment(r->provider.context, model));
}
void ready(void *p) noexcept {
    auto *r = static_cast<Runtime *>(p);
    if (auto o = r->self.lock())
        wake(o);
}
Resource *emptyResource(Runtime &r) {
    for (auto &x : r.resources)
        if (!x.id)
            return &x;
    return nullptr;
}
Observation *emptyObservation(Runtime &r) {
    for (auto &x : r.observations)
        if (!x.id)
            return &x;
    return nullptr;
}
void clear(Observation &x, Runtime &r) {
    x.close.discard();
    x.id = 0;
    x.entered = false;
    --r.audit.c[F::WaiterIndices];
}
void requestTarget(const Owner &o, const Handle &h) {
    if (h.kind == 'e')
        requestEngineClose(o);
    else if (h.kind == 'o')
        o->engine->close_operation(h.id);
    else
        o->engine->stop(h.id);
    wake(o);
}
bool targetKind(napi_env env, const Handle &h) {
    if (h.kind == 'e' || h.kind == 'o' || h.kind == 's')
        return true;
    error(env, "InvalidHandle");
    return false;
}
bool stream(napi_env env, napi_value v, Stream &out) {
    double x;
    if (napi_get_value_double(env, v, &x) != napi_ok || (x != 0 && x != 1)) {
        error(env, "InvalidArgument");
        return false;
    }
    out = x == 0 ? Stream::State : Stream::Event;
    return true;
}
napi_value returnedHandle(napi_env env, const Owner &o, const Handle &h) {
    napi_value result = nullptr;
    try {
        result = string(env, encode(h));
    } catch (...) {
        result = nullptr;
    }
    if (result)
        return result;
    // Successful submit/observe may already exist. Close its real owner; never
    // report this as a pre-admission rollback or silently lose the issued id.
    requestTarget(o, h);
    bool pending = false;
    napi_is_exception_pending(env, &pending);
    if (pending) {
        napi_value e = nullptr;
        napi_get_and_clear_last_exception(env, &e);
    }
    return error(env, "AdmissionDeliveryFailure");
}
} // namespace
void callbackDrop(void *p) noexcept {
    auto *c = static_cast<CallbackContext *>(p);
    Audit *a = &c->owner->audit;
    delete c;
    --a->c[F::CallbackContextsLive];
}
Callback callback(Runtime &r, Handle target, uint64_t id, bool listener) {
    auto *c = new CallbackContext{&r, target, id, listener};
    ++r.audit.c[F::CallbackContextsLive];
    return Callback(c, callbackCall, callbackDrop);
}
void callbackCall(void *p, Status outcome, const void *model) {
    auto *c = static_cast<CallbackContext *>(p);
    Runtime &r = *c->owner;
    if (c->listener) {
        struct MarkFailure {
            Runtime &r;
            Handle target;
            int depth = std::uncaught_exceptions();
            ~MarkFailure() {
                if (std::uncaught_exceptions() > depth)
                    if (auto *x = resource(r, target))
                        x->closeOutcome = Status::CallbackFailure;
            }
        } mark{r, c->target};
        if (r.envClosing || r.shutting || std::this_thread::get_id() != r.thread)
            throw ListenerFailure{};
        ++r.audit.c[F::InDispatch];
        struct Leave {
            Runtime &r;
            ~Leave() {
                --r.audit.c[F::InDispatch];
            }
        } leave{r};
        mp_snapshot view{};
        napi_value copied = nullptr, fn = nullptr, receiver = nullptr, kind = nullptr,
                   key = nullptr, result = nullptr;
        auto status = r.provider.read_view(r.provider.context, model, &view) == MP_OK
                          ? napi_ok
                          : napi_generic_failure;
        if (status == napi_ok)
            status = copy(r.env, Status::Ok, view.value, view.text, view.size, copied);
        if (status == napi_ok)
            status = napi_get_reference_value(r.env, r.dispatcher, &fn);
        if (status == napi_ok)
            status = napi_get_undefined(r.env, &receiver);
        if (status == napi_ok)
            status = napi_create_int32(r.env, 1, &kind);
        const auto encoded = encode(c->target);
        if (status == napi_ok)
            status = napi_create_string_utf8(r.env, encoded.data(), encoded.size(), &key);
        napi_value argv[] = {kind, key, copied};
        if (status == napi_ok)
            status = napi_call_function(r.env, receiver, fn, 3, argv, &result);
        if (status != napi_ok) {
            if (status == napi_pending_exception) {
                napi_value e = nullptr;
                if (napi_get_and_clear_last_exception(r.env, &e) != napi_ok)
                    ++r.audit.c[F::ResidualErrors];
            }
            throw ListenerFailure{};
        }
        return;
    }
    if (auto *x = observation(r, c->observation))
        x->outcome = outcome;
    if (!r.copyTarget || r.copyTarget->observation != c->observation)
        return; // immediate close status only
    auto &t = *r.copyTarget;
    mp_snapshot view{0, reinterpret_cast<const uint8_t *>(""), 0};
    if (outcome == Status::Ok && model &&
        r.provider.read_view(r.provider.context, model, &view) != MP_OK) {
        t.status = napi_generic_failure;
        return;
    }
    t.status = copy(r.env, outcome, view.value, view.text, view.size, t.value);
}
void discardObservations(const Owner &o) {
    for (auto &x : o->observations)
        if (x.id && !x.entered) {
            if (x.result) {
                ResultDelivery delivery;
                auto s = o->engine->take_result(x.target.id, delivery);
                if (s == Status::NotReady)
                    continue;
                if (s != Status::Ok) {
                    ++o->audit.c[F::ResidualErrors];
                    continue;
                }
                delivery.discard();
            }
            clear(x, *o);
        }
    refresh(*o);
}

// MADB_NATIVE_CALLBACK_BEGIN
#define BEGIN(name, count)                                                                         \
    napi_value js_##name(napi_env env, napi_callback_info info) {                                  \
        try {                                                                                      \
            napi_value argv[count]{};                                                              \
            if (!arguments(env, info, count, argv))                                                \
                return nullptr;
// MADB_NATIVE_CALLBACK_END
#define END                                                                                        \
    }                                                                                              \
    catch (const std::bad_alloc &) {                                                               \
        return error(env, "ResourceExhausted");                                                    \
    }                                                                                              \
    catch (...) {                                                                                  \
        return error(env, "InternalFailure");                                                      \
    }                                                                                              \
    }
BEGIN(runtimeOpen, 2)
    return openRuntime(env, argv[0], argv[1]);
END
BEGIN(connect, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    auto o = find(env, h, 'r');
    if (!o)
        return nullptr;
    if (o->runtimeClosing)
        return error(env, "Closing");
    retireGeneration(o);
    if (o->engine || o->provider.context)
        return error(env, "Closing");
    if (o->generation == std::numeric_limits<uint64_t>::max())
        return error(env, "ResourceExhausted");
    auto status = GatewayProviderCreate(&o->provider);
    if (status != MP_OK)
        return error(env, providerStatus(status));
    ++o->audit.c[F::ProviderContextLive];
    ++o->generation;
    o->generationRetired = false;
    o->engineAcked = false;
    try {
        o->engine = std::make_unique<Engine>(madb::c07::probe::provider_port(o->provider));
    } catch (...) {
        o->provider.destroy_context(o->provider.context);
        o->provider = {};
        --o->audit.c[F::ProviderContextLive];
        o->generationRetired = true;
        throw;
    }
    ++o->audit.c[F::EnginesCreated];
    o->audit.c[F::WorkerJoined] = 0;
    auto s = NativeWorker::start(*o->engine, {o.get(), run, ready}, o->worker);
    if (s != Status::Ok) {
        o->engine.reset();
        ++o->audit.c[F::EnginesDestroyed];
        o->provider.destroy_context(o->provider.context);
        o->provider = {};
        --o->audit.c[F::ProviderContextLive];
        o->generationRetired = true;
        return error(env, s);
    }
    refresh(*o);
    return returnedHandle(env, o, {o->epoch, o->generation, 0, 'e'});
END
BEGIN(roundtrip, 3)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    auto o = find(env, h, 'e', true);
    if (!o)
        return nullptr;
    Entry entry(o);
    Input input;
    if (!snapshot(env, argv[1], argv[2], input))
        return nullptr;
    if (o->runtimeClosing || o->engine->stats().closing)
        return error(env, "Closing");
    if (o->scratch)
        return error(env, "ResourceExhausted");
    Scratch scratch(*o);
    void *model = nullptr;
    auto s = o->provider.clone(
        o->provider.context,
        {input.value, reinterpret_cast<const uint8_t *>(input.text.data()), input.text.size()},
        &model);
    struct Release {
        Runtime &r;
        void *&p;
        ~Release() {
            if (p && r.provider.release(r.provider.context, p) != MP_OK)
                std::terminate();
        }
    } release{*o, model};
    if (s != MP_OK)
        return error(env, providerStatus(s));
    if (!model)
        return error(env, "InternalFailure");
    mp_snapshot view{};
    s = o->provider.read_view(o->provider.context, model, &view);
    if (s != MP_OK)
        return error(env, providerStatus(s));
    napi_value result = nullptr;
    if (copy(env, Status::Ok, view.value, view.text, view.size, result) != napi_ok)
        return error(env, "InternalFailure");
    return result;
END
BEGIN(submit, 3)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    auto o = find(env, h, 'e', true);
    if (!o)
        return nullptr;
    Entry entry(o);
    Input input;
    if (!snapshot(env, argv[1], argv[2], input))
        return nullptr;
    if (o->runtimeClosing || o->engine->stats().closing)
        return error(env, "Closing");
    auto *slot = emptyResource(*o);
    if (!slot)
        return error(env, "ResourceExhausted");
    uint64_t id = 0;
    auto s = o->worker->submit(input.view(), id);
    if (s != Status::Ok)
        return error(env, s);
    *slot = {id, 'o', false};
    refresh(*o);
    return returnedHandle(env, o, {o->epoch, o->generation, id, 'o'});
END
BEGIN(cancel, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    auto o = find(env, h, 'o');
    if (!o)
        return nullptr;
    Entry entry(o);
    auto s = o->engine->cancel(h.id);
    if (s != Status::Ok && s != Status::Closed)
        return error(env, s);
    wake(o);
    refresh(*o);
    return undefined(env);
END
BEGIN(observe, 2)
    Handle h;
    Stream kind;
    if (!handle(env, argv[0], h) || !stream(env, argv[1], kind))
        return nullptr;
    auto o = find(env, h, 'e', true);
    if (!o)
        return nullptr;
    Entry entry(o);
    auto *slot = emptyResource(*o);
    if (!slot)
        return error(env, "ResourceExhausted");
    Handle target{o->epoch, o->generation, 0, 's'};
    auto cb = callback(*o, target, 0, true);
    auto *context = static_cast<CallbackContext *>(cb.context);
    uint64_t id = 0;
    auto s = o->engine->observe(kind, std::move(cb), id);
    if (s != Status::Ok)
        return error(env, s);
    context->target.id = id;
    *slot = {id, 's', true};
    refresh(*o);
    return returnedHandle(env, o, context->target);
END
BEGIN(publish, 4)
    Handle h;
    Stream kind;
    if (!handle(env, argv[0], h) || !stream(env, argv[1], kind))
        return nullptr;
    auto o = find(env, h, 'e', true);
    if (!o)
        return nullptr;
    Entry entry(o);
    Input input;
    if (!snapshot(env, argv[2], argv[3], input))
        return nullptr;
    if (o->runtimeClosing || o->engine->stats().closing)
        return error(env, "Closing");
    auto s = o->engine->publish(kind, input.view());
    if (s != Status::Ok)
        return error(env, s);
    refresh(*o);
    wake(o);
    return undefined(env);
END
BEGIN(requestClose, 1)
    Handle h;
    if (!handle(env, argv[0], h) || !targetKind(env, h))
        return nullptr;
    auto o = find(env, h);
    if (!o)
        return nullptr;
    Entry entry(o);
    requestTarget(o, h);
    refresh(*o);
    return undefined(env);
END
BEGIN(claimResult, 2)
    Handle h;
    uint64_t id;
    if (!handle(env, argv[0], h) || !observationId(env, argv[1], id))
        return nullptr;
    auto o = find(env, h, 'o');
    if (!o)
        return nullptr;
    if (o->abandonPending)
        return error(env, "Closing");
    Entry entry(o);
    if (id <= o->nextObservation)
        return error(env, "InvalidArgument");
    auto *x = emptyObservation(*o);
    if (!x)
        return error(env, "ResourceExhausted");
    napi_value accepted = undefined(env);
    if (!accepted)
        return nullptr;
    auto cb = callback(*o, h, id, false);
    ResultClaim claim;
    auto s = o->engine->claim_result(h.id, claim);
    if (s != Status::Ok)
        return error(env, s);
    s = claim.commit(std::move(cb));
    if (s != Status::Ok)
        return error(env, s);
    x->id = id;
    x->target = h;
    x->result = true;
    x->entered = false;
    o->nextObservation = id;
    ++o->audit.c[F::WaiterIndices];
    refresh(*o);
    wake(o);
    return accepted; // no fallible JS call after commit
END
BEGIN(watchClose, 2)
    Handle h;
    uint64_t id;
    if (!handle(env, argv[0], h) || !targetKind(env, h) || !observationId(env, argv[1], id))
        return nullptr;
    auto o = find(env, h);
    if (!o)
        return nullptr;
    if (o->abandonPending)
        return error(env, "Closing");
    Entry entry(o);
    requestTarget(o, h); // close request precedes observer admission, including capacity failure
    if (id <= o->nextObservation)
        return error(env, "InvalidArgument");
    auto *x = emptyObservation(*o);
    if (!x)
        return error(env, "ResourceExhausted");
    napi_value pending = nullptr;
    if (napi_create_int32(env, -1, &pending) != napi_ok)
        return error(env, "InternalFailure");
    auto cb = callback(*o, h, id, false);
    x->id = id;
    x->target = h;
    x->result = false;
    x->outcome = Status::Ok;
    const auto before = o->engine->stats().waiters;
    CloseWaiter token;
    auto s = h.kind == 'e'   ? o->engine->watch_close(std::move(cb), token)
             : h.kind == 'o' ? o->engine->watch_operation_close(h.id, std::move(cb), token)
                             : o->engine->watch_subscription_close(h.id, std::move(cb), token);
    if (s != Status::Ok) {
        x->id = 0;
        return error(env, s);
    }
    // Immediate receipts contain no Core ownership. invoke consumes only that
    // receipt; ready pending records still count in Core and cannot be tested
    // merely by ready(). Compare actual waiter delta instead.
    const bool owns = o->engine->stats().waiters > before;
    if (!owns) {
        auto outcome = token.invoke();
        x->id = 0;
        napi_value value = nullptr;
        if (napi_create_int32(env, static_cast<int32_t>(outcome), &value) != napi_ok)
            return error(env, "InternalFailure");
        return value;
    }
    x->close = std::move(token);
    o->nextObservation = id;
    ++o->audit.c[F::WaiterIndices];
    refresh(*o);
    wake(o);
    return pending;
END
BEGIN(tryTake, 2)
    Handle h;
    uint64_t id;
    if (!handle(env, argv[0], h) || !observationId(env, argv[1], id))
        return nullptr;
    auto o = find(env, h, 'r');
    if (!o)
        return nullptr;
    Entry entry(o);
    auto *x = observation(*o, id);
    if (!x || x->entered)
        return error(env, "TakeBeforeGuardResidual");
    ResultDelivery result;
    Status s;
    if (x->result) {
        s = o->engine->take_result(x->target.id, result);
        if (s != Status::Ok && s != Status::NotReady)
            return error(env, "TakeBeforeGuardResidual");
    } else
        s = x->close.ready() ? Status::Ok : Status::NotReady;
    if (s == Status::NotReady) {
        napi_value v = nullptr;
        if (napi_get_null(env, &v) != napi_ok)
            return error(env, "InternalFailure");
        return v;
    }
    x->entered = true;
    CopyTarget target{id, nullptr, napi_ok};
    o->copyTarget = &target;
    if (x->result)
        result.invoke();
    else
        x->close.invoke();
    o->copyTarget = nullptr;
    clear(*x, *o);
    refresh(*o);
    if (target.status != napi_ok || !target.value) {
        bool pending = false;
        napi_is_exception_pending(env, &pending);
        if (pending) {
            napi_value e = nullptr;
            if (napi_get_and_clear_last_exception(env, &e) != napi_ok)
                ++o->audit.c[F::ResidualErrors];
        }
        return error(env, "DeliveryFailureConsumed");
    }
    return target.value;
END
BEGIN(deliver, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    auto o = find(env, h, 's');
    if (!o)
        return nullptr;
    Entry entry(o);
    Delivery delivery;
    auto s = o->engine->take_delivery(h.id, delivery);
    if (s == Status::Ok)
        s = delivery.invoke();
    refresh(*o);
    napi_value result = nullptr;
    if (napi_create_int32(env, static_cast<int32_t>(s), &result) != napi_ok)
        return error(env, "DeliveryFailureConsumed");
    return result;
END
BEGIN(requestRuntimeClose, 2)
    Handle h;
    double completionMode = 0;
    if (!handle(env, argv[0], h))
        return nullptr;
    if (napi_get_value_double(env, argv[1], &completionMode) != napi_ok ||
        (completionMode != 0 && completionMode != 1))
        return error(env, "InvalidArgument");
    auto o = find(env, h, 'r');
    if (!o)
        return nullptr;
    // Prepare the return value before accepting the irreversible handoff.
    napi_value accepted = undefined(env);
    if (!accepted)
        return nullptr;
    closeRuntime(o, completionMode == 1);
    return accepted;
END
BEGIN(pollRuntime, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    return pollFrame(env, h, true);
END
BEGIN(consumeRuntimeReceipt, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    return frame(env, h, true);
END
BEGIN(audit, 1)
    Handle h;
    if (!handle(env, argv[0], h))
        return nullptr;
    return pollFrame(env, h, false);
END
#undef BEGIN
#undef END
// MADB_NATIVE_REGISTRAR_BEGIN

napi_value init(napi_env env, napi_value exports) {
#include "exports.generated.inc"
    if (napi_define_properties(env, exports, sizeof(methods) / sizeof(methods[0]), methods) !=
        napi_ok)
        return error(env, "InternalFailure");
    return exports;
}
static napi_module module{1, 0, nullptr, init, GatewayModuleName, nullptr, {0}};
__attribute__((constructor)) static void registerModule() {
    napi_module_register(&module);
}
// MADB_NATIVE_REGISTRAR_END
} // namespace gateway
