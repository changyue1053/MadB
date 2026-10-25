#ifndef C07_INDEPENDENT_PROVIDER_H
#define C07_INDEPENDENT_PROVIDER_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#define CP_NOEXCEPT noexcept
#else
#define CP_NOEXCEPT
#endif
typedef struct cp_context cp_context;
typedef struct cp_counts {
    size_t attempts, allocated, released, live, bad_release;
} cp_counts;
typedef void (*cp_observer)(void *, int);
cp_context *cp_create(void) CP_NOEXCEPT;
void cp_delete(cp_context *) CP_NOEXCEPT;
void cp_hook(cp_context *, cp_observer, void *) CP_NOEXCEPT;
void cp_fail_after(cp_context *, size_t successful_attempts) CP_NOEXCEPT;
void *cp_clone(cp_context *, int64_t, const unsigned char *, size_t) CP_NOEXCEPT;
void cp_destroy(cp_context *, void *) CP_NOEXCEPT;
cp_counts cp_read(cp_context *) CP_NOEXCEPT;
int cp_value(const void *, int64_t *, const unsigned char **, size_t *) CP_NOEXCEPT;
int cp_apply(void *) CP_NOEXCEPT;
#ifdef __cplusplus
}
#endif
#endif
