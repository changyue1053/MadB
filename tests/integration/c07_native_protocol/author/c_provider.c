#include "c_provider.h"
#include <limits.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
static void require(int condition) {
    if (!condition)
        abort();
}
struct c_payload {
    int32_t value;
    size_t size;
    unsigned char text[];
};
struct c_provider {
    _Atomic size_t calls, allocations, frees, live, fail_at;
    void (*on_clone)(void *);
    void (*on_destroy)(void *);
    void *hook_context;
};
struct c_provider *c_provider_create(void) {
    struct c_provider *p = calloc(1, sizeof(*p));
    require(p != NULL);
    return p;
}
void c_provider_drop(struct c_provider *p) {
    require(atomic_load(&p->live) == 0);
    free(p);
}
void c_provider_fail_at(struct c_provider *p, size_t call) { atomic_store(&p->fail_at, call); }
void c_provider_hooks(struct c_provider *p, void (*clone)(void *), void (*destroy)(void *),
                      void *ctx) {
    p->on_clone = clone;
    p->on_destroy = destroy;
    p->hook_context = ctx;
}
void *c_provider_clone(struct c_provider *p, int64_t value, const unsigned char *text,
                       size_t size) {
    size_t call = atomic_fetch_add(&p->calls, 1) + 1;
    if (p->on_clone)
        p->on_clone(p->hook_context);
    if (atomic_load(&p->fail_at) == call)
        return NULL;
    struct c_payload *result = malloc(sizeof(*result) + size);
    if (!result)
        return NULL;
    result->value = (int32_t)value;
    result->size = size;
    if (size)
        memcpy(result->text, text, size);
    atomic_fetch_add(&p->allocations, 1);
    atomic_fetch_add(&p->live, 1);
    return result;
}
void c_provider_destroy(struct c_provider *p, void *ptr) {
    require(ptr != NULL);
    if (p->on_destroy)
        p->on_destroy(p->hook_context);
    free(ptr);
    atomic_fetch_add(&p->frees, 1);
    require(atomic_fetch_sub(&p->live, 1) > 0);
}
int c_provider_increment(void *ptr) {
    struct c_payload *p = ptr;
    if (p->value == INT32_MAX)
        return 0;
    ++p->value;
    return 1;
}
int32_t c_payload_value(const void *p) { return ((const struct c_payload *)p)->value; }
size_t c_payload_size(const void *p) { return ((const struct c_payload *)p)->size; }
const unsigned char *c_payload_text(const void *p) { return ((const struct c_payload *)p)->text; }
size_t c_provider_calls(const struct c_provider *p) { return atomic_load(&p->calls); }
size_t c_provider_allocations(const struct c_provider *p) { return atomic_load(&p->allocations); }
size_t c_provider_frees(const struct c_provider *p) { return atomic_load(&p->frees); }
size_t c_provider_live(const struct c_provider *p) { return atomic_load(&p->live); }
