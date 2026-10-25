#pragma once
#include "runtime.hpp"
#include <unistd.h>
namespace gateway {
using T = TestFrame;
void testMark(Audit &, size_t field, size_t sequence = 48) noexcept;
void testSet(Audit &, size_t field, int64_t value) noexcept;
void testGate(Runtime &, int phase) noexcept;
void testSubmitted(Runtime &, uint64_t id) noexcept;
void testUnwind(Runtime &) noexcept;
int testBeforeClone(Runtime &) noexcept;
Provider testProvider(Runtime &) noexcept;
mp_status testRoundtripClone(Runtime &, mp_snapshot, void **) noexcept;
bool testFault(Runtime &, int stage) noexcept;
void testWorkerStarted(void *) noexcept;
void testWorkerLoopExited(void *) noexcept;
void testWorkerAfterTerminal(void *) noexcept;
void testJoinStarted(void *) noexcept;
void testJoinReturned(void *) noexcept;
void testExecutorReleased(void *) noexcept;
} // namespace gateway
namespace gateway {
template <size_t N> napi_value testNumbers(napi_env env, const std::array<int64_t, N> &values) {
    napi_value result = nullptr;
    if (napi_create_array_with_length(env, N, &result) != napi_ok)
        return error(env, "InternalFailure");
    for (size_t i = 0; i < N; ++i) {
        if (values[i] > 9007199254740991LL || values[i] < -9007199254740991LL)
            return error(env, "DiagnosticRangeExceeded");
        napi_value value = nullptr;
        auto key = std::to_string(i);
        if (napi_create_double(env, static_cast<double>(values[i]), &value) != napi_ok)
            return error(env, "InternalFailure");
        napi_property_descriptor field{key.c_str(), nullptr, nullptr,         nullptr,
                                       nullptr,     value,   napi_enumerable, nullptr};
        if (napi_define_properties(env, result, 1, &field) != napi_ok)
            return error(env, "InternalFailure");
    }
    return result;
}
} // namespace gateway
