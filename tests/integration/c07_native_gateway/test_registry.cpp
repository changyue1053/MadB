// Host-only Node-API return harness for the actual runtime registry. This does
// not execute a JS engine or establish target lifetime/Node-API behavior.
#define MADB_NAPI_OBSERVER_NO_WRAPPERS
#include "runtime.cpp"
#include <cstring>
#include <iostream>
#include <vector>
struct napi_value__ {
    double number = 0;
    std::string text;
    std::vector<double> frame;
};
static std::vector<std::unique_ptr<napi_value__>> allocations;
static std::string lastError;
static napi_value value() {
    allocations.emplace_back(std::make_unique<napi_value__>());
    return allocations.back().get();
}
extern "C" {
napi_status napi_throw_error(napi_env, const char *code, const char *) {
    lastError = code;
    return napi_ok;
}
napi_status napi_create_array_with_length(napi_env, size_t n, napi_value *out) {
    *out = value();
    (*out)->frame.resize(n);
    return napi_ok;
}
napi_status napi_create_double(napi_env, double d, napi_value *out) {
    *out = value();
    (*out)->number = d;
    return napi_ok;
}
napi_status napi_define_properties(napi_env, napi_value object, size_t n,
                                   const napi_property_descriptor *fields) {
    for (size_t i = 0; i < n; ++i)
        object->frame.at(std::stoul(fields[i].utf8name)) = fields[i].value->number;
    return napi_ok;
}
napi_status napi_get_value_string_utf8(napi_env, napi_value v, char *out, size_t size,
                                       size_t *length) {
    *length = v->text.size();
    if (out) {
        if (size <= *length)
            return napi_generic_failure;
        std::memcpy(out, v->text.data(), *length);
        out[*length] = 0;
    }
    return napi_ok;
}
napi_status napi_typeof(napi_env, napi_value, napi_valuetype *out) {
    *out = napi_function;
    return napi_ok;
}
napi_status napi_create_string_utf8(napi_env, const char *, size_t, napi_value *) {
    return napi_generic_failure;
}
}
#define require(v)                                                                                 \
    do {                                                                                           \
        if (!(v)) {                                                                                \
            std::cerr << "failure line " << __LINE__ << ": " #v << "\n";                           \
            std::abort();                                                                          \
        }                                                                                          \
    } while (0)
int main() {
    using namespace gateway;
    auto env = reinterpret_cast<napi_env>(uintptr_t{1});
    auto other = reinterpret_cast<napi_env>(uintptr_t{2});
    napi_value expected = value();
    expected->text = identity();
    napi_value dispatcher = value();
    napi_observer::Scope authority(napi_observer::Mode::Entry, env);
    // Shared bootstrap executes the real identity branches before any reservation.
    const auto start = testBootstrapSnapshot();
    for (size_t index = 0; index < 3; ++index) {
        auto bad = identity();
        bad[13 + index * 65] = bad[13 + index * 65] == 'a' ? 'b' : 'a';
        expected->text = bad;
        require(openRuntime(env, expected, dispatcher) == nullptr);
        require(lastError == (index == 0 ? "AbiMismatch" : index == 1 ? "ProfileMismatch" : "ArtifactMismatch"));
    }
    const auto rejected = testBootstrapSnapshot();
    require(rejected[static_cast<size_t>(BootstrapEvent::OpenAttempt)] == start[0] + 3);
    for (size_t index = 2; index <= 4; ++index)
        require(rejected[index] == start[index] + 1);
    for (size_t index = 5; index < rejected.size(); ++index)
        require(rejected[index] == start[index]);
    expected->text = identity();
    // Actual openRuntime reserves epoch1, its returned handle string fails before
    // Owner adoption, and Reservation marks it retired without physical teardown.
    require(openRuntime(env, expected, dispatcher) == nullptr);
    require(lastError == "InternalFailure");
    const auto first = nextEpoch;
    require(slots[0].retired && !physical(slots[0].audit));
    auto direct = testObserve(env, {first, 0, 0, 'r'}, false);
    require(direct && direct->frame.size() == 178);
    require(direct->frame[F::Phase] == 0 && direct->frame[54 + T::ReadMode] == 0);
    // A second real reservation archives the failed first epoch and also fails.
    require(openRuntime(env, expected, dispatcher) == nullptr);
    require(nextEpoch == first + 1);
    require(history[0].epoch == first && !history[0].physicalAck);
    auto old = testObserve(env, {first, 0, 0, 'r'}, false);
    require(old && old->frame[F::Phase] == 0 && old->frame[54 + T::ReadMode] == 0);
    auto foreign = testObserve(other, {first, 0, 0, 'r'}, true);
    require(foreign && foreign->frame[F::Phase] == 0 && foreign->frame[54 + T::ReadMode] == 0);
    require(testObserve(other, {first, 0, 0, 'r'}, false) == nullptr && lastError == "WrongEnv");
    require(testObserve(env, {999, 0, 0, 'r'}, true) == nullptr &&
            lastError == "ObservationExpired");
    require(testObserve(env, {first, 1, 1, 'o'}, true) == nullptr && lastError == "InvalidHandle");
    // Positive serialization control: only a stored true predicate permits ACK.
    history[0].physicalAck = true;
    history[0].values[F::MechanismTeardownStarted] = 1;
    auto ack = testObserve(other, {first, 0, 0, 'r'}, true);
    require(ack && ack->frame[F::Phase] == 3 && ack->frame[54 + T::ReadMode] == 2);
    std::cout << "11 registry checks passed; host NAPI return harness, no target claim\n";
}
