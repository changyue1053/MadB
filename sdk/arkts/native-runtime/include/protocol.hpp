#pragma once
#include <cstddef>
#include <cstdint>
#include <memory>
namespace madb::c07 {
// PRIVATE PROTOTYPE ONLY: no stable/public ABI, Node-API handle, or env thread
// semantics. Engine methods are synchronized; an individual move-only token must
// never be concurrently accessed. The actual env/DSO/provider owner outlives all
// tokens and invokes provider functions only from its declared lawful threads.
// Numeric resource ids never repeat within one Engine (exhaustion is rejected).
// They are not public handles: a facade-owned Closed tombstone must retain its
// immutable outcome; it must not ask a reused native slot for historical errors.

enum class Status {
    Ok,
    InvalidArgument,
    ResourceExhausted,
    Closing,
    Closed,
    Cancelled,
    AlreadyConsumed,
    NotReady,
    DomainFailure,
    CallbackFailure,
    InternalFailure
};
enum class Winner { Pending, Success, DomainFailure, Cancelled, InternalFailure };
enum class Stream { State, Event };
struct Snapshot {
    int64_t value;
    const unsigned char *text;
    size_t size;
};
// clone must copy the supplied Snapshot; destroy pairs with its own allocator.
// Both execute outside protocol locks. Neither may throw across this private port.
struct Provider {
    void *context;
    void *(*clone)(void *, Snapshot) noexcept;
    void (*destroy)(void *, void *) noexcept;
};
struct Callback {
    void *context = nullptr;
    void (*call)(void *, Status, const void *) = nullptr;
    void (*destroy)(void *) noexcept = nullptr;
    Callback() = default;
    Callback(void *p, void (*fn)(void *, Status, const void *), void (*drop)(void *) noexcept)
        : context(p), call(fn), destroy(drop) {}
    Callback(const Callback &) = delete;
    Callback &operator=(const Callback &) = delete;
    Callback(Callback &&) noexcept;
    Callback &operator=(Callback &&) noexcept;
    ~Callback();
    void reset() noexcept;
};
struct Stats {
    size_t operations, subscriptions, eventTickets, stateTickets, sources, entered, bytes, waiters,
        admissions, work, executors;
    bool closing, closed;
};
struct Core;
class Payload {
  public:
    Payload() = default;
    Payload(Payload &&) noexcept;
    Payload &operator=(Payload &&) noexcept;
    Payload(const Payload &) = delete;
    Payload &operator=(const Payload &) = delete;
    ~Payload();
    void reset() noexcept;
    void *object() const noexcept { return object_; }

  private:
    friend struct Core;
    friend class Engine;
    friend class Publication;
    friend class Work;
    friend class Delivery;
    friend class ResultDelivery;
    std::shared_ptr<Core> core_;
    void *object_ = nullptr;
    size_t bytes_ = 0;
    uint64_t owner_ = 0, order_ = 0;
    bool subscription_ = false, event_ = false, source_ = false, inFlight_ = false,
         entered_ = false, charged_ = false;
};
// The external owner drives one real worker. Holding this lease is not proof that
// an OS thread has exited: finish() must occur only after its actual exit/join ACK.
class Executor {
  public:
    Executor() = default;
    Executor(Executor &&) noexcept;
    Executor &operator=(Executor &&) noexcept;
    Executor(const Executor &) = delete;
    ~Executor();
    void finish();

  private:
    friend class Engine;
    std::shared_ptr<Core> core_;
};
// One outstanding Running Work per Engine. input() is exclusively borrowed until
// succeed/fail/finish. succeed moves ownership; it does not end the work lease.
class Work {
  public:
    Work() = default;
    Work(Work &&) noexcept;
    Work &operator=(Work &&) noexcept;
    Work(const Work &) = delete;
    ~Work();
    void *input() const { return payload_.object(); }
    bool cancelled() const;
    Status succeed();
    Status fail();
    void finish();

  private:
    friend class Engine;
    std::shared_ptr<Core> core_;
    uint64_t id_ = 0;
    Payload payload_;
};
// Source admission precedes clone/enqueue. Snapshot bytes remain a synchronous
// borrow during allocate(), never retained as uncharged scratch by this object.
// Explicit abandon rolls back the whole recipient set. No recipient is a value-
// only no-op receipt, with no Core, heap allocation, or admission held.
class Publication {
  public:
    Publication();
    Publication(Publication &&) noexcept;
    Publication &operator=(Publication &&) noexcept;
    Publication(const Publication &) = delete;
    ~Publication();
    Status allocate(Snapshot);
    Status commit();
    void abandon();

  private:
    friend class Engine;
    struct Data;
    std::unique_ptr<Data> data_;
    bool noOp_ = false;
};
class Delivery {
  public:
    Delivery() = default;
    Delivery(Delivery &&) noexcept;
    Delivery &operator=(Delivery &&) noexcept;
    Delivery(const Delivery &) = delete;
    ~Delivery();
    Status invoke();
    void discard();

  private:
    friend class Engine;
    Payload payload_;
    std::shared_ptr<Core> stateCore_;
    uint64_t stateId_ = 0;
};
// Successful reservation owns a claimant plus one waiter, even across close.
// Registration failure must rollback. After commit, the env owner must eventually
// take_result and consume/discard its ResultDelivery, including env-exit discard.
class ResultClaim {
  public:
    ResultClaim() = default;
    ResultClaim(ResultClaim &&) noexcept;
    ResultClaim &operator=(ResultClaim &&) noexcept;
    ResultClaim(const ResultClaim &) = delete;
    ~ResultClaim();
    Status commit(Callback);
    void rollback();

  private:
    friend class Engine;
    std::shared_ptr<Core> core_;
    uint64_t id_ = 0;
};
// invoke lends its payload to the private callback only until return; that port
// must copy/convert before returning. It is not an arbitrary JS callback thread.
// Callback and payload destruction complete before the waiter is returned.
class ResultDelivery {
  public:
    ResultDelivery() = default;
    ResultDelivery(ResultDelivery &&) noexcept;
    ResultDelivery &operator=(ResultDelivery &&) noexcept;
    ResultDelivery(const ResultDelivery &) = delete;
    ~ResultDelivery();
    Status invoke();
    void discard();

  private:
    friend class Engine;
    std::shared_ptr<Core> core_;
    uint64_t id_ = 0;
    Status outcome_ = Status::InternalFailure;
    Payload payload_;
    Callback callback_;
};
// Pending AND ready records occupy the shared 32-slot waiter budget until actual
// invoke/discard completes. ready() does not release ownership. Already-closed
// watch invokes/destroys the private callback synchronously and returns a value-
// only receipt; its later invoke() only consumes that value, without calling twice.
// Neither form chooses JS Promise scheduling or an env executor.
class CloseWaiter {
  public:
    CloseWaiter() = default;
    CloseWaiter(CloseWaiter &&) noexcept;
    CloseWaiter &operator=(CloseWaiter &&) noexcept;
    CloseWaiter(const CloseWaiter &) = delete;
    ~CloseWaiter();
    bool ready() const;
    Status invoke();
    void discard();

  private:
    friend class Engine;
    std::shared_ptr<Core> core_;
    uint64_t id_ = 0;
    int kind_ = 0;
    bool immediate_ = false;
    Status immediateStatus_ = Status::Ok;
    Callback callback_;
};
class Engine {
  public:
    explicit Engine(Provider);
    ~Engine();
    Engine(const Engine &) = delete;
    Status acquire_executor(Executor &);
    Status start(Snapshot, uint64_t &);
    Status take_work(Work &);
    Status cancel(uint64_t);
    Winner winner(uint64_t) const;
    Status claim_result(uint64_t, ResultClaim &);
    Status take_result(uint64_t, ResultDelivery &);
    Status observe(Stream, Callback, uint64_t &);
    Status reserve_publish(Stream, Snapshot, Publication &);
    Status publish(Stream, Snapshot);
    Status take_delivery(uint64_t, Delivery &);
    void stop(uint64_t);
    void close_operation(uint64_t);
    void request_close();
    bool operation_closed(uint64_t) const;
    bool subscription_closed(uint64_t) const;
    Status watch_operation_close(uint64_t, Callback, CloseWaiter &);
    Status watch_subscription_close(uint64_t, Callback, CloseWaiter &);
    Status watch_close(Callback, CloseWaiter &);
    Stats stats() const;

  private:
    std::shared_ptr<Core> core_;
};
bool valid_snapshot(Snapshot) noexcept;
} // namespace madb::c07
