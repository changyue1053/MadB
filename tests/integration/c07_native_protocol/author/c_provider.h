#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
struct c_provider;
struct c_provider *c_provider_create(void);
void c_provider_drop(struct c_provider *);
void c_provider_fail_at(struct c_provider *, size_t absolute_call);
void c_provider_hooks(struct c_provider *, void (*on_clone)(void *), void (*on_destroy)(void *),
                      void *);
void *c_provider_clone(struct c_provider *, int64_t value, const unsigned char *, size_t);
void c_provider_destroy(struct c_provider *, void *);
int c_provider_increment(void *);
int32_t c_payload_value(const void *);
size_t c_payload_size(const void *);
const unsigned char *c_payload_text(const void *);
size_t c_provider_calls(const struct c_provider *);
size_t c_provider_allocations(const struct c_provider *);
size_t c_provider_frees(const struct c_provider *);
size_t c_provider_live(const struct c_provider *);
#ifdef __cplusplus
}
#endif
