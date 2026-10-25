#pragma once
#include "protocol.hpp"
#include <condition_variable>
#include <mutex>
namespace gateway {
struct TestControl {
    std::mutex mutex;
    std::condition_variable changed;
    int gatePhase = 0;
    bool gateArmed = false, gateArrived = false, gateReleased = false, gateResumed = false;
    uint64_t gateOperation = 0, gateSequence = 0;
    int completionFault = 0, cloneNth = 0, cloneMode = 0;
    madb::c07::Publication publication;
    bool sourceHeld = false;
};
} // namespace gateway
