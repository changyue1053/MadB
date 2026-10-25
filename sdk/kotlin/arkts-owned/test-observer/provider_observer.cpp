// Generated from actual SDK signatures; do not hand-edit.
#include <napi/native_api.h>
#include <node_api.h>
#include <type_traits>
#include "observer.h"
#define MADB_KN_OBSERVER_ROLE 1
#define MADB_KN_OBSERVER_API_COUNT 83
#define MADB_KN_OBSERVER_COUNTER_INITIALIZERS {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}, {0}
#include "observer_core.inc"
using MadbArg_napi_acquire_threadsafe_function_0 = napi_threadsafe_function;
extern "C" decltype(napi_acquire_threadsafe_function) __real_napi_acquire_threadsafe_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_acquire_threadsafe_function(MadbArg_napi_acquire_threadsafe_function_0 arg0) {
    recordAttempt(0, 1, nullptr);
    return __real_napi_acquire_threadsafe_function(arg0);
}
static_assert(std::is_same<decltype(&__wrap_napi_acquire_threadsafe_function), decltype(&napi_acquire_threadsafe_function)>::value, "SDK signature mismatch: napi_acquire_threadsafe_function");
using MadbArg_napi_add_async_cleanup_hook_0 = napi_env;
using MadbArg_napi_add_async_cleanup_hook_1 = napi_async_cleanup_hook;
using MadbArg_napi_add_async_cleanup_hook_2 = void *;
using MadbArg_napi_add_async_cleanup_hook_3 = napi_async_cleanup_hook_handle *;
extern "C" decltype(napi_add_async_cleanup_hook) __real_napi_add_async_cleanup_hook;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_add_async_cleanup_hook(MadbArg_napi_add_async_cleanup_hook_0 arg0, MadbArg_napi_add_async_cleanup_hook_1 arg1, MadbArg_napi_add_async_cleanup_hook_2 arg2, MadbArg_napi_add_async_cleanup_hook_3 arg3) {
    recordAttempt(1, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_add_async_cleanup_hook(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_add_async_cleanup_hook), decltype(&napi_add_async_cleanup_hook)>::value, "SDK signature mismatch: napi_add_async_cleanup_hook");
using MadbArg_napi_add_env_cleanup_hook_0 = napi_env;
using MadbArg_napi_add_env_cleanup_hook_1 = void (*)(void *);
using MadbArg_napi_add_env_cleanup_hook_2 = void *;
extern "C" decltype(napi_add_env_cleanup_hook) __real_napi_add_env_cleanup_hook;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_add_env_cleanup_hook(MadbArg_napi_add_env_cleanup_hook_0 arg0, MadbArg_napi_add_env_cleanup_hook_1 arg1, MadbArg_napi_add_env_cleanup_hook_2 arg2) {
    recordAttempt(2, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_add_env_cleanup_hook(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_add_env_cleanup_hook), decltype(&napi_add_env_cleanup_hook)>::value, "SDK signature mismatch: napi_add_env_cleanup_hook");
using MadbArg_napi_add_finalizer_0 = napi_env;
using MadbArg_napi_add_finalizer_1 = napi_value;
using MadbArg_napi_add_finalizer_2 = void *;
using MadbArg_napi_add_finalizer_3 = napi_finalize;
using MadbArg_napi_add_finalizer_4 = void *;
using MadbArg_napi_add_finalizer_5 = napi_ref *;
extern "C" decltype(napi_add_finalizer) __real_napi_add_finalizer;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_add_finalizer(MadbArg_napi_add_finalizer_0 arg0, MadbArg_napi_add_finalizer_1 arg1, MadbArg_napi_add_finalizer_2 arg2, MadbArg_napi_add_finalizer_3 arg3, MadbArg_napi_add_finalizer_4 arg4, MadbArg_napi_add_finalizer_5 arg5) {
    recordAttempt(3, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_add_finalizer(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_add_finalizer), decltype(&napi_add_finalizer)>::value, "SDK signature mismatch: napi_add_finalizer");
using MadbArg_napi_call_function_0 = napi_env;
using MadbArg_napi_call_function_1 = napi_value;
using MadbArg_napi_call_function_2 = napi_value;
using MadbArg_napi_call_function_3 = size_t;
using MadbArg_napi_call_function_4 = const napi_value *;
using MadbArg_napi_call_function_5 = napi_value *;
extern "C" decltype(napi_call_function) __real_napi_call_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_call_function(MadbArg_napi_call_function_0 arg0, MadbArg_napi_call_function_1 arg1, MadbArg_napi_call_function_2 arg2, MadbArg_napi_call_function_3 arg3, MadbArg_napi_call_function_4 arg4, MadbArg_napi_call_function_5 arg5) {
    recordAttempt(4, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_call_function(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_call_function), decltype(&napi_call_function)>::value, "SDK signature mismatch: napi_call_function");
using MadbArg_napi_call_threadsafe_function_0 = napi_threadsafe_function;
using MadbArg_napi_call_threadsafe_function_1 = void *;
using MadbArg_napi_call_threadsafe_function_2 = napi_threadsafe_function_call_mode;
extern "C" decltype(napi_call_threadsafe_function) __real_napi_call_threadsafe_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_call_threadsafe_function(MadbArg_napi_call_threadsafe_function_0 arg0, MadbArg_napi_call_threadsafe_function_1 arg1, MadbArg_napi_call_threadsafe_function_2 arg2) {
    recordAttempt(5, 1, nullptr);
    return __real_napi_call_threadsafe_function(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_call_threadsafe_function), decltype(&napi_call_threadsafe_function)>::value, "SDK signature mismatch: napi_call_threadsafe_function");
using MadbArg_napi_call_threadsafe_function_with_priority_0 = napi_threadsafe_function;
using MadbArg_napi_call_threadsafe_function_with_priority_1 = void *;
using MadbArg_napi_call_threadsafe_function_with_priority_2 = napi_task_priority;
using MadbArg_napi_call_threadsafe_function_with_priority_3 = bool;
extern "C" decltype(napi_call_threadsafe_function_with_priority) __real_napi_call_threadsafe_function_with_priority;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_call_threadsafe_function_with_priority(MadbArg_napi_call_threadsafe_function_with_priority_0 arg0, MadbArg_napi_call_threadsafe_function_with_priority_1 arg1, MadbArg_napi_call_threadsafe_function_with_priority_2 arg2, MadbArg_napi_call_threadsafe_function_with_priority_3 arg3) {
    recordAttempt(6, 1, nullptr);
    return __real_napi_call_threadsafe_function_with_priority(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_call_threadsafe_function_with_priority), decltype(&napi_call_threadsafe_function_with_priority)>::value, "SDK signature mismatch: napi_call_threadsafe_function_with_priority");
using MadbArg_napi_close_handle_scope_0 = napi_env;
using MadbArg_napi_close_handle_scope_1 = napi_handle_scope;
extern "C" decltype(napi_close_handle_scope) __real_napi_close_handle_scope;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_close_handle_scope(MadbArg_napi_close_handle_scope_0 arg0, MadbArg_napi_close_handle_scope_1 arg1) {
    recordAttempt(7, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_close_handle_scope(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_close_handle_scope), decltype(&napi_close_handle_scope)>::value, "SDK signature mismatch: napi_close_handle_scope");
using MadbArg_napi_create_array_0 = napi_env;
using MadbArg_napi_create_array_1 = napi_value *;
extern "C" decltype(napi_create_array) __real_napi_create_array;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_array(MadbArg_napi_create_array_0 arg0, MadbArg_napi_create_array_1 arg1) {
    recordAttempt(8, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_array(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_array), decltype(&napi_create_array)>::value, "SDK signature mismatch: napi_create_array");
using MadbArg_napi_create_arraybuffer_0 = napi_env;
using MadbArg_napi_create_arraybuffer_1 = size_t;
using MadbArg_napi_create_arraybuffer_2 = void **;
using MadbArg_napi_create_arraybuffer_3 = napi_value *;
extern "C" decltype(napi_create_arraybuffer) __real_napi_create_arraybuffer;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_arraybuffer(MadbArg_napi_create_arraybuffer_0 arg0, MadbArg_napi_create_arraybuffer_1 arg1, MadbArg_napi_create_arraybuffer_2 arg2, MadbArg_napi_create_arraybuffer_3 arg3) {
    recordAttempt(9, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_arraybuffer(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_arraybuffer), decltype(&napi_create_arraybuffer)>::value, "SDK signature mismatch: napi_create_arraybuffer");
using MadbArg_napi_create_bigint_int64_0 = napi_env;
using MadbArg_napi_create_bigint_int64_1 = int64_t;
using MadbArg_napi_create_bigint_int64_2 = napi_value *;
extern "C" decltype(napi_create_bigint_int64) __real_napi_create_bigint_int64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_bigint_int64(MadbArg_napi_create_bigint_int64_0 arg0, MadbArg_napi_create_bigint_int64_1 arg1, MadbArg_napi_create_bigint_int64_2 arg2) {
    recordAttempt(10, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_bigint_int64(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_bigint_int64), decltype(&napi_create_bigint_int64)>::value, "SDK signature mismatch: napi_create_bigint_int64");
using MadbArg_napi_create_bigint_uint64_0 = napi_env;
using MadbArg_napi_create_bigint_uint64_1 = uint64_t;
using MadbArg_napi_create_bigint_uint64_2 = napi_value *;
extern "C" decltype(napi_create_bigint_uint64) __real_napi_create_bigint_uint64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_bigint_uint64(MadbArg_napi_create_bigint_uint64_0 arg0, MadbArg_napi_create_bigint_uint64_1 arg1, MadbArg_napi_create_bigint_uint64_2 arg2) {
    recordAttempt(11, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_bigint_uint64(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_bigint_uint64), decltype(&napi_create_bigint_uint64)>::value, "SDK signature mismatch: napi_create_bigint_uint64");
using MadbArg_napi_create_callsite_info_0 = napi_env;
using MadbArg_napi_create_callsite_info_1 = napi_callsite_info *;
extern "C" decltype(napi_create_callsite_info) __real_napi_create_callsite_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_callsite_info(MadbArg_napi_create_callsite_info_0 arg0, MadbArg_napi_create_callsite_info_1 arg1) {
    recordAttempt(12, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_callsite_info(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_callsite_info), decltype(&napi_create_callsite_info)>::value, "SDK signature mismatch: napi_create_callsite_info");
using MadbArg_napi_create_double_0 = napi_env;
using MadbArg_napi_create_double_1 = double;
using MadbArg_napi_create_double_2 = napi_value *;
extern "C" decltype(napi_create_double) __real_napi_create_double;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_double(MadbArg_napi_create_double_0 arg0, MadbArg_napi_create_double_1 arg1, MadbArg_napi_create_double_2 arg2) {
    recordAttempt(13, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_double(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_double), decltype(&napi_create_double)>::value, "SDK signature mismatch: napi_create_double");
using MadbArg_napi_create_error_0 = napi_env;
using MadbArg_napi_create_error_1 = napi_value;
using MadbArg_napi_create_error_2 = napi_value;
using MadbArg_napi_create_error_3 = napi_value *;
extern "C" decltype(napi_create_error) __real_napi_create_error;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_error(MadbArg_napi_create_error_0 arg0, MadbArg_napi_create_error_1 arg1, MadbArg_napi_create_error_2 arg2, MadbArg_napi_create_error_3 arg3) {
    recordAttempt(14, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_error(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_error), decltype(&napi_create_error)>::value, "SDK signature mismatch: napi_create_error");
using MadbArg_napi_create_external_0 = napi_env;
using MadbArg_napi_create_external_1 = void *;
using MadbArg_napi_create_external_2 = napi_finalize;
using MadbArg_napi_create_external_3 = void *;
using MadbArg_napi_create_external_4 = napi_value *;
extern "C" decltype(napi_create_external) __real_napi_create_external;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_external(MadbArg_napi_create_external_0 arg0, MadbArg_napi_create_external_1 arg1, MadbArg_napi_create_external_2 arg2, MadbArg_napi_create_external_3 arg3, MadbArg_napi_create_external_4 arg4) {
    recordAttempt(15, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_external(arg0, arg1, arg2, arg3, arg4);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_external), decltype(&napi_create_external)>::value, "SDK signature mismatch: napi_create_external");
using MadbArg_napi_create_function_0 = napi_env;
using MadbArg_napi_create_function_1 = const char *;
using MadbArg_napi_create_function_2 = size_t;
using MadbArg_napi_create_function_3 = napi_callback;
using MadbArg_napi_create_function_4 = void *;
using MadbArg_napi_create_function_5 = napi_value *;
extern "C" decltype(napi_create_function) __real_napi_create_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_function(MadbArg_napi_create_function_0 arg0, MadbArg_napi_create_function_1 arg1, MadbArg_napi_create_function_2 arg2, MadbArg_napi_create_function_3 arg3, MadbArg_napi_create_function_4 arg4, MadbArg_napi_create_function_5 arg5) {
    recordAttempt(16, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_function(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_function), decltype(&napi_create_function)>::value, "SDK signature mismatch: napi_create_function");
using MadbArg_napi_create_int32_0 = napi_env;
using MadbArg_napi_create_int32_1 = int32_t;
using MadbArg_napi_create_int32_2 = napi_value *;
extern "C" decltype(napi_create_int32) __real_napi_create_int32;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_int32(MadbArg_napi_create_int32_0 arg0, MadbArg_napi_create_int32_1 arg1, MadbArg_napi_create_int32_2 arg2) {
    recordAttempt(17, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_int32(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_int32), decltype(&napi_create_int32)>::value, "SDK signature mismatch: napi_create_int32");
using MadbArg_napi_create_int64_0 = napi_env;
using MadbArg_napi_create_int64_1 = int64_t;
using MadbArg_napi_create_int64_2 = napi_value *;
extern "C" decltype(napi_create_int64) __real_napi_create_int64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_int64(MadbArg_napi_create_int64_0 arg0, MadbArg_napi_create_int64_1 arg1, MadbArg_napi_create_int64_2 arg2) {
    recordAttempt(18, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_int64(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_int64), decltype(&napi_create_int64)>::value, "SDK signature mismatch: napi_create_int64");
using MadbArg_napi_create_object_0 = napi_env;
using MadbArg_napi_create_object_1 = napi_value *;
extern "C" decltype(napi_create_object) __real_napi_create_object;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_object(MadbArg_napi_create_object_0 arg0, MadbArg_napi_create_object_1 arg1) {
    recordAttempt(19, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_object(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_object), decltype(&napi_create_object)>::value, "SDK signature mismatch: napi_create_object");
using MadbArg_napi_create_promise_0 = napi_env;
using MadbArg_napi_create_promise_1 = napi_deferred *;
using MadbArg_napi_create_promise_2 = napi_value *;
extern "C" decltype(napi_create_promise) __real_napi_create_promise;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_promise(MadbArg_napi_create_promise_0 arg0, MadbArg_napi_create_promise_1 arg1, MadbArg_napi_create_promise_2 arg2) {
    recordAttempt(20, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_promise(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_promise), decltype(&napi_create_promise)>::value, "SDK signature mismatch: napi_create_promise");
using MadbArg_napi_create_reference_0 = napi_env;
using MadbArg_napi_create_reference_1 = napi_value;
using MadbArg_napi_create_reference_2 = uint32_t;
using MadbArg_napi_create_reference_3 = napi_ref *;
extern "C" decltype(napi_create_reference) __real_napi_create_reference;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_reference(MadbArg_napi_create_reference_0 arg0, MadbArg_napi_create_reference_1 arg1, MadbArg_napi_create_reference_2 arg2, MadbArg_napi_create_reference_3 arg3) {
    recordAttempt(21, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_reference(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_reference), decltype(&napi_create_reference)>::value, "SDK signature mismatch: napi_create_reference");
using MadbArg_napi_create_string_latin1_0 = napi_env;
using MadbArg_napi_create_string_latin1_1 = const char *;
using MadbArg_napi_create_string_latin1_2 = size_t;
using MadbArg_napi_create_string_latin1_3 = napi_value *;
extern "C" decltype(napi_create_string_latin1) __real_napi_create_string_latin1;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_string_latin1(MadbArg_napi_create_string_latin1_0 arg0, MadbArg_napi_create_string_latin1_1 arg1, MadbArg_napi_create_string_latin1_2 arg2, MadbArg_napi_create_string_latin1_3 arg3) {
    recordAttempt(22, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_string_latin1(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_string_latin1), decltype(&napi_create_string_latin1)>::value, "SDK signature mismatch: napi_create_string_latin1");
using MadbArg_napi_create_string_utf16_0 = napi_env;
using MadbArg_napi_create_string_utf16_1 = const char16_t *;
using MadbArg_napi_create_string_utf16_2 = size_t;
using MadbArg_napi_create_string_utf16_3 = napi_value *;
extern "C" decltype(napi_create_string_utf16) __real_napi_create_string_utf16;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_string_utf16(MadbArg_napi_create_string_utf16_0 arg0, MadbArg_napi_create_string_utf16_1 arg1, MadbArg_napi_create_string_utf16_2 arg2, MadbArg_napi_create_string_utf16_3 arg3) {
    recordAttempt(23, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_string_utf16(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_string_utf16), decltype(&napi_create_string_utf16)>::value, "SDK signature mismatch: napi_create_string_utf16");
using MadbArg_napi_create_string_utf8_0 = napi_env;
using MadbArg_napi_create_string_utf8_1 = const char *;
using MadbArg_napi_create_string_utf8_2 = size_t;
using MadbArg_napi_create_string_utf8_3 = napi_value *;
extern "C" decltype(napi_create_string_utf8) __real_napi_create_string_utf8;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_string_utf8(MadbArg_napi_create_string_utf8_0 arg0, MadbArg_napi_create_string_utf8_1 arg1, MadbArg_napi_create_string_utf8_2 arg2, MadbArg_napi_create_string_utf8_3 arg3) {
    recordAttempt(24, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_string_utf8(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_string_utf8), decltype(&napi_create_string_utf8)>::value, "SDK signature mismatch: napi_create_string_utf8");
using MadbArg_napi_create_threadsafe_function_0 = napi_env;
using MadbArg_napi_create_threadsafe_function_1 = napi_value;
using MadbArg_napi_create_threadsafe_function_2 = napi_value;
using MadbArg_napi_create_threadsafe_function_3 = napi_value;
using MadbArg_napi_create_threadsafe_function_4 = size_t;
using MadbArg_napi_create_threadsafe_function_5 = size_t;
using MadbArg_napi_create_threadsafe_function_6 = void *;
using MadbArg_napi_create_threadsafe_function_7 = napi_finalize;
using MadbArg_napi_create_threadsafe_function_8 = void *;
using MadbArg_napi_create_threadsafe_function_9 = napi_threadsafe_function_call_js;
using MadbArg_napi_create_threadsafe_function_10 = napi_threadsafe_function *;
extern "C" decltype(napi_create_threadsafe_function) __real_napi_create_threadsafe_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_threadsafe_function(MadbArg_napi_create_threadsafe_function_0 arg0, MadbArg_napi_create_threadsafe_function_1 arg1, MadbArg_napi_create_threadsafe_function_2 arg2, MadbArg_napi_create_threadsafe_function_3 arg3, MadbArg_napi_create_threadsafe_function_4 arg4, MadbArg_napi_create_threadsafe_function_5 arg5, MadbArg_napi_create_threadsafe_function_6 arg6, MadbArg_napi_create_threadsafe_function_7 arg7, MadbArg_napi_create_threadsafe_function_8 arg8, MadbArg_napi_create_threadsafe_function_9 arg9, MadbArg_napi_create_threadsafe_function_10 arg10) {
    recordAttempt(25, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_threadsafe_function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_threadsafe_function), decltype(&napi_create_threadsafe_function)>::value, "SDK signature mismatch: napi_create_threadsafe_function");
using MadbArg_napi_create_typedarray_0 = napi_env;
using MadbArg_napi_create_typedarray_1 = napi_typedarray_type;
using MadbArg_napi_create_typedarray_2 = size_t;
using MadbArg_napi_create_typedarray_3 = napi_value;
using MadbArg_napi_create_typedarray_4 = size_t;
using MadbArg_napi_create_typedarray_5 = napi_value *;
extern "C" decltype(napi_create_typedarray) __real_napi_create_typedarray;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_typedarray(MadbArg_napi_create_typedarray_0 arg0, MadbArg_napi_create_typedarray_1 arg1, MadbArg_napi_create_typedarray_2 arg2, MadbArg_napi_create_typedarray_3 arg3, MadbArg_napi_create_typedarray_4 arg4, MadbArg_napi_create_typedarray_5 arg5) {
    recordAttempt(26, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_typedarray(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_typedarray), decltype(&napi_create_typedarray)>::value, "SDK signature mismatch: napi_create_typedarray");
using MadbArg_napi_create_uint32_0 = napi_env;
using MadbArg_napi_create_uint32_1 = uint32_t;
using MadbArg_napi_create_uint32_2 = napi_value *;
extern "C" decltype(napi_create_uint32) __real_napi_create_uint32;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_create_uint32(MadbArg_napi_create_uint32_0 arg0, MadbArg_napi_create_uint32_1 arg1, MadbArg_napi_create_uint32_2 arg2) {
    recordAttempt(27, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_create_uint32(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_create_uint32), decltype(&napi_create_uint32)>::value, "SDK signature mismatch: napi_create_uint32");
using MadbArg_napi_define_class_0 = napi_env;
using MadbArg_napi_define_class_1 = const char *;
using MadbArg_napi_define_class_2 = size_t;
using MadbArg_napi_define_class_3 = napi_callback;
using MadbArg_napi_define_class_4 = void *;
using MadbArg_napi_define_class_5 = size_t;
using MadbArg_napi_define_class_6 = const napi_property_descriptor *;
using MadbArg_napi_define_class_7 = napi_value *;
extern "C" decltype(napi_define_class) __real_napi_define_class;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_define_class(MadbArg_napi_define_class_0 arg0, MadbArg_napi_define_class_1 arg1, MadbArg_napi_define_class_2 arg2, MadbArg_napi_define_class_3 arg3, MadbArg_napi_define_class_4 arg4, MadbArg_napi_define_class_5 arg5, MadbArg_napi_define_class_6 arg6, MadbArg_napi_define_class_7 arg7) {
    recordAttempt(28, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_define_class(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7);
}
static_assert(std::is_same<decltype(&__wrap_napi_define_class), decltype(&napi_define_class)>::value, "SDK signature mismatch: napi_define_class");
using MadbArg_napi_define_properties_0 = napi_env;
using MadbArg_napi_define_properties_1 = napi_value;
using MadbArg_napi_define_properties_2 = size_t;
using MadbArg_napi_define_properties_3 = const napi_property_descriptor *;
extern "C" decltype(napi_define_properties) __real_napi_define_properties;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_define_properties(MadbArg_napi_define_properties_0 arg0, MadbArg_napi_define_properties_1 arg1, MadbArg_napi_define_properties_2 arg2, MadbArg_napi_define_properties_3 arg3) {
    recordAttempt(29, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_define_properties(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_define_properties), decltype(&napi_define_properties)>::value, "SDK signature mismatch: napi_define_properties");
using MadbArg_napi_delete_callsite_info_0 = napi_env;
using MadbArg_napi_delete_callsite_info_1 = napi_callsite_info;
extern "C" decltype(napi_delete_callsite_info) __real_napi_delete_callsite_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_delete_callsite_info(MadbArg_napi_delete_callsite_info_0 arg0, MadbArg_napi_delete_callsite_info_1 arg1) {
    recordAttempt(30, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_delete_callsite_info(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_delete_callsite_info), decltype(&napi_delete_callsite_info)>::value, "SDK signature mismatch: napi_delete_callsite_info");
using MadbArg_napi_delete_reference_0 = napi_env;
using MadbArg_napi_delete_reference_1 = napi_ref;
extern "C" decltype(napi_delete_reference) __real_napi_delete_reference;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_delete_reference(MadbArg_napi_delete_reference_0 arg0, MadbArg_napi_delete_reference_1 arg1) {
    recordAttempt(31, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_delete_reference(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_delete_reference), decltype(&napi_delete_reference)>::value, "SDK signature mismatch: napi_delete_reference");
using MadbArg_napi_get_and_clear_last_exception_0 = napi_env;
using MadbArg_napi_get_and_clear_last_exception_1 = napi_value *;
extern "C" decltype(napi_get_and_clear_last_exception) __real_napi_get_and_clear_last_exception;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_and_clear_last_exception(MadbArg_napi_get_and_clear_last_exception_0 arg0, MadbArg_napi_get_and_clear_last_exception_1 arg1) {
    recordAttempt(32, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_and_clear_last_exception(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_and_clear_last_exception), decltype(&napi_get_and_clear_last_exception)>::value, "SDK signature mismatch: napi_get_and_clear_last_exception");
using MadbArg_napi_get_array_length_0 = napi_env;
using MadbArg_napi_get_array_length_1 = napi_value;
using MadbArg_napi_get_array_length_2 = uint32_t *;
extern "C" decltype(napi_get_array_length) __real_napi_get_array_length;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_array_length(MadbArg_napi_get_array_length_0 arg0, MadbArg_napi_get_array_length_1 arg1, MadbArg_napi_get_array_length_2 arg2) {
    recordAttempt(33, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_array_length(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_array_length), decltype(&napi_get_array_length)>::value, "SDK signature mismatch: napi_get_array_length");
using MadbArg_napi_get_arraybuffer_info_0 = napi_env;
using MadbArg_napi_get_arraybuffer_info_1 = napi_value;
using MadbArg_napi_get_arraybuffer_info_2 = void **;
using MadbArg_napi_get_arraybuffer_info_3 = size_t *;
extern "C" decltype(napi_get_arraybuffer_info) __real_napi_get_arraybuffer_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_arraybuffer_info(MadbArg_napi_get_arraybuffer_info_0 arg0, MadbArg_napi_get_arraybuffer_info_1 arg1, MadbArg_napi_get_arraybuffer_info_2 arg2, MadbArg_napi_get_arraybuffer_info_3 arg3) {
    recordAttempt(34, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_arraybuffer_info(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_arraybuffer_info), decltype(&napi_get_arraybuffer_info)>::value, "SDK signature mismatch: napi_get_arraybuffer_info");
using MadbArg_napi_get_boolean_0 = napi_env;
using MadbArg_napi_get_boolean_1 = bool;
using MadbArg_napi_get_boolean_2 = napi_value *;
extern "C" decltype(napi_get_boolean) __real_napi_get_boolean;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_boolean(MadbArg_napi_get_boolean_0 arg0, MadbArg_napi_get_boolean_1 arg1, MadbArg_napi_get_boolean_2 arg2) {
    recordAttempt(35, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_boolean(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_boolean), decltype(&napi_get_boolean)>::value, "SDK signature mismatch: napi_get_boolean");
using MadbArg_napi_get_cb_info_0 = napi_env;
using MadbArg_napi_get_cb_info_1 = napi_callback_info;
using MadbArg_napi_get_cb_info_2 = size_t *;
using MadbArg_napi_get_cb_info_3 = napi_value *;
using MadbArg_napi_get_cb_info_4 = napi_value *;
using MadbArg_napi_get_cb_info_5 = void **;
extern "C" decltype(napi_get_cb_info) __real_napi_get_cb_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_cb_info(MadbArg_napi_get_cb_info_0 arg0, MadbArg_napi_get_cb_info_1 arg1, MadbArg_napi_get_cb_info_2 arg2, MadbArg_napi_get_cb_info_3 arg3, MadbArg_napi_get_cb_info_4 arg4, MadbArg_napi_get_cb_info_5 arg5) {
    recordAttempt(36, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_cb_info(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_cb_info), decltype(&napi_get_cb_info)>::value, "SDK signature mismatch: napi_get_cb_info");
using MadbArg_napi_get_element_0 = napi_env;
using MadbArg_napi_get_element_1 = napi_value;
using MadbArg_napi_get_element_2 = uint32_t;
using MadbArg_napi_get_element_3 = napi_value *;
extern "C" decltype(napi_get_element) __real_napi_get_element;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_element(MadbArg_napi_get_element_0 arg0, MadbArg_napi_get_element_1 arg1, MadbArg_napi_get_element_2 arg2, MadbArg_napi_get_element_3 arg3) {
    recordAttempt(37, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_element(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_element), decltype(&napi_get_element)>::value, "SDK signature mismatch: napi_get_element");
using MadbArg_napi_get_global_0 = napi_env;
using MadbArg_napi_get_global_1 = napi_value *;
extern "C" decltype(napi_get_global) __real_napi_get_global;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_global(MadbArg_napi_get_global_0 arg0, MadbArg_napi_get_global_1 arg1) {
    recordAttempt(38, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_global(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_global), decltype(&napi_get_global)>::value, "SDK signature mismatch: napi_get_global");
using MadbArg_napi_get_named_property_0 = napi_env;
using MadbArg_napi_get_named_property_1 = napi_value;
using MadbArg_napi_get_named_property_2 = const char *;
using MadbArg_napi_get_named_property_3 = napi_value *;
extern "C" decltype(napi_get_named_property) __real_napi_get_named_property;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_named_property(MadbArg_napi_get_named_property_0 arg0, MadbArg_napi_get_named_property_1 arg1, MadbArg_napi_get_named_property_2 arg2, MadbArg_napi_get_named_property_3 arg3) {
    recordAttempt(39, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_named_property(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_named_property), decltype(&napi_get_named_property)>::value, "SDK signature mismatch: napi_get_named_property");
using MadbArg_napi_get_null_0 = napi_env;
using MadbArg_napi_get_null_1 = napi_value *;
extern "C" decltype(napi_get_null) __real_napi_get_null;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_null(MadbArg_napi_get_null_0 arg0, MadbArg_napi_get_null_1 arg1) {
    recordAttempt(40, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_null(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_null), decltype(&napi_get_null)>::value, "SDK signature mismatch: napi_get_null");
using MadbArg_napi_get_property_0 = napi_env;
using MadbArg_napi_get_property_1 = napi_value;
using MadbArg_napi_get_property_2 = napi_value;
using MadbArg_napi_get_property_3 = napi_value *;
extern "C" decltype(napi_get_property) __real_napi_get_property;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_property(MadbArg_napi_get_property_0 arg0, MadbArg_napi_get_property_1 arg1, MadbArg_napi_get_property_2 arg2, MadbArg_napi_get_property_3 arg3) {
    recordAttempt(41, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_property(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_property), decltype(&napi_get_property)>::value, "SDK signature mismatch: napi_get_property");
using MadbArg_napi_get_property_names_0 = napi_env;
using MadbArg_napi_get_property_names_1 = napi_value;
using MadbArg_napi_get_property_names_2 = napi_value *;
extern "C" decltype(napi_get_property_names) __real_napi_get_property_names;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_property_names(MadbArg_napi_get_property_names_0 arg0, MadbArg_napi_get_property_names_1 arg1, MadbArg_napi_get_property_names_2 arg2) {
    recordAttempt(42, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_property_names(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_property_names), decltype(&napi_get_property_names)>::value, "SDK signature mismatch: napi_get_property_names");
using MadbArg_napi_get_property_with_callsite_info_0 = napi_env;
using MadbArg_napi_get_property_with_callsite_info_1 = napi_value;
using MadbArg_napi_get_property_with_callsite_info_2 = napi_value;
using MadbArg_napi_get_property_with_callsite_info_3 = napi_callsite_info;
using MadbArg_napi_get_property_with_callsite_info_4 = napi_value *;
using MadbArg_napi_get_property_with_callsite_info_5 = bool *;
extern "C" decltype(napi_get_property_with_callsite_info) __real_napi_get_property_with_callsite_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_property_with_callsite_info(MadbArg_napi_get_property_with_callsite_info_0 arg0, MadbArg_napi_get_property_with_callsite_info_1 arg1, MadbArg_napi_get_property_with_callsite_info_2 arg2, MadbArg_napi_get_property_with_callsite_info_3 arg3, MadbArg_napi_get_property_with_callsite_info_4 arg4, MadbArg_napi_get_property_with_callsite_info_5 arg5) {
    recordAttempt(43, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_property_with_callsite_info(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_property_with_callsite_info), decltype(&napi_get_property_with_callsite_info)>::value, "SDK signature mismatch: napi_get_property_with_callsite_info");
using MadbArg_napi_get_reference_value_0 = napi_env;
using MadbArg_napi_get_reference_value_1 = napi_ref;
using MadbArg_napi_get_reference_value_2 = napi_value *;
extern "C" decltype(napi_get_reference_value) __real_napi_get_reference_value;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_reference_value(MadbArg_napi_get_reference_value_0 arg0, MadbArg_napi_get_reference_value_1 arg1, MadbArg_napi_get_reference_value_2 arg2) {
    recordAttempt(44, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_reference_value(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_reference_value), decltype(&napi_get_reference_value)>::value, "SDK signature mismatch: napi_get_reference_value");
using MadbArg_napi_get_typedarray_info_0 = napi_env;
using MadbArg_napi_get_typedarray_info_1 = napi_value;
using MadbArg_napi_get_typedarray_info_2 = napi_typedarray_type *;
using MadbArg_napi_get_typedarray_info_3 = size_t *;
using MadbArg_napi_get_typedarray_info_4 = void **;
using MadbArg_napi_get_typedarray_info_5 = napi_value *;
using MadbArg_napi_get_typedarray_info_6 = size_t *;
extern "C" decltype(napi_get_typedarray_info) __real_napi_get_typedarray_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_typedarray_info(MadbArg_napi_get_typedarray_info_0 arg0, MadbArg_napi_get_typedarray_info_1 arg1, MadbArg_napi_get_typedarray_info_2 arg2, MadbArg_napi_get_typedarray_info_3 arg3, MadbArg_napi_get_typedarray_info_4 arg4, MadbArg_napi_get_typedarray_info_5 arg5, MadbArg_napi_get_typedarray_info_6 arg6) {
    recordAttempt(45, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_typedarray_info(arg0, arg1, arg2, arg3, arg4, arg5, arg6);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_typedarray_info), decltype(&napi_get_typedarray_info)>::value, "SDK signature mismatch: napi_get_typedarray_info");
using MadbArg_napi_get_undefined_0 = napi_env;
using MadbArg_napi_get_undefined_1 = napi_value *;
extern "C" decltype(napi_get_undefined) __real_napi_get_undefined;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_undefined(MadbArg_napi_get_undefined_0 arg0, MadbArg_napi_get_undefined_1 arg1) {
    recordAttempt(46, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_undefined(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_undefined), decltype(&napi_get_undefined)>::value, "SDK signature mismatch: napi_get_undefined");
using MadbArg_napi_get_uv_event_loop_0 = napi_env;
using MadbArg_napi_get_uv_event_loop_1 = struct uv_loop_s **;
extern "C" decltype(napi_get_uv_event_loop) __real_napi_get_uv_event_loop;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_uv_event_loop(MadbArg_napi_get_uv_event_loop_0 arg0, MadbArg_napi_get_uv_event_loop_1 arg1) {
    recordAttempt(47, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_uv_event_loop(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_uv_event_loop), decltype(&napi_get_uv_event_loop)>::value, "SDK signature mismatch: napi_get_uv_event_loop");
using MadbArg_napi_get_value_bigint_int64_0 = napi_env;
using MadbArg_napi_get_value_bigint_int64_1 = napi_value;
using MadbArg_napi_get_value_bigint_int64_2 = int64_t *;
using MadbArg_napi_get_value_bigint_int64_3 = bool *;
extern "C" decltype(napi_get_value_bigint_int64) __real_napi_get_value_bigint_int64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_bigint_int64(MadbArg_napi_get_value_bigint_int64_0 arg0, MadbArg_napi_get_value_bigint_int64_1 arg1, MadbArg_napi_get_value_bigint_int64_2 arg2, MadbArg_napi_get_value_bigint_int64_3 arg3) {
    recordAttempt(48, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_bigint_int64(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_bigint_int64), decltype(&napi_get_value_bigint_int64)>::value, "SDK signature mismatch: napi_get_value_bigint_int64");
using MadbArg_napi_get_value_bigint_uint64_0 = napi_env;
using MadbArg_napi_get_value_bigint_uint64_1 = napi_value;
using MadbArg_napi_get_value_bigint_uint64_2 = uint64_t *;
using MadbArg_napi_get_value_bigint_uint64_3 = bool *;
extern "C" decltype(napi_get_value_bigint_uint64) __real_napi_get_value_bigint_uint64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_bigint_uint64(MadbArg_napi_get_value_bigint_uint64_0 arg0, MadbArg_napi_get_value_bigint_uint64_1 arg1, MadbArg_napi_get_value_bigint_uint64_2 arg2, MadbArg_napi_get_value_bigint_uint64_3 arg3) {
    recordAttempt(49, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_bigint_uint64(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_bigint_uint64), decltype(&napi_get_value_bigint_uint64)>::value, "SDK signature mismatch: napi_get_value_bigint_uint64");
using MadbArg_napi_get_value_bool_0 = napi_env;
using MadbArg_napi_get_value_bool_1 = napi_value;
using MadbArg_napi_get_value_bool_2 = bool *;
extern "C" decltype(napi_get_value_bool) __real_napi_get_value_bool;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_bool(MadbArg_napi_get_value_bool_0 arg0, MadbArg_napi_get_value_bool_1 arg1, MadbArg_napi_get_value_bool_2 arg2) {
    recordAttempt(50, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_bool(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_bool), decltype(&napi_get_value_bool)>::value, "SDK signature mismatch: napi_get_value_bool");
using MadbArg_napi_get_value_double_0 = napi_env;
using MadbArg_napi_get_value_double_1 = napi_value;
using MadbArg_napi_get_value_double_2 = double *;
extern "C" decltype(napi_get_value_double) __real_napi_get_value_double;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_double(MadbArg_napi_get_value_double_0 arg0, MadbArg_napi_get_value_double_1 arg1, MadbArg_napi_get_value_double_2 arg2) {
    recordAttempt(51, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_double(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_double), decltype(&napi_get_value_double)>::value, "SDK signature mismatch: napi_get_value_double");
using MadbArg_napi_get_value_external_0 = napi_env;
using MadbArg_napi_get_value_external_1 = napi_value;
using MadbArg_napi_get_value_external_2 = void **;
extern "C" decltype(napi_get_value_external) __real_napi_get_value_external;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_external(MadbArg_napi_get_value_external_0 arg0, MadbArg_napi_get_value_external_1 arg1, MadbArg_napi_get_value_external_2 arg2) {
    recordAttempt(52, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_external(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_external), decltype(&napi_get_value_external)>::value, "SDK signature mismatch: napi_get_value_external");
using MadbArg_napi_get_value_int32_0 = napi_env;
using MadbArg_napi_get_value_int32_1 = napi_value;
using MadbArg_napi_get_value_int32_2 = int32_t *;
extern "C" decltype(napi_get_value_int32) __real_napi_get_value_int32;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_int32(MadbArg_napi_get_value_int32_0 arg0, MadbArg_napi_get_value_int32_1 arg1, MadbArg_napi_get_value_int32_2 arg2) {
    recordAttempt(53, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_int32(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_int32), decltype(&napi_get_value_int32)>::value, "SDK signature mismatch: napi_get_value_int32");
using MadbArg_napi_get_value_int64_0 = napi_env;
using MadbArg_napi_get_value_int64_1 = napi_value;
using MadbArg_napi_get_value_int64_2 = int64_t *;
extern "C" decltype(napi_get_value_int64) __real_napi_get_value_int64;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_int64(MadbArg_napi_get_value_int64_0 arg0, MadbArg_napi_get_value_int64_1 arg1, MadbArg_napi_get_value_int64_2 arg2) {
    recordAttempt(54, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_int64(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_int64), decltype(&napi_get_value_int64)>::value, "SDK signature mismatch: napi_get_value_int64");
using MadbArg_napi_get_value_string_utf16_0 = napi_env;
using MadbArg_napi_get_value_string_utf16_1 = napi_value;
using MadbArg_napi_get_value_string_utf16_2 = char16_t *;
using MadbArg_napi_get_value_string_utf16_3 = size_t;
using MadbArg_napi_get_value_string_utf16_4 = size_t *;
extern "C" decltype(napi_get_value_string_utf16) __real_napi_get_value_string_utf16;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_string_utf16(MadbArg_napi_get_value_string_utf16_0 arg0, MadbArg_napi_get_value_string_utf16_1 arg1, MadbArg_napi_get_value_string_utf16_2 arg2, MadbArg_napi_get_value_string_utf16_3 arg3, MadbArg_napi_get_value_string_utf16_4 arg4) {
    recordAttempt(55, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_string_utf16(arg0, arg1, arg2, arg3, arg4);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_string_utf16), decltype(&napi_get_value_string_utf16)>::value, "SDK signature mismatch: napi_get_value_string_utf16");
using MadbArg_napi_get_value_uint32_0 = napi_env;
using MadbArg_napi_get_value_uint32_1 = napi_value;
using MadbArg_napi_get_value_uint32_2 = uint32_t *;
extern "C" decltype(napi_get_value_uint32) __real_napi_get_value_uint32;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_get_value_uint32(MadbArg_napi_get_value_uint32_0 arg0, MadbArg_napi_get_value_uint32_1 arg1, MadbArg_napi_get_value_uint32_2 arg2) {
    recordAttempt(56, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_get_value_uint32(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_get_value_uint32), decltype(&napi_get_value_uint32)>::value, "SDK signature mismatch: napi_get_value_uint32");
using MadbArg_napi_instanceof_0 = napi_env;
using MadbArg_napi_instanceof_1 = napi_value;
using MadbArg_napi_instanceof_2 = napi_value;
using MadbArg_napi_instanceof_3 = bool *;
extern "C" decltype(napi_instanceof) __real_napi_instanceof;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_instanceof(MadbArg_napi_instanceof_0 arg0, MadbArg_napi_instanceof_1 arg1, MadbArg_napi_instanceof_2 arg2, MadbArg_napi_instanceof_3 arg3) {
    recordAttempt(57, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_instanceof(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_instanceof), decltype(&napi_instanceof)>::value, "SDK signature mismatch: napi_instanceof");
using MadbArg_napi_is_array_0 = napi_env;
using MadbArg_napi_is_array_1 = napi_value;
using MadbArg_napi_is_array_2 = bool *;
extern "C" decltype(napi_is_array) __real_napi_is_array;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_array(MadbArg_napi_is_array_0 arg0, MadbArg_napi_is_array_1 arg1, MadbArg_napi_is_array_2 arg2) {
    recordAttempt(58, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_array(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_array), decltype(&napi_is_array)>::value, "SDK signature mismatch: napi_is_array");
using MadbArg_napi_is_arraybuffer_0 = napi_env;
using MadbArg_napi_is_arraybuffer_1 = napi_value;
using MadbArg_napi_is_arraybuffer_2 = bool *;
extern "C" decltype(napi_is_arraybuffer) __real_napi_is_arraybuffer;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_arraybuffer(MadbArg_napi_is_arraybuffer_0 arg0, MadbArg_napi_is_arraybuffer_1 arg1, MadbArg_napi_is_arraybuffer_2 arg2) {
    recordAttempt(59, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_arraybuffer(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_arraybuffer), decltype(&napi_is_arraybuffer)>::value, "SDK signature mismatch: napi_is_arraybuffer");
using MadbArg_napi_is_exception_pending_0 = napi_env;
using MadbArg_napi_is_exception_pending_1 = bool *;
extern "C" decltype(napi_is_exception_pending) __real_napi_is_exception_pending;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_exception_pending(MadbArg_napi_is_exception_pending_0 arg0, MadbArg_napi_is_exception_pending_1 arg1) {
    recordAttempt(60, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_exception_pending(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_exception_pending), decltype(&napi_is_exception_pending)>::value, "SDK signature mismatch: napi_is_exception_pending");
using MadbArg_napi_is_promise_0 = napi_env;
using MadbArg_napi_is_promise_1 = napi_value;
using MadbArg_napi_is_promise_2 = bool *;
extern "C" decltype(napi_is_promise) __real_napi_is_promise;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_promise(MadbArg_napi_is_promise_0 arg0, MadbArg_napi_is_promise_1 arg1, MadbArg_napi_is_promise_2 arg2) {
    recordAttempt(61, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_promise(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_promise), decltype(&napi_is_promise)>::value, "SDK signature mismatch: napi_is_promise");
using MadbArg_napi_is_typedarray_0 = napi_env;
using MadbArg_napi_is_typedarray_1 = napi_value;
using MadbArg_napi_is_typedarray_2 = bool *;
extern "C" decltype(napi_is_typedarray) __real_napi_is_typedarray;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_is_typedarray(MadbArg_napi_is_typedarray_0 arg0, MadbArg_napi_is_typedarray_1 arg1, MadbArg_napi_is_typedarray_2 arg2) {
    recordAttempt(62, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_is_typedarray(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_is_typedarray), decltype(&napi_is_typedarray)>::value, "SDK signature mismatch: napi_is_typedarray");
using MadbArg_napi_module_register_0 = napi_module *;
extern "C" decltype(napi_module_register) __real_napi_module_register;
extern "C" __attribute__((visibility("hidden"))) void __wrap_napi_module_register(MadbArg_napi_module_register_0 arg0) {
    recordAttempt(63, 2, nullptr);
    __real_napi_module_register(arg0);
}
static_assert(std::is_same<decltype(&__wrap_napi_module_register), decltype(&napi_module_register)>::value, "SDK signature mismatch: napi_module_register");
using MadbArg_napi_new_instance_0 = napi_env;
using MadbArg_napi_new_instance_1 = napi_value;
using MadbArg_napi_new_instance_2 = size_t;
using MadbArg_napi_new_instance_3 = const napi_value *;
using MadbArg_napi_new_instance_4 = napi_value *;
extern "C" decltype(napi_new_instance) __real_napi_new_instance;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_new_instance(MadbArg_napi_new_instance_0 arg0, MadbArg_napi_new_instance_1 arg1, MadbArg_napi_new_instance_2 arg2, MadbArg_napi_new_instance_3 arg3, MadbArg_napi_new_instance_4 arg4) {
    recordAttempt(64, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_new_instance(arg0, arg1, arg2, arg3, arg4);
}
static_assert(std::is_same<decltype(&__wrap_napi_new_instance), decltype(&napi_new_instance)>::value, "SDK signature mismatch: napi_new_instance");
using MadbArg_napi_open_handle_scope_0 = napi_env;
using MadbArg_napi_open_handle_scope_1 = napi_handle_scope *;
extern "C" decltype(napi_open_handle_scope) __real_napi_open_handle_scope;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_open_handle_scope(MadbArg_napi_open_handle_scope_0 arg0, MadbArg_napi_open_handle_scope_1 arg1) {
    recordAttempt(65, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_open_handle_scope(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_open_handle_scope), decltype(&napi_open_handle_scope)>::value, "SDK signature mismatch: napi_open_handle_scope");
using MadbArg_napi_reject_deferred_0 = napi_env;
using MadbArg_napi_reject_deferred_1 = napi_deferred;
using MadbArg_napi_reject_deferred_2 = napi_value;
extern "C" decltype(napi_reject_deferred) __real_napi_reject_deferred;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_reject_deferred(MadbArg_napi_reject_deferred_0 arg0, MadbArg_napi_reject_deferred_1 arg1, MadbArg_napi_reject_deferred_2 arg2) {
    recordAttempt(66, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_reject_deferred(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_reject_deferred), decltype(&napi_reject_deferred)>::value, "SDK signature mismatch: napi_reject_deferred");
using MadbArg_napi_release_threadsafe_function_0 = napi_threadsafe_function;
using MadbArg_napi_release_threadsafe_function_1 = napi_threadsafe_function_release_mode;
extern "C" decltype(napi_release_threadsafe_function) __real_napi_release_threadsafe_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_release_threadsafe_function(MadbArg_napi_release_threadsafe_function_0 arg0, MadbArg_napi_release_threadsafe_function_1 arg1) {
    recordAttempt(67, 1, nullptr);
    return __real_napi_release_threadsafe_function(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_release_threadsafe_function), decltype(&napi_release_threadsafe_function)>::value, "SDK signature mismatch: napi_release_threadsafe_function");
using MadbArg_napi_remove_async_cleanup_hook_0 = napi_async_cleanup_hook_handle;
extern "C" decltype(napi_remove_async_cleanup_hook) __real_napi_remove_async_cleanup_hook;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_remove_async_cleanup_hook(MadbArg_napi_remove_async_cleanup_hook_0 arg0) {
    recordAttempt(68, 3, nullptr);
    return __real_napi_remove_async_cleanup_hook(arg0);
}
static_assert(std::is_same<decltype(&__wrap_napi_remove_async_cleanup_hook), decltype(&napi_remove_async_cleanup_hook)>::value, "SDK signature mismatch: napi_remove_async_cleanup_hook");
using MadbArg_napi_remove_env_cleanup_hook_0 = napi_env;
using MadbArg_napi_remove_env_cleanup_hook_1 = void (*)(void *);
using MadbArg_napi_remove_env_cleanup_hook_2 = void *;
extern "C" decltype(napi_remove_env_cleanup_hook) __real_napi_remove_env_cleanup_hook;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_remove_env_cleanup_hook(MadbArg_napi_remove_env_cleanup_hook_0 arg0, MadbArg_napi_remove_env_cleanup_hook_1 arg1, MadbArg_napi_remove_env_cleanup_hook_2 arg2) {
    recordAttempt(69, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_remove_env_cleanup_hook(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_remove_env_cleanup_hook), decltype(&napi_remove_env_cleanup_hook)>::value, "SDK signature mismatch: napi_remove_env_cleanup_hook");
using MadbArg_napi_resolve_deferred_0 = napi_env;
using MadbArg_napi_resolve_deferred_1 = napi_deferred;
using MadbArg_napi_resolve_deferred_2 = napi_value;
extern "C" decltype(napi_resolve_deferred) __real_napi_resolve_deferred;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_resolve_deferred(MadbArg_napi_resolve_deferred_0 arg0, MadbArg_napi_resolve_deferred_1 arg1, MadbArg_napi_resolve_deferred_2 arg2) {
    recordAttempt(70, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_resolve_deferred(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_resolve_deferred), decltype(&napi_resolve_deferred)>::value, "SDK signature mismatch: napi_resolve_deferred");
using MadbArg_napi_set_element_0 = napi_env;
using MadbArg_napi_set_element_1 = napi_value;
using MadbArg_napi_set_element_2 = uint32_t;
using MadbArg_napi_set_element_3 = napi_value;
extern "C" decltype(napi_set_element) __real_napi_set_element;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_set_element(MadbArg_napi_set_element_0 arg0, MadbArg_napi_set_element_1 arg1, MadbArg_napi_set_element_2 arg2, MadbArg_napi_set_element_3 arg3) {
    recordAttempt(71, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_set_element(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_set_element), decltype(&napi_set_element)>::value, "SDK signature mismatch: napi_set_element");
using MadbArg_napi_set_named_property_0 = napi_env;
using MadbArg_napi_set_named_property_1 = napi_value;
using MadbArg_napi_set_named_property_2 = const char *;
using MadbArg_napi_set_named_property_3 = napi_value;
extern "C" decltype(napi_set_named_property) __real_napi_set_named_property;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_set_named_property(MadbArg_napi_set_named_property_0 arg0, MadbArg_napi_set_named_property_1 arg1, MadbArg_napi_set_named_property_2 arg2, MadbArg_napi_set_named_property_3 arg3) {
    recordAttempt(72, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_set_named_property(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_set_named_property), decltype(&napi_set_named_property)>::value, "SDK signature mismatch: napi_set_named_property");
using MadbArg_napi_set_property_0 = napi_env;
using MadbArg_napi_set_property_1 = napi_value;
using MadbArg_napi_set_property_2 = napi_value;
using MadbArg_napi_set_property_3 = napi_value;
extern "C" decltype(napi_set_property) __real_napi_set_property;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_set_property(MadbArg_napi_set_property_0 arg0, MadbArg_napi_set_property_1 arg1, MadbArg_napi_set_property_2 arg2, MadbArg_napi_set_property_3 arg3) {
    recordAttempt(73, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_set_property(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_set_property), decltype(&napi_set_property)>::value, "SDK signature mismatch: napi_set_property");
using MadbArg_napi_set_property_with_callsite_info_0 = napi_env;
using MadbArg_napi_set_property_with_callsite_info_1 = napi_value;
using MadbArg_napi_set_property_with_callsite_info_2 = napi_value;
using MadbArg_napi_set_property_with_callsite_info_3 = napi_value;
using MadbArg_napi_set_property_with_callsite_info_4 = napi_callsite_info;
using MadbArg_napi_set_property_with_callsite_info_5 = bool *;
extern "C" decltype(napi_set_property_with_callsite_info) __real_napi_set_property_with_callsite_info;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_set_property_with_callsite_info(MadbArg_napi_set_property_with_callsite_info_0 arg0, MadbArg_napi_set_property_with_callsite_info_1 arg1, MadbArg_napi_set_property_with_callsite_info_2 arg2, MadbArg_napi_set_property_with_callsite_info_3 arg3, MadbArg_napi_set_property_with_callsite_info_4 arg4, MadbArg_napi_set_property_with_callsite_info_5 arg5) {
    recordAttempt(74, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_set_property_with_callsite_info(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_set_property_with_callsite_info), decltype(&napi_set_property_with_callsite_info)>::value, "SDK signature mismatch: napi_set_property_with_callsite_info");
using MadbArg_napi_strict_equals_0 = napi_env;
using MadbArg_napi_strict_equals_1 = napi_value;
using MadbArg_napi_strict_equals_2 = napi_value;
using MadbArg_napi_strict_equals_3 = bool *;
extern "C" decltype(napi_strict_equals) __real_napi_strict_equals;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_strict_equals(MadbArg_napi_strict_equals_0 arg0, MadbArg_napi_strict_equals_1 arg1, MadbArg_napi_strict_equals_2 arg2, MadbArg_napi_strict_equals_3 arg3) {
    recordAttempt(75, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_strict_equals(arg0, arg1, arg2, arg3);
}
static_assert(std::is_same<decltype(&__wrap_napi_strict_equals), decltype(&napi_strict_equals)>::value, "SDK signature mismatch: napi_strict_equals");
using MadbArg_napi_throw_error_0 = napi_env;
using MadbArg_napi_throw_error_1 = const char *;
using MadbArg_napi_throw_error_2 = const char *;
extern "C" decltype(napi_throw_error) __real_napi_throw_error;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_throw_error(MadbArg_napi_throw_error_0 arg0, MadbArg_napi_throw_error_1 arg1, MadbArg_napi_throw_error_2 arg2) {
    recordAttempt(76, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_throw_error(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_throw_error), decltype(&napi_throw_error)>::value, "SDK signature mismatch: napi_throw_error");
using MadbArg_napi_throw_range_error_0 = napi_env;
using MadbArg_napi_throw_range_error_1 = const char *;
using MadbArg_napi_throw_range_error_2 = const char *;
extern "C" decltype(napi_throw_range_error) __real_napi_throw_range_error;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_throw_range_error(MadbArg_napi_throw_range_error_0 arg0, MadbArg_napi_throw_range_error_1 arg1, MadbArg_napi_throw_range_error_2 arg2) {
    recordAttempt(77, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_throw_range_error(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_throw_range_error), decltype(&napi_throw_range_error)>::value, "SDK signature mismatch: napi_throw_range_error");
using MadbArg_napi_throw_type_error_0 = napi_env;
using MadbArg_napi_throw_type_error_1 = const char *;
using MadbArg_napi_throw_type_error_2 = const char *;
extern "C" decltype(napi_throw_type_error) __real_napi_throw_type_error;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_throw_type_error(MadbArg_napi_throw_type_error_0 arg0, MadbArg_napi_throw_type_error_1 arg1, MadbArg_napi_throw_type_error_2 arg2) {
    recordAttempt(78, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_throw_type_error(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_throw_type_error), decltype(&napi_throw_type_error)>::value, "SDK signature mismatch: napi_throw_type_error");
using MadbArg_napi_typeof_0 = napi_env;
using MadbArg_napi_typeof_1 = napi_value;
using MadbArg_napi_typeof_2 = napi_valuetype *;
extern "C" decltype(napi_typeof) __real_napi_typeof;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_typeof(MadbArg_napi_typeof_0 arg0, MadbArg_napi_typeof_1 arg1, MadbArg_napi_typeof_2 arg2) {
    recordAttempt(79, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_typeof(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_typeof), decltype(&napi_typeof)>::value, "SDK signature mismatch: napi_typeof");
using MadbArg_napi_unref_threadsafe_function_0 = napi_env;
using MadbArg_napi_unref_threadsafe_function_1 = napi_threadsafe_function;
extern "C" decltype(napi_unref_threadsafe_function) __real_napi_unref_threadsafe_function;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_unref_threadsafe_function(MadbArg_napi_unref_threadsafe_function_0 arg0, MadbArg_napi_unref_threadsafe_function_1 arg1) {
    recordAttempt(80, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_unref_threadsafe_function(arg0, arg1);
}
static_assert(std::is_same<decltype(&__wrap_napi_unref_threadsafe_function), decltype(&napi_unref_threadsafe_function)>::value, "SDK signature mismatch: napi_unref_threadsafe_function");
using MadbArg_napi_unwrap_0 = napi_env;
using MadbArg_napi_unwrap_1 = napi_value;
using MadbArg_napi_unwrap_2 = void **;
extern "C" decltype(napi_unwrap) __real_napi_unwrap;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_unwrap(MadbArg_napi_unwrap_0 arg0, MadbArg_napi_unwrap_1 arg1, MadbArg_napi_unwrap_2 arg2) {
    recordAttempt(81, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_unwrap(arg0, arg1, arg2);
}
static_assert(std::is_same<decltype(&__wrap_napi_unwrap), decltype(&napi_unwrap)>::value, "SDK signature mismatch: napi_unwrap");
using MadbArg_napi_wrap_0 = napi_env;
using MadbArg_napi_wrap_1 = napi_value;
using MadbArg_napi_wrap_2 = void *;
using MadbArg_napi_wrap_3 = napi_finalize;
using MadbArg_napi_wrap_4 = void *;
using MadbArg_napi_wrap_5 = napi_ref *;
extern "C" decltype(napi_wrap) __real_napi_wrap;
extern "C" __attribute__((visibility("hidden"))) napi_status __wrap_napi_wrap(MadbArg_napi_wrap_0 arg0, MadbArg_napi_wrap_1 arg1, MadbArg_napi_wrap_2 arg2, MadbArg_napi_wrap_3 arg3, MadbArg_napi_wrap_4 arg4, MadbArg_napi_wrap_5 arg5) {
    recordAttempt(82, 0, reinterpret_cast<void *>(arg0));
    return __real_napi_wrap(arg0, arg1, arg2, arg3, arg4, arg5);
}
static_assert(std::is_same<decltype(&__wrap_napi_wrap), decltype(&napi_wrap)>::value, "SDK signature mismatch: napi_wrap");
