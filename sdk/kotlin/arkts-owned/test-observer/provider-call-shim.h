// Generated call-only interception; include after original weak declarations.
#pragma once
extern "C" __attribute__((visibility("hidden"))) decltype(napi_create_callsite_info) __wrap_napi_create_callsite_info;
#define napi_create_callsite_info(...) __wrap_napi_create_callsite_info(__VA_ARGS__)
extern "C" __attribute__((visibility("hidden"))) decltype(napi_delete_callsite_info) __wrap_napi_delete_callsite_info;
#define napi_delete_callsite_info(...) __wrap_napi_delete_callsite_info(__VA_ARGS__)
extern "C" __attribute__((visibility("hidden"))) decltype(napi_get_property_with_callsite_info) __wrap_napi_get_property_with_callsite_info;
#define napi_get_property_with_callsite_info(...) __wrap_napi_get_property_with_callsite_info(__VA_ARGS__)
extern "C" __attribute__((visibility("hidden"))) decltype(napi_set_property_with_callsite_info) __wrap_napi_set_property_with_callsite_info;
#define napi_set_property_with_callsite_info(...) __wrap_napi_set_property_with_callsite_info(__VA_ARGS__)
