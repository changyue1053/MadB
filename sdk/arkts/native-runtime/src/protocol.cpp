#include "protocol.hpp"
#include <array>
#include <cstdlib>
#include <limits>
#include <mutex>
#include <new>
#include <utility>

namespace madb::c07 {
namespace {
// Internal ownership corruption must fail closed in Release as well as Debug.
void invariant(bool condition) noexcept {
    if (!condition)
        std::abort();
}
constexpr size_t N = 8, B = 4096, W = 32;
enum class Phase { Queued, Running, Exited };
enum class Claim { None, Reserved, Committed, Taken, Finished };
Status outcome(Winner w) {
    switch (w) {
    case Winner::Success:
        return Status::Ok;
    case Winner::Cancelled:
        return Status::Cancelled;
    case Winner::DomainFailure:
        return Status::DomainFailure;
    default:
        return Status::InternalFailure;
    }
}
} // namespace
Callback::Callback(Callback &&x) noexcept { *this = std::move(x); }
Callback &Callback::operator=(Callback &&x) noexcept {
    if (this != &x) {
        reset();
        context = std::exchange(x.context, nullptr);
        call = std::exchange(x.call, nullptr);
        destroy = std::exchange(x.destroy, nullptr);
    }
    return *this;
}
Callback::~Callback() { reset(); }
void Callback::reset() noexcept {
    auto p = std::exchange(context, nullptr);
    auto fn = std::exchange(destroy, nullptr);
    call = nullptr;
    if (fn)
        fn(p);
}
bool valid_snapshot(Snapshot s) noexcept {
    if (s.value < std::numeric_limits<int32_t>::min() ||
        s.value > std::numeric_limits<int32_t>::max() || s.size > 1024 || (s.size && !s.text))
        return false;
    size_t i = 0;
    while (i < s.size) {
        const uint32_t a = s.text[i++];
        if (a < 0x80)
            continue;
        unsigned trailing = 0;
        uint32_t code = 0, minimum = 0;
        if (a >= 0xC2 && a <= 0xDF) {
            trailing = 1;
            code = a & 31;
            minimum = 0x80;
        } else if (a >= 0xE0 && a <= 0xEF) {
            trailing = 2;
            code = a & 15;
            minimum = 0x800;
        } else if (a >= 0xF0 && a <= 0xF4) {
            trailing = 3;
            code = a & 7;
            minimum = 0x10000;
        } else
            return false;
        if (s.size - i < trailing)
            return false;
        while (trailing--) {
            const uint32_t b = s.text[i++];
            if ((b & 0xC0) != 0x80)
                return false;
            code = (code << 6) | (b & 63);
        }
        if (code < minimum || code > 0x10FFFF || (code >= 0xD800 && code <= 0xDFFF))
            return false;
    }
    return true;
}
struct Core : std::enable_shared_from_this<Core> {
    explicit Core(Provider value) : provider(value) {}
    struct Op {
        uint64_t id = 0;
        bool active = false, closing = false, initializing = false;
        Phase phase = Phase::Exited;
        Winner winner = Winner::Pending;
        Claim claim = Claim::None;
        size_t payloads = 0;
        Payload input, stored;
        Callback callback;
    };
    struct Sub {
        uint64_t id = 0;
        bool active = false, open = false, releasing = false;
        Stream stream = Stream::Event;
        size_t outstanding = 0, inFlight = 0, entered = 0;
        Status error = Status::Ok;
        Callback callback;
        std::array<Payload, N> queue;
        size_t count = 0;
        std::array<uint64_t, N> sourceOrders{};
    };
    struct CloseRecord {
        uint64_t id = 0, target = 0;
        int kind = 0;
        bool active = false, ready = false;
        Status outcome = Status::Ok;
    };
    std::array<CloseRecord, W> closeRecords;
    uint64_t nextObserver = 1;
    mutable std::mutex mutex;
    Provider provider;
    std::array<Op, N> ops;
    std::array<Sub, N> subs;
    uint64_t next = 1, nextOrder = 1;
    size_t bytes = 0, eventTickets = 0, stateTickets = 0, sources = 0, entered = 0, waiters = 0,
           admissions = 0, work = 0, executors = 0;
    bool closing = false, closed = false, executorTaken = false;
    Op *op(uint64_t id) {
        for (auto &o : ops)
            if (o.id == id && id)
                return &o;
        return nullptr;
    }
    Sub *sub(uint64_t id) {
        for (auto &s : subs)
            if (s.id == id && id)
                return &s;
        return nullptr;
    }
    Status cloneInto(Payload &payload, Snapshot input) noexcept {
        // Caller already owns the permit and holds no protocol lock. Adopt first:
        // an error result may still transfer a cleanup-owned provider allocation.
        invariant(payload.charged_ && !payload.object_);
        auto result = provider.clone(provider.context, input);
        payload.object_ = result.owned;
        switch (result.status) {
        case CloneStatus::Ok:
            return result.owned ? Status::Ok : Status::InternalFailure;
        case CloneStatus::InvalidArgument:
            return Status::InvalidArgument;
        case CloneStatus::AllocationFailure:
            return Status::ResourceExhausted;
        case CloneStatus::InternalFailure:
            return Status::InternalFailure;
        }
        return Status::InternalFailure;
    }
    void reserve(Payload &p, size_t n, uint64_t owner, bool isSub, bool event, bool source,
                 uint64_t order = 0) {
        invariant(!p.charged_);
        p.core_ = shared_from_this();
        p.bytes_ = n;
        p.owner_ = owner;
        p.subscription_ = isSub;
        p.event_ = event;
        p.source_ = source;
        p.order_ = order;
        p.charged_ = true;
        bytes += n;
        if (isSub) {
            ++sub(owner)->outstanding;
            if (event)
                ++eventTickets;
            else
                ++stateTickets;
        } else
            ++op(owner)->payloads;
        if (source) {
            ++sources;
            auto *s = sub(owner);
            bool saved = false;
            for (auto &value : s->sourceOrders)
                if (!value) {
                    value = order;
                    saved = true;
                    break;
                }
            invariant(saved);
        }
    }
    void unsource(const Payload &p) {
        invariant(sources);
        --sources;
        auto *s = sub(p.owner_);
        bool found = false;
        for (auto &value : s->sourceOrders)
            if (value == p.order_) {
                value = 0;
                found = true;
                break;
            }
        invariant(found);
    }
    void retire(const Payload &p) {
        {
            std::lock_guard<std::mutex> l(mutex);
            invariant(bytes >= p.bytes_);
            bytes -= p.bytes_;
            if (p.subscription_) {
                auto *s = sub(p.owner_);
                invariant(s && s->outstanding);
                --s->outstanding;
                if (p.event_) {
                    invariant(eventTickets);
                    --eventTickets;
                } else {
                    invariant(stateTickets);
                    --stateTickets;
                }
                if (p.inFlight_) {
                    invariant(s->inFlight);
                    --s->inFlight;
                }
                if (p.entered_) {
                    invariant(s->entered && entered);
                    --s->entered;
                    --entered;
                }
            } else {
                auto *o = op(p.owner_);
                invariant(o && o->payloads);
                --o->payloads;
            }
            if (p.source_)
                unsource(p);
        }
        drain();
    }
    CloseRecord *closeRecord(uint64_t id) {
        for (auto &record : closeRecords)
            if (record.active && record.id == id)
                return &record;
        return nullptr;
    }
    void latch(int kind, uint64_t target, Status status) {
        for (auto &record : closeRecords)
            if (record.active && record.kind == kind && record.target == target && !record.ready) {
                record.ready = true;
                record.outcome = status;
            }
    }
    Status registerClose(int kind, uint64_t target, uint64_t &recordId, bool &immediate) {
        std::lock_guard<std::mutex> l(mutex);
        bool ready = false;
        Status status = Status::Ok;
        if (!kind)
            ready = closed;
        else if (kind == 1) {
            auto *o = op(target);
            ready = !o || !o->active;
        } else {
            auto *s = sub(target);
            ready = !s || !s->active;
            if (s)
                status = s->error;
        }
        if (ready) {
            immediate = true;
            return status;
        }
        if (waiters == W || nextObserver == std::numeric_limits<uint64_t>::max())
            return Status::ResourceExhausted;
        for (auto &record : closeRecords)
            if (!record.active) {
                record = {nextObserver++, target, kind, true, false, Status::Ok};
                recordId = record.id;
                ++waiters;
                return Status::Ok;
            }
        invariant(false);
        return Status::InternalFailure;
    }
    void drain() {
        // External context release is deliberately outside the protocol lock.
        for (;;) {
            Callback release;
            uint64_t subId = 0;
            {
                std::lock_guard<std::mutex> l(mutex);
                for (auto &o : ops)
                    if (o.active && o.closing && o.phase == Phase::Exited && !o.payloads &&
                        o.claim != Claim::Reserved && o.claim != Claim::Committed &&
                        o.claim != Claim::Taken) {
                        latch(1, o.id, Status::Ok);
                        o.active = false;
                    }
                for (auto &s : subs)
                    if (s.active && !s.open && !s.outstanding && !s.inFlight && !s.releasing) {
                        s.releasing = true;
                        subId = s.id;
                        release = std::move(s.callback);
                        break;
                    }
                if (!subId) {
                    bool children = false;
                    for (auto &o : ops)
                        children |= o.active;
                    for (auto &s : subs)
                        children |= s.active;
                    if (closing && !children && !admissions && !bytes && !work && !executors) {
                        closed = true;
                        latch(0, 0, Status::Ok);
                    }
                    return;
                }
            }
            release.reset();
            {
                std::lock_guard<std::mutex> l(mutex);
                auto *s = sub(subId);
                invariant(s && s->releasing);
                s->releasing = false;
                latch(2, s->id, s->error);
                s->active = false;
            }
        }
    }
    void stop(uint64_t id) {
        std::array<Payload, N> garbage;
        {
            std::lock_guard<std::mutex> l(mutex);
            auto *s = sub(id);
            if (!s || !s->active)
                return;
            s->open = false;
            for (size_t i = 0; i < s->count; ++i)
                garbage[i] = std::move(s->queue[i]);
            s->count = 0;
        }
        for (auto &p : garbage)
            p.reset();
        drain();
    }
    void closeOp(uint64_t id) {
        Payload input, stored;
        {
            std::lock_guard<std::mutex> l(mutex);
            auto *o = op(id);
            if (!o || !o->active)
                return;
            o->closing = true;
            if (o->winner == Winner::Pending)
                o->winner = Winner::Cancelled;
            if (o->phase == Phase::Queued) {
                o->phase = Phase::Exited;
                invariant(work);
                --work;
                input = std::move(o->input);
            }
            if (o->claim == Claim::None || o->claim == Claim::Finished)
                stored = std::move(o->stored);
        }
        input.reset();
        stored.reset();
        drain();
    }
    bool ready(int kind, uint64_t id) {
        std::lock_guard<std::mutex> l(mutex);
        if (!kind)
            return closed;
        if (kind == 1) {
            auto *o = op(id);
            return !o || !o->active;
        }
        auto *s = sub(id);
        return !s || !s->active;
    }
};
Payload::Payload(Payload &&p) noexcept { *this = std::move(p); }
Payload &Payload::operator=(Payload &&p) noexcept {
    if (this != &p) {
        reset();
        core_ = std::move(p.core_);
        object_ = std::exchange(p.object_, nullptr);
        bytes_ = p.bytes_;
        owner_ = p.owner_;
        order_ = p.order_;
        subscription_ = p.subscription_;
        event_ = p.event_;
        source_ = p.source_;
        inFlight_ = p.inFlight_;
        entered_ = p.entered_;
        charged_ = std::exchange(p.charged_, false);
    }
    return *this;
}
Payload::~Payload() { reset(); }
void Payload::reset() noexcept {
    if (!charged_)
        return;
    auto core = core_;
    auto ptr = std::exchange(object_, nullptr);
    if (ptr)
        core->provider.destroy(core->provider.context,
                               ptr); // Still charged throughout provider release.
    core->retire(*this);
    charged_ = false;
    core_.reset();
}
Executor::Executor(Executor &&x) noexcept : core_(std::move(x.core_)) {}
Executor &Executor::operator=(Executor &&x) noexcept {
    if (this != &x) {
        finish();
        core_ = std::move(x.core_);
    }
    return *this;
}
Executor::~Executor() { finish(); }
void Executor::finish() {
    if (!core_)
        return;
    auto c = std::move(core_);
    {
        std::lock_guard<std::mutex> l(c->mutex);
        invariant(c->executors == 1);
        --c->executors;
    }
    c->drain();
}
Work::Work(Work &&x) noexcept
    : core_(std::move(x.core_)), id_(x.id_), payload_(std::move(x.payload_)) {}
Work &Work::operator=(Work &&x) noexcept {
    if (this != &x) {
        finish();
        core_ = std::move(x.core_);
        id_ = x.id_;
        payload_ = std::move(x.payload_);
    }
    return *this;
}
Work::~Work() { finish(); }
bool Work::cancelled() const {
    if (!core_)
        return true;
    std::lock_guard<std::mutex> l(core_->mutex);
    return core_->op(id_)->winner == Winner::Cancelled;
}
Status Work::succeed() {
    if (!core_)
        return Status::Closed;
    Status result = Status::AlreadyConsumed;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        auto *o = core_->op(id_);
        if (o->winner == Winner::Pending) {
            o->winner = Winner::Success;
            o->stored = std::move(payload_);
            result = Status::Ok;
        } else if (o->winner == Winner::Cancelled)
            result = Status::Cancelled;
    }
    if (result != Status::Ok)
        payload_.reset();
    core_->drain();
    return result;
}
Status Work::fail() {
    if (!core_)
        return Status::Closed;
    Status result = Status::AlreadyConsumed;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        auto *o = core_->op(id_);
        if (o->winner == Winner::Pending) {
            o->winner = Winner::DomainFailure;
            result = Status::Ok;
        } else if (o->winner == Winner::Cancelled)
            result = Status::Cancelled;
    }
    payload_.reset();
    core_->drain();
    return result;
}
void Work::finish() {
    if (!core_)
        return;
    auto c = std::move(core_);
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *o = c->op(id_);
        if (o->winner == Winner::Pending)
            o->winner = Winner::InternalFailure;
    }
    payload_.reset();
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *o = c->op(id_);
        invariant(o->phase == Phase::Running);
        o->phase = Phase::Exited;
        invariant(c->work);
        --c->work;
    }
    c->drain();
}
struct Publication::Data {
    std::shared_ptr<Core> core;
    std::array<Payload, N> payloads;
    size_t count = 0;
    int64_t value = 0;
    size_t size = 0;
    uint64_t order = 0;
    bool allocated = false;
};
Publication::Publication() = default;
Publication::Publication(Publication &&x) noexcept
    : data_(std::move(x.data_)), noOp_(std::exchange(x.noOp_, false)) {}
Publication &Publication::operator=(Publication &&x) noexcept {
    if (this != &x) {
        abandon();
        data_ = std::move(x.data_);
        noOp_ = std::exchange(x.noOp_, false);
    }
    return *this;
}
Publication::~Publication() { abandon(); }
Status Publication::allocate(Snapshot s) {
    if (noOp_) {
        if (valid_snapshot(s))
            return Status::Ok;
        noOp_ = false;
        return Status::InvalidArgument;
    }
    if (!data_)
        return Status::Closed;
    if (data_->allocated || !valid_snapshot(s) || s.value != data_->value ||
        s.size != data_->size) {
        abandon();
        return Status::InvalidArgument;
    }
    auto c = data_->core;
    for (size_t i = 0; i < data_->count; ++i) {
        auto &p = data_->payloads[i];
        auto status = c->cloneInto(p, s);
        if (status != Status::Ok) {
            abandon();
            return status;
        }
    }
    data_->allocated = true;
    return Status::Ok;
}
Status Publication::commit() {
    if (noOp_) {
        noOp_ = false;
        return Status::Ok;
    }
    if (!data_)
        return Status::Closed;
    if (!data_->allocated)
        return Status::NotReady;
    auto c = data_->core;
    std::array<Payload, N> replaced;
    Status result = Status::Ok;
    {
        std::lock_guard<std::mutex> l(c->mutex);
        if (c->closing)
            result = Status::Closing;
        for (size_t i = 0; i < data_->count && result == Status::Ok; ++i) {
            auto *s = c->sub(data_->payloads[i].owner_);
            if (!s || !s->open)
                result = Status::Closing;
        }
        if (result == Status::Ok) {
            for (size_t i = 0; i < data_->count; ++i) {
                auto &p = data_->payloads[i];
                auto *s = c->sub(p.owner_);
                invariant(p.source_);
                c->unsource(p);
                p.source_ = false;
                if (s->stream == Stream::State && s->count) {
                    replaced[i] = std::move(s->queue[0]);
                    s->count = 0;
                }
                invariant(s->count < N);
                size_t at = s->count;
                while (at && s->queue[at - 1].order_ > p.order_) {
                    s->queue[at] = std::move(s->queue[at - 1]);
                    --at;
                }
                s->queue[at] = std::move(p);
                ++s->count;
            }
        }
    }
    for (auto &p : replaced)
        p.reset();
    abandon();
    return result;
}
void Publication::abandon() {
    noOp_ = false;
    if (!data_)
        return;
    auto data = std::move(data_);
    auto c = data->core;
    for (auto &p : data->payloads)
        p.reset();
    {
        std::lock_guard<std::mutex> l(c->mutex);
        invariant(c->admissions);
        --c->admissions;
    }
    c->drain();
}
Delivery::Delivery(Delivery &&x) noexcept
    : payload_(std::move(x.payload_)), stateCore_(std::move(x.stateCore_)), stateId_(x.stateId_) {}
Delivery &Delivery::operator=(Delivery &&x) noexcept {
    if (this != &x) {
        discard();
        payload_ = std::move(x.payload_);
        stateCore_ = std::move(x.stateCore_);
        stateId_ = x.stateId_;
    }
    return *this;
}
Delivery::~Delivery() { discard(); }
Status Delivery::invoke() {
    if (!stateCore_ && !payload_.charged_)
        return Status::Closed;
    auto c = stateCore_ ? stateCore_ : payload_.core_;
    const auto owner = stateCore_ ? stateId_ : payload_.owner_;
    Callback *callback = nullptr;
    Status result = Status::Ok;
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *s = c->sub(owner);
        invariant(s && s->inFlight);
        if (!s->open)
            result = Status::Closing;
        else {
            if (stateCore_) {
                // State stays replaceable until this exact invocation-entry point.
                invariant(s->count == 1);
                payload_ = std::move(s->queue[0]);
                s->count = 0;
                payload_.inFlight_ = true;
                stateCore_.reset();
            }
            invariant(!payload_.entered_);
            payload_.entered_ = true;
            ++s->entered;
            ++c->entered;
            callback = &s->callback;
        }
    }
    if (callback) {
        try {
            if (callback->call)
                callback->call(callback->context, Status::Ok, payload_.object_);
        } catch (...) {
            result = Status::CallbackFailure;
            {
                std::lock_guard<std::mutex> l(c->mutex);
                c->sub(owner)->error = result;
            }
            c->stop(owner);
        }
    }
    discard();
    return result;
}
void Delivery::discard() {
    if (stateCore_) {
        auto c = std::move(stateCore_);
        {
            std::lock_guard<std::mutex> l(c->mutex);
            auto *s = c->sub(stateId_);
            invariant(s && s->inFlight);
            --s->inFlight;
        }
        c->drain();
    }
    payload_.reset();
}
ResultClaim::ResultClaim(ResultClaim &&x) noexcept : core_(std::move(x.core_)), id_(x.id_) {}
ResultClaim &ResultClaim::operator=(ResultClaim &&x) noexcept {
    if (this != &x) {
        rollback();
        core_ = std::move(x.core_);
        id_ = x.id_;
    }
    return *this;
}
ResultClaim::~ResultClaim() { rollback(); }
Status ResultClaim::commit(Callback callback) {
    if (!core_)
        return Status::Closed;
    auto c = core_;
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *o = c->op(id_);
        invariant(o && o->claim == Claim::Reserved);
        o->callback = std::move(callback);
        o->claim = Claim::Committed;
    }
    core_.reset();
    return Status::Ok;
}
void ResultClaim::rollback() {
    if (!core_)
        return;
    auto c = std::move(core_);
    Payload garbage;
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *o = c->op(id_);
        invariant(o && o->claim == Claim::Reserved);
        o->claim = Claim::None;
        invariant(c->waiters);
        --c->waiters;
        if (o->closing)
            garbage = std::move(o->stored);
    }
    garbage.reset();
    c->drain();
}
ResultDelivery::ResultDelivery(ResultDelivery &&x) noexcept
    : core_(std::move(x.core_)), id_(x.id_), outcome_(x.outcome_), payload_(std::move(x.payload_)),
      callback_(std::move(x.callback_)) {}
ResultDelivery &ResultDelivery::operator=(ResultDelivery &&x) noexcept {
    if (this != &x) {
        discard();
        core_ = std::move(x.core_);
        id_ = x.id_;
        outcome_ = x.outcome_;
        payload_ = std::move(x.payload_);
        callback_ = std::move(x.callback_);
    }
    return *this;
}
ResultDelivery::~ResultDelivery() { discard(); }
Status ResultDelivery::invoke() {
    if (!core_)
        return Status::Closed;
    Status result = outcome_;
    try {
        if (callback_.call)
            callback_.call(callback_.context, outcome_, payload_.object());
    } catch (...) {
        result = Status::CallbackFailure;
    }
    discard();
    return result;
}
void ResultDelivery::discard() {
    if (!core_)
        return;
    auto c = std::move(core_);
    payload_.reset();
    callback_.reset();
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *o = c->op(id_);
        invariant(o && o->claim == Claim::Taken);
        o->claim = Claim::Finished;
        invariant(c->waiters);
        --c->waiters;
    }
    c->drain();
}
CloseWaiter::CloseWaiter(CloseWaiter &&x) noexcept
    : core_(std::move(x.core_)), id_(x.id_), kind_(x.kind_),
      immediate_(std::exchange(x.immediate_, false)), immediateStatus_(x.immediateStatus_),
      callback_(std::move(x.callback_)) {}
CloseWaiter &CloseWaiter::operator=(CloseWaiter &&x) noexcept {
    if (this != &x) {
        discard();
        core_ = std::move(x.core_);
        id_ = x.id_;
        kind_ = x.kind_;
        immediate_ = std::exchange(x.immediate_, false);
        immediateStatus_ = x.immediateStatus_;
        callback_ = std::move(x.callback_);
    }
    return *this;
}
CloseWaiter::~CloseWaiter() { discard(); }
bool CloseWaiter::ready() const {
    if (immediate_)
        return true;
    if (!core_)
        return false;
    std::lock_guard<std::mutex> l(core_->mutex);
    auto *record = core_->closeRecord(id_);
    return record && record->ready;
}
Status CloseWaiter::invoke() {
    if (immediate_) {
        immediate_ = false;
        return immediateStatus_;
    }
    if (!core_)
        return Status::Closed;
    Status result = Status::Ok;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        auto *record = core_->closeRecord(id_);
        invariant(record);
        if (!record->ready)
            return Status::NotReady;
        result = record->outcome;
    }
    try {
        if (callback_.call)
            callback_.call(callback_.context, result, nullptr);
    } catch (...) {
        result = Status::CallbackFailure;
    }
    discard();
    return result;
}
void CloseWaiter::discard() {
    immediate_ = false;
    if (!core_)
        return;
    auto c = std::move(core_);
    callback_.reset();
    {
        std::lock_guard<std::mutex> l(c->mutex);
        auto *record = c->closeRecord(id_);
        invariant(record && c->waiters);
        record->active = false;
        --c->waiters;
    }
    c->drain();
}
Engine::Engine(Provider provider) : core_(std::make_shared<Core>(provider)) {
    invariant(provider.clone && provider.destroy);
}
Engine::~Engine() { request_close(); }
Status Engine::acquire_executor(Executor &out) {
    if (out.core_)
        return Status::InvalidArgument;
    std::lock_guard<std::mutex> l(core_->mutex);
    if (core_->closing)
        return Status::Closing;
    if (core_->executorTaken)
        return Status::AlreadyConsumed;
    core_->executorTaken = true;
    core_->executors = 1;
    out.core_ = core_;
    return Status::Ok;
}
Status Engine::start(Snapshot s, uint64_t &id) {
    if (!valid_snapshot(s))
        return Status::InvalidArgument;
    Payload input;
    uint64_t allocated = 0;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        if (core_->closing)
            return Status::Closing;
        if (!core_->executors)
            return Status::Closed;
        if (core_->next == std::numeric_limits<uint64_t>::max())
            return Status::ResourceExhausted;
        if (core_->bytes + s.size > B)
            return Status::ResourceExhausted;
        Core::Op *slot = nullptr;
        for (auto &o : core_->ops)
            if (!o.active) {
                slot = &o;
                break;
            }
        if (!slot)
            return Status::ResourceExhausted;
        invariant(!slot->input.object() && !slot->stored.object() && !slot->callback.destroy);
        slot->id = core_->next++;
        slot->active = true;
        slot->closing = false;
        slot->initializing = true;
        slot->phase = Phase::Queued;
        slot->winner = Winner::Pending;
        slot->claim = Claim::None;
        slot->payloads = 0;
        allocated = slot->id;
        ++core_->work;
        ++core_->admissions;
        core_->reserve(input, s.size, allocated, false, false, false);
    }
    Status result = core_->cloneInto(input, s);
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        auto *o = core_->op(allocated);
        o->initializing = false;
        invariant(core_->admissions);
        --core_->admissions;
        if (result == Status::Ok && (core_->closing || o->closing))
            result = Status::Closing;
        if (result == Status::Ok) {
            if (o->winner == Winner::Pending)
                o->input = std::move(input);
            id = allocated;
        } else {
            o->closing = true;
            if (o->phase == Phase::Queued) {
                o->phase = Phase::Exited;
                --core_->work;
            }
        }
    }
    input.reset();
    core_->drain();
    return result;
}
Status Engine::take_work(Work &out) {
    if (out.core_)
        return Status::InvalidArgument;
    std::lock_guard<std::mutex> l(core_->mutex);
    if (core_->closing)
        return Status::Closing;
    if (!core_->executors)
        return Status::Closed;
    for (auto &o : core_->ops)
        if (o.active && o.phase == Phase::Running)
            return Status::NotReady;
    Core::Op *next = nullptr;
    for (auto &o : core_->ops)
        if (o.active && o.phase == Phase::Queued && !o.initializing && (!next || o.id < next->id))
            next = &o;
    if (!next)
        return Status::NotReady;
    next->phase = Phase::Running;
    out.core_ = core_;
    out.id_ = next->id;
    out.payload_ = std::move(next->input);
    return Status::Ok;
}
Status Engine::cancel(uint64_t id) {
    Payload garbage;
    Status result = Status::Ok;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        auto *o = core_->op(id);
        if (!o || !o->active)
            return Status::Closed;
        if (o->winner == Winner::Pending)
            o->winner = Winner::Cancelled;
        if (o->winner == Winner::Cancelled && o->phase == Phase::Queued) {
            o->phase = Phase::Exited;
            invariant(core_->work);
            --core_->work;
            garbage = std::move(o->input);
        }
    }
    garbage.reset();
    core_->drain();
    return result;
}
Winner Engine::winner(uint64_t id) const {
    std::lock_guard<std::mutex> l(core_->mutex);
    auto *o = core_->op(id);
    return o ? o->winner : Winner::InternalFailure;
}
Status Engine::claim_result(uint64_t id, ResultClaim &out) {
    if (out.core_)
        return Status::InvalidArgument;
    std::lock_guard<std::mutex> l(core_->mutex);
    auto *o = core_->op(id);
    if (!o || !o->active)
        return Status::Closed;
    if (o->closing)
        return Status::Closing;
    if (o->claim != Claim::None)
        return Status::AlreadyConsumed;
    if (core_->waiters == W)
        return Status::ResourceExhausted;
    ++core_->waiters;
    o->claim = Claim::Reserved;
    out.core_ = core_;
    out.id_ = id;
    return Status::Ok;
}
Status Engine::take_result(uint64_t id, ResultDelivery &out) {
    if (out.core_)
        return Status::InvalidArgument;
    std::lock_guard<std::mutex> l(core_->mutex);
    auto *o = core_->op(id);
    if (!o || !o->active)
        return Status::Closed;
    if (o->claim != Claim::Committed || o->winner == Winner::Pending)
        return Status::NotReady;
    o->claim = Claim::Taken;
    out.core_ = core_;
    out.id_ = id;
    out.outcome_ = outcome(o->winner);
    out.payload_ = std::move(o->stored);
    out.callback_ = std::move(o->callback);
    return Status::Ok;
}
Status Engine::observe(Stream stream, Callback callback, uint64_t &id) {
    Status result = Status::ResourceExhausted;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        if (core_->closing)
            return Status::Closing;
        if (core_->next == std::numeric_limits<uint64_t>::max())
            return Status::ResourceExhausted;
        for (auto &s : core_->subs)
            if (!s.active) {
                invariant(!s.count && !s.outstanding && !s.callback.destroy);
                s.id = core_->next++;
                s.active = s.open = true;
                s.releasing = false;
                s.stream = stream;
                s.error = Status::Ok;
                s.callback = std::move(callback);
                id = s.id;
                result = Status::Ok;
                break;
            }
    }
    return result;
}
Status Engine::reserve_publish(Stream stream, Snapshot s, Publication &out) {
    if (out.data_ || out.noOp_ || !valid_snapshot(s))
        return Status::InvalidArgument;
    std::unique_ptr<Publication::Data> data;
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        if (core_->closing)
            return Status::Closing;
        size_t count = 0;
        for (auto &sub : core_->subs)
            if (sub.active && sub.open && sub.stream == stream)
                ++count;
        if (!count) {
            out.noOp_ = true;
            return Status::Ok;
        }
        if (count * s.size > B - core_->bytes ||
            (stream == Stream::Event && count > N - core_->eventTickets))
            return Status::ResourceExhausted;
        // Only one staging transaction per selected state subscriber at a time.
        if (stream == Stream::State)
            for (auto &sub : core_->subs)
                if (sub.active && sub.open && sub.stream == stream &&
                    sub.outstanding > sub.count + sub.entered)
                    return Status::ResourceExhausted;
        if (core_->nextOrder == std::numeric_limits<uint64_t>::max())
            return Status::ResourceExhausted;
        data.reset(new (std::nothrow) Publication::Data);
        if (!data)
            return Status::ResourceExhausted;
        data->core = core_;
        data->value = s.value;
        data->size = s.size;
        data->order = core_->nextOrder++;
        for (auto &sub : core_->subs)
            if (sub.active && sub.open && sub.stream == stream)
                core_->reserve(data->payloads[data->count++], s.size, sub.id, true,
                               stream == Stream::Event, true, data->order);
        ++core_->admissions;
    }
    out.data_ = std::move(data);
    return Status::Ok;
}
Status Engine::publish(Stream stream, Snapshot s) {
    Publication p;
    auto status = reserve_publish(stream, s, p);
    if (status != Status::Ok)
        return status;
    status = p.allocate(s);
    return status == Status::Ok ? p.commit() : status;
}
Status Engine::take_delivery(uint64_t id, Delivery &out) {
    if (out.payload_.charged_ || out.stateCore_)
        return Status::InvalidArgument;
    std::lock_guard<std::mutex> l(core_->mutex);
    auto *s = core_->sub(id);
    if (!s || !s->active)
        return Status::Closed;
    if (!s->open)
        return Status::Closing;
    if (!s->count || s->inFlight)
        return Status::NotReady;
    for (auto order : s->sourceOrders)
        if (order && order < s->queue[0].order_)
            return Status::NotReady;
    if (s->stream == Stream::State) {
        // This lease protects callback lifetime without freezing a second pending value.
        ++s->inFlight;
        out.stateCore_ = core_;
        out.stateId_ = id;
        return Status::Ok;
    }
    out.payload_ = std::move(s->queue[0]);
    for (size_t i = 1; i < s->count; ++i)
        s->queue[i - 1] = std::move(s->queue[i]);
    --s->count;
    ++s->inFlight;
    out.payload_.inFlight_ = true;
    return Status::Ok;
}
void Engine::stop(uint64_t id) { core_->stop(id); }
void Engine::close_operation(uint64_t id) { core_->closeOp(id); }
void Engine::request_close() {
    std::array<uint64_t, N> ops{}, subs{};
    {
        std::lock_guard<std::mutex> l(core_->mutex);
        core_->closing = true;
        for (size_t i = 0; i < N; ++i) {
            ops[i] = core_->ops[i].id;
            subs[i] = core_->subs[i].id;
        }
    }
    for (auto id : ops)
        core_->closeOp(id);
    for (auto id : subs)
        core_->stop(id);
    core_->drain();
}
bool Engine::operation_closed(uint64_t id) const { return core_->ready(1, id); }
bool Engine::subscription_closed(uint64_t id) const { return core_->ready(2, id); }
Status Engine::watch_operation_close(uint64_t id, Callback callback, CloseWaiter &out) {
    if (out.core_ || out.immediate_)
        return Status::InvalidArgument;
    close_operation(id);
    bool immediate = false;
    uint64_t recordId = 0;
    auto status = core_->registerClose(1, id, recordId, immediate);
    if (immediate) {
        out.immediate_ = true;
        out.immediateStatus_ = status;
        try {
            if (callback.call)
                callback.call(callback.context, status, nullptr);
        } catch (...) {
            out.immediateStatus_ = Status::CallbackFailure;
        }
        callback.reset();
        return Status::Ok;
    }
    if (status != Status::Ok)
        return status;
    out.core_ = core_;
    out.id_ = recordId;
    out.kind_ = 1;
    out.callback_ = std::move(callback);
    return Status::Ok;
}
Status Engine::watch_subscription_close(uint64_t id, Callback callback, CloseWaiter &out) {
    if (out.core_ || out.immediate_)
        return Status::InvalidArgument;
    stop(id);
    bool immediate = false;
    uint64_t recordId = 0;
    auto status = core_->registerClose(2, id, recordId, immediate);
    if (immediate) {
        out.immediate_ = true;
        out.immediateStatus_ = status;
        try {
            if (callback.call)
                callback.call(callback.context, status, nullptr);
        } catch (...) {
            out.immediateStatus_ = Status::CallbackFailure;
        }
        callback.reset();
        return Status::Ok;
    }
    if (status != Status::Ok)
        return status;
    out.core_ = core_;
    out.id_ = recordId;
    out.kind_ = 2;
    out.callback_ = std::move(callback);
    return Status::Ok;
}
Status Engine::watch_close(Callback callback, CloseWaiter &out) {
    if (out.core_ || out.immediate_)
        return Status::InvalidArgument;
    request_close();
    bool immediate = false;
    uint64_t recordId = 0;
    auto status = core_->registerClose(0, 0, recordId, immediate);
    if (immediate) {
        out.immediate_ = true;
        out.immediateStatus_ = status;
        try {
            if (callback.call)
                callback.call(callback.context, status, nullptr);
        } catch (...) {
            out.immediateStatus_ = Status::CallbackFailure;
        }
        callback.reset();
        return Status::Ok;
    }
    if (status != Status::Ok)
        return status;
    out.core_ = core_;
    out.id_ = recordId;
    out.kind_ = 0;
    out.callback_ = std::move(callback);
    return Status::Ok;
}
Stats Engine::stats() const {
    std::lock_guard<std::mutex> l(core_->mutex);
    size_t ops = 0, subs = 0;
    for (auto &o : core_->ops)
        ops += o.active;
    for (auto &s : core_->subs)
        subs += s.active;
    return {ops,
            subs,
            core_->eventTickets,
            core_->stateTickets,
            core_->sources,
            core_->entered,
            core_->bytes,
            core_->waiters,
            core_->admissions,
            core_->work,
            core_->executors,
            core_->closing,
            core_->closed};
}
} // namespace madb::c07
