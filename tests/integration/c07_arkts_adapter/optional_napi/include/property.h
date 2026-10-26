#pragma once
#include <cstddef>
using napi_env=void*; using napi_value=void*; using napi_callsite_info=void*;
enum napi_status { napi_ok=0, napi_generic_failure=9, napi_pending_exception=10 };
extern "C" {
napi_status napi_create_callsite_info(napi_env,napi_callsite_info*);
napi_status napi_delete_callsite_info(napi_env,napi_callsite_info);
napi_status napi_get_property_with_callsite_info(napi_env,napi_value,napi_value,napi_callsite_info,napi_value*,bool*);
napi_status napi_set_property_with_callsite_info(napi_env,napi_value,napi_value,napi_value,napi_callsite_info,bool*);
napi_status napi_get_property_names(napi_env,napi_value,napi_value*);
napi_status napi_get_named_property(napi_env,napi_value,const char*,napi_value*);
napi_status napi_get_property(napi_env,napi_value,napi_value,napi_value*);
napi_status napi_set_named_property(napi_env,napi_value,const char*,napi_value);
napi_status napi_set_property(napi_env,napi_value,napi_value,napi_value);
}
bool hasCallsiteInfoSupport();
napi_callsite_info createCallsiteInfo(napi_env);
void deleteCallsiteInfo(napi_env,napi_callsite_info);
napi_value getPropertyWithCallsiteInfo(napi_env,napi_value,napi_value,napi_callsite_info);
void setPropertyWithCallsiteInfo(napi_env,napi_value,napi_value,napi_value,napi_callsite_info);
