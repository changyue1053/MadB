#pragma once
#include "runtime.hpp"
#include <unistd.h>
namespace gateway {
using T = TestFrame;
inline bool testObservationHandle(napi_env env, napi_value value, Handle &out) {
    std::array<char, 320> bytes{};
    size_t length = 0, copied = 0;
    if (napi_get_value_string_utf8(env, value, nullptr, 0, &length) != napi_ok ||
        length >= bytes.size() ||
        napi_get_value_string_utf8(env, value, bytes.data(), bytes.size(), &copied) != napi_ok ||
        copied != length) {
        error(env, "InvalidArgument");
        return false;
    }
    std::string_view key(bytes.data(), length), prefix = "madb-observe/1/";
    if (key.substr(0, prefix.size()) != prefix ||
        !parseBoundHandle(key.substr(prefix.size()), GatewayRouteIdentity, out) ||
        out.kind != 'r') {
        error(env, "InvalidHandle");
        return false;
    }
    return true;
}
void testMark(Audit &, size_t field, size_t sequence = 48) noexcept;
void testSet(Audit &, size_t field, int64_t value) noexcept;
void testGate(Runtime &, int phase) noexcept;
void testSubmitted(Runtime &, uint64_t id) noexcept;
void testUnwind(Runtime &) noexcept;
Status testDeliver(Runtime &, uint64_t);
std::array<int64_t, 8> testDeliverySnapshot(const Runtime &) noexcept;
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
