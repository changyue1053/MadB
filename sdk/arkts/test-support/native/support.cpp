#include "support.hpp"
#include <unistd.h>
namespace gateway {
void testMark(Audit &a, size_t field, size_t sequence) noexcept {
    std::lock_guard<std::mutex> l(a.testMutex);
    ++a.test[field];
    const auto n = ++a.test[T::LastEventSequence];
    if (sequence < 48)
        a.test[sequence] = n;
}
void testSet(Audit &a, size_t field, int64_t value) noexcept {
    std::lock_guard<std::mutex> l(a.testMutex);
    a.test[field] = value;
}
void testGate(Runtime &r, int phase) noexcept {
    std::unique_lock<std::mutex> l(r.test.mutex);
    if (!r.test.gateArmed || r.test.gatePhase != phase)
        return;
    r.test.gateArrived = true;
    testSet(r.audit, T::GateArrived, 1);
    testSet(r.audit, T::WorkerTid, gettid());
    r.test.changed.wait(l, [&] { return r.test.gateReleased || r.envClosing; });
    r.test.gateResumed = true;
    r.test.gateArmed = false;
    testSet(r.audit, T::GateResumed, 1);
    testSet(r.audit, T::GateArmed, 0);
}
void testSubmitted(Runtime &r, uint64_t id) noexcept {
    std::lock_guard<std::mutex> l(r.test.mutex);
    if (r.test.gateArmed && !r.test.gateOperation)
        r.test.gateOperation = id;
}
void testUnwind(Runtime &r) noexcept {
    {
        std::lock_guard<std::mutex> l(r.test.mutex);
        r.test.gateReleased = true;
        testSet(r.audit, T::GateReleased, 1);
    }
    r.test.changed.notify_all();
    if (r.test.sourceHeld) {
        r.test.publication.abandon();
        r.test.sourceHeld = false;
        testSet(r.audit, T::SourceHeld, 0);
        testMark(r.audit, T::SourceAbandoned);
    }
}
// Called only while TestControl.mutex serializes the test injection and the
// real provider call. It cannot be consumed by a concurrent increment.
int testBeforeClone(Runtime &r) noexcept {
    if (r.test.cloneNth <= 0)
        return 0;
    testMark(r.audit, T::CloneCallsSinceArm);
    if (--r.test.cloneNth != 0)
        return 0;
    testMark(r.audit, T::CloneFaultHits);
    if (r.test.cloneMode != 4)
        r.provider.inject(r.provider.context, r.test.cloneMode);
    return r.test.cloneMode;
}
Provider testProvider(Runtime &r) noexcept {
    return {&r,
            [](void *p, Snapshot s) noexcept -> CloneResult {
                auto &r = *static_cast<Runtime *>(p);
                std::lock_guard<std::mutex> l(r.test.mutex);
                const int mode = testBeforeClone(r);
                auto port = madb::c07::probe::provider_port(r.provider);
                auto result = port.clone(port.context, s);
                // Test-only post-clone port failure. Core receives the real non-null
                // allocation even on error, and must invoke its paired release itself.
                if (mode == 4 && result.status == CloneStatus::Ok && result.owned)
                    result.status = CloneStatus::InternalFailure;
                return result;
            },
            [](void *p, void *owned) noexcept {
                auto &r = *static_cast<Runtime *>(p);
                auto port = madb::c07::probe::provider_port(r.provider);
                port.destroy(port.context, owned);
            }};
}
mp_status testRoundtripClone(Runtime &r, mp_snapshot s, void **out) noexcept {
    std::lock_guard<std::mutex> l(r.test.mutex);
    const int mode = testBeforeClone(r);
    auto status = r.provider.clone(r.provider.context, s, out);
    return mode == 4 && status == MP_OK && *out ? MP_INTERNAL_FAILURE : status;
}
bool testFault(Runtime &r, int stage) noexcept {
    if (r.test.completionFault != stage)
        return false;
    r.test.completionFault = 0;
    testSet(r.audit, T::CompletionStage, 0);
    testMark(r.audit, T::CompletionFaultHits);
    testMark(r.audit, stage == 1   ? T::ClaimRollbackHits
                      : stage == 2 ? T::BeforeGuardHits
                                   : T::AfterGuardHits);
    return true;
}
namespace {
struct TlsWitness {
    Audit *a = nullptr;
    ~TlsWitness() {
        if (a)
            testMark(*a, T::WorkerTlsExited, T::TlsExitedSequence);
    }
};
thread_local TlsWitness witness;
} // namespace
void testWorkerStarted(void *p) noexcept {
    auto &r = *static_cast<Runtime *>(p);
    witness.a = &r.audit;
    testSet(r.audit, T::WorkerTid, gettid());
    testMark(r.audit, T::WorkerStarted, T::StartedSequence);
}
void testWorkerLoopExited(void *p) noexcept {
    testMark(static_cast<Runtime *>(p)->audit, T::WorkerLoopExited, T::LoopExitedSequence);
}
void testWorkerAfterTerminal(void *p) noexcept {
    testGate(*static_cast<Runtime *>(p), 2);
}
void testJoinStarted(void *p) noexcept {
    auto &r = *static_cast<Runtime *>(p);
    testSet(r.audit, T::CleanupTid, gettid());
    testMark(r.audit, T::JoinStarted, T::JoinStartedSequence);
}
void testJoinReturned(void *p) noexcept {
    testMark(static_cast<Runtime *>(p)->audit, T::JoinReturned, T::JoinReturnedSequence);
}
void testExecutorReleased(void *p) noexcept {
    testMark(static_cast<Runtime *>(p)->audit, T::ExecutorReleased, T::ExecutorReleasedSequence);
}
} // namespace gateway
