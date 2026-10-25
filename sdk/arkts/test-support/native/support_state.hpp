#pragma once
#include "protocol.hpp"
#include <array>
#include <condition_variable>
#include <cstdint>
#include <mutex>
namespace gateway {
enum class BootstrapEvent : size_t {
    OpenAttempt,
    IdentityMatched,
    AbiRejected,
    ProfileRejected,
    ArtifactRejected,
    Reservation,
    ContextCreated,
    EngineCreated,
    WorkerStarted,
    Count
};
void testBootstrapEvent(BootstrapEvent) noexcept;
std::array<uint64_t, static_cast<size_t>(BootstrapEvent::Count)> testBootstrapSnapshot() noexcept;
struct TestControl {
    std::mutex mutex;
    std::condition_variable changed;
    int gatePhase = 0;
    bool gateArmed = false, gateArrived = false, gateReleased = false, gateResumed = false;
    uint64_t gateOperation = 0, gateSequence = 0;
    int completionFault = 0, cloneNth = 0, cloneMode = 0;
    madb::c07::Publication publication;
    bool sourceHeld = false;
    // Owner-env-only test seam. This is the original Core lease, never a copy.
    madb::c07::Delivery delivery;
    uint64_t deliveryTarget = 0, deliveryGeneration = 0;
    bool deliveryArmed = false, deliveryHeld = false;
    bool deliveryEntered = false, deliveryReleased = false;
    uint64_t deliveryTakes = 0, deliveryInvokes = 0, deliveryDiscards = 0;
};
} // namespace gateway
