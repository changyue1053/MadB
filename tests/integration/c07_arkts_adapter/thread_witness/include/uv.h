#ifndef REVIEW_UV_H
#define REVIEW_UV_H
#ifdef __cplusplus
extern "C" {
#endif
typedef struct uv_loop_s { int unused; } uv_loop_t;
typedef struct uv_work_s { void *data; } uv_work_t;
typedef void (*uv_work_cb)(uv_work_t*);
typedef void (*uv_after_work_cb)(uv_work_t*,int);
int uv_queue_work(uv_loop_t*,uv_work_t*,uv_work_cb,uv_after_work_cb);
#ifdef __cplusplus
}
#endif
#endif
