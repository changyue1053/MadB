#ifndef REVIEW_NODE_API_H
#define REVIEW_NODE_API_H
#ifdef __cplusplus
extern "C" {
#endif
typedef struct fake_env *napi_env;
typedef struct fake_hook *napi_async_cleanup_hook_handle;
typedef enum { napi_ok=0, napi_generic_failure=9 } napi_status;
typedef void (*napi_async_cleanup_hook)(napi_async_cleanup_hook_handle,void*);
struct uv_loop_s;
napi_status napi_get_uv_event_loop(napi_env,struct uv_loop_s**);
napi_status napi_add_async_cleanup_hook(napi_env,napi_async_cleanup_hook,void*,napi_async_cleanup_hook_handle*);
napi_status napi_remove_async_cleanup_hook(napi_async_cleanup_hook_handle);
#ifdef __cplusplus
}
#endif
#endif
