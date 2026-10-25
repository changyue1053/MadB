// Generated from actual SDK signatures; do not hand-edit.
#include <napi/native_api.h>
#include <node_api.h>
#include <type_traits>
#include "observer.h"
#define MADB_KN_OBSERVER_ROLE 0
#define MADB_KN_OBSERVER_API_COUNT 12
#define MADB_KN_OBSERVER_COUNTER_INITIALIZERS {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}
#include "observer_core.inc"
using MadbArg_napi_create_array_with_length_0 = napi_env;
using MadbArg_napi_create_array_with_length_1 = size_t;
using MadbArg_napi_create_array_with_length_2 = napi_value *;
extern "C" decltype(napi_create_array_with_length) __real_napi_create_array_with_length;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_array_with_length(MadbArg_napi_create_array_with_length_0 arg0, MadbArg_napi_create_array_with_length_1 arg1, MadbArg_napi_create_array_with_length_2 arg2) {
    recordAttempt(0, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_array_with_length(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_array_with_length), decltype(&napi_create_array_with_length)>::value, "SDK signature mismatch: napi_create_array_with_length");
using MadbArg_napi_create_double_0 = napi_env;
using MadbArg_napi_create_double_1 = double;
using MadbArg_napi_create_double_2 = napi_value *;
extern "C" decltype(napi_create_double) __real_napi_create_double;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_double(MadbArg_napi_create_double_0 arg0, MadbArg_napi_create_double_1 arg1, MadbArg_napi_create_double_2 arg2) {
    recordAttempt(1, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_double(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_double), decltype(&napi_create_double)>::value, "SDK signature mismatch: napi_create_double");
using MadbArg_napi_create_object_0 = napi_env;
using MadbArg_napi_create_object_1 = napi_value *;
extern "C" decltype(napi_create_object) __real_napi_create_object;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_object(MadbArg_napi_create_object_0 arg0, MadbArg_napi_create_object_1 arg1) {
    recordAttempt(2, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_object(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_object), decltype(&napi_create_object)>::value, "SDK signature mismatch: napi_create_object");
using MadbArg_napi_create_string_utf8_0 = napi_env;
using MadbArg_napi_create_string_utf8_1 = const char *;
using MadbArg_napi_create_string_utf8_2 = size_t;
using MadbArg_napi_create_string_utf8_3 = napi_value *;
extern "C" decltype(napi_create_string_utf8) __real_napi_create_string_utf8;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_string_utf8(MadbArg_napi_create_string_utf8_0 arg0, MadbArg_napi_create_string_utf8_1 arg1, MadbArg_napi_create_string_utf8_2 arg2, MadbArg_napi_create_string_utf8_3 arg3) {
    recordAttempt(3, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_string_utf8(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_string_utf8), decltype(&napi_create_string_utf8)>::value, "SDK signature mismatch: napi_create_string_utf8");
using MadbArg_napi_define_properties_0 = napi_env;
using MadbArg_napi_define_properties_1 = napi_value;
using MadbArg_napi_define_properties_2 = size_t;
using MadbArg_napi_define_properties_3 = const napi_property_descriptor *;
extern "C" decltype(napi_define_properties) __real_napi_define_properties;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_define_properties(MadbArg_napi_define_properties_0 arg0, MadbArg_napi_define_properties_1 arg1, MadbArg_napi_define_properties_2 arg2, MadbArg_napi_define_properties_3 arg3) {
    recordAttempt(4, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_define_properties(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_define_properties), decltype(&napi_define_properties)>::value, "SDK signature mismatch: napi_define_properties");
using MadbArg_napi_get_cb_info_0 = napi_env;
using MadbArg_napi_get_cb_info_1 = napi_callback_info;
using MadbArg_napi_get_cb_info_2 = size_t *;
using MadbArg_napi_get_cb_info_3 = napi_value *;
using MadbArg_napi_get_cb_info_4 = napi_value *;
using MadbArg_napi_get_cb_info_5 = void **;
extern "C" decltype(napi_get_cb_info) __real_napi_get_cb_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_cb_info(MadbArg_napi_get_cb_info_0 arg0, MadbArg_napi_get_cb_info_1 arg1, MadbArg_napi_get_cb_info_2 arg2, MadbArg_napi_get_cb_info_3 arg3, MadbArg_napi_get_cb_info_4 arg4, MadbArg_napi_get_cb_info_5 arg5) {
    recordAttempt(5, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_cb_info(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_cb_info), decltype(&napi_get_cb_info)>::value, "SDK signature mismatch: napi_get_cb_info");
using MadbArg_napi_get_value_double_0 = napi_env;
using MadbArg_napi_get_value_double_1 = napi_value;
using MadbArg_napi_get_value_double_2 = double *;
extern "C" decltype(napi_get_value_double) __real_napi_get_value_double;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_double(MadbArg_napi_get_value_double_0 arg0, MadbArg_napi_get_value_double_1 arg1, MadbArg_napi_get_value_double_2 arg2) {
    recordAttempt(6, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_double(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_double), decltype(&napi_get_value_double)>::value, "SDK signature mismatch: napi_get_value_double");
using MadbArg_napi_get_value_string_utf8_0 = napi_env;
using MadbArg_napi_get_value_string_utf8_1 = napi_value;
using MadbArg_napi_get_value_string_utf8_2 = char *;
using MadbArg_napi_get_value_string_utf8_3 = size_t;
using MadbArg_napi_get_value_string_utf8_4 = size_t *;
extern "C" decltype(napi_get_value_string_utf8) __real_napi_get_value_string_utf8;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_string_utf8(MadbArg_napi_get_value_string_utf8_0 arg0, MadbArg_napi_get_value_string_utf8_1 arg1, MadbArg_napi_get_value_string_utf8_2 arg2, MadbArg_napi_get_value_string_utf8_3 arg3, MadbArg_napi_get_value_string_utf8_4 arg4) {
    recordAttempt(7, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_string_utf8(arg0, arg1, arg2, arg3, arg4);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_string_utf8), decltype(&napi_get_value_string_utf8)>::value, "SDK signature mismatch: napi_get_value_string_utf8");
using MadbArg_napi_is_exception_pending_0 = napi_env;
using MadbArg_napi_is_exception_pending_1 = bool *;
extern "C" decltype(napi_is_exception_pending) __real_napi_is_exception_pending;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_exception_pending(MadbArg_napi_is_exception_pending_0 arg0, MadbArg_napi_is_exception_pending_1 arg1) {
    recordAttempt(8, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_exception_pending(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_exception_pending), decltype(&napi_is_exception_pending)>::value, "SDK signature mismatch: napi_is_exception_pending");
using MadbArg_napi_module_register_0 = napi_module *;
extern "C" decltype(napi_module_register) __real_napi_module_register;
extern "C" __attribute__((visibility("hidden"))) void __wrap_napi_module_register(MadbArg_napi_module_register_0 arg0) {
    recordAttempt(9, 2, nullptr);
    __real_napi_module_register(arg0);
}
static_assert(std::is_same<decltype(&__wrap_napi_module_register), decltype(&napi_module_register)>::value, "SDK signature mismatch: napi_module_register");
using MadbArg_napi_throw_error_0 = napi_env;
using MadbArg_napi_throw_error_1 = const char *;
using MadbArg_napi_throw_error_2 = const char *;
extern "C" decltype(napi_throw_error) __real_napi_throw_error;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_throw_error(MadbArg_napi_throw_error_0 arg0, MadbArg_napi_throw_error_1 arg1, MadbArg_napi_throw_error_2 arg2) {
    recordAttempt(10, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_throw_error(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_throw_error), decltype(&napi_throw_error)>::value, "SDK signature mismatch: napi_throw_error");
using MadbArg_napi_typeof_0 = napi_env;
using MadbArg_napi_typeof_1 = napi_value;
using MadbArg_napi_typeof_2 = napi_valuetype *;
extern "C" decltype(napi_typeof) __real_napi_typeof;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_typeof(MadbArg_napi_typeof_0 arg0, MadbArg_napi_typeof_1 arg1, MadbArg_napi_typeof_2 arg2) {
    recordAttempt(11, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_typeof(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_typeof), decltype(&napi_typeof)>::value, "SDK signature mismatch: napi_typeof");
