#include "property.h"
#include "logger.h"

// ---- weak extern declarations for IC APIs (@since API 24) ----
// SDK<24 的 native_api.h 不提供声明；weak 保证 API<24 设备加载不失败。
extern "C" __attribute__((weak))
napi_status napi_create_callsite_info(napi_env env, napi_callsite_info* result);

extern "C" __attribute__((weak))
napi_status napi_delete_callsite_info(napi_env env, napi_callsite_info info);

extern "C" __attribute__((weak))
napi_status napi_get_property_with_callsite_info(
    napi_env env, napi_value object, napi_value key,
    napi_callsite_info info, napi_value* result, bool* hit);

extern "C" __attribute__((weak))
napi_status napi_set_property_with_callsite_info(
    napi_env env, napi_value object, napi_value key, napi_value value,
    napi_callsite_info info, bool* hit);

napi_value getPropertyNames(napi_env env, napi_value value)
{
    napi_value names;
    napi_status status = napi_get_property_names(env, value, &names);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to get property names", return nullptr);
    return names;
}

napi_value getNamedProperty(napi_env env, napi_value obj, const char *key)
{
    napi_value result;
    napi_status status = napi_get_named_property(env, obj, key, &result);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to get property", return nullptr);
    return result;
}

napi_value getProperty(napi_env env, napi_value obj, napi_value key)
{
    napi_value result = nullptr;
    napi_status status = napi_get_property(env, obj, key, &result);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to get property", return nullptr);
    return result;
}

void setNamedProperty(napi_env env, napi_value obj, const char *key, napi_value value)
{
    napi_status status = napi_set_named_property(env, obj, key, value);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to set property", return);
}

// ---- IC bridge implementations ----

bool hasCallsiteInfoSupport() {
    return napi_create_callsite_info != nullptr
        && napi_delete_callsite_info != nullptr
        && napi_get_property_with_callsite_info != nullptr
        && napi_set_property_with_callsite_info != nullptr;
}

napi_callsite_info createCallsiteInfo(napi_env env) {
    napi_callsite_info info = nullptr;
    napi_status status = napi_create_callsite_info(env, &info);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to create callsite_info", return nullptr);
    return info;
}

void deleteCallsiteInfo(napi_env env, napi_callsite_info info) {
    if (info == nullptr) {
        return;
    }
    napi_status status = napi_delete_callsite_info(env, info);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to delete callsite_info", return);
}

napi_value getPropertyWithCallsiteInfo(napi_env env, napi_value obj, napi_value key, napi_callsite_info info) {
    if (napi_get_property_with_callsite_info == nullptr) {
        // 符号不存在时安全退化到 napi_get_property
        return getProperty(env, obj, key);
    }
    napi_value result = nullptr;
    napi_status status = napi_get_property_with_callsite_info(env, obj, key, info, &result, nullptr);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to get property (IC)", return nullptr);
    return result;
}

void setPropertyWithCallsiteInfo(napi_env env, napi_value obj, napi_value key,
                                 napi_value value, napi_callsite_info info) {
    if (napi_set_property_with_callsite_info == nullptr) {
        // 符号不存在时安全退化到 napi_set_property
        napi_status status = napi_set_property(env, obj, key, value);
        NAPI_CHECK(env, status, "propertyCpp", "Failed to set property (fallback)", return);
        return;
    }
    napi_status status = napi_set_property_with_callsite_info(env, obj, key, value, info, nullptr);
    NAPI_CHECK(env, status, "propertyCpp", "Failed to set property (IC)", return);
}
