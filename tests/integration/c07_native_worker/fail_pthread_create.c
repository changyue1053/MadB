#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>
static _Atomic int attempted = 0;
static int fail_create(pthread_t *thread, const pthread_attr_t *attr, void *(*body)(void *),
                       void *context) {
    (void)thread;
    (void)attr;
    (void)body;
    (void)context;
    atomic_fetch_add(&attempted, 1);
    return EAGAIN;
}
int probe_pthread_create_failures(void) { return atomic_load(&attempted); }
#if defined(__APPLE__)
__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} interpose __attribute__((section("__DATA,__interpose"))) = {(const void *)fail_create,
                                                              (const void *)pthread_create};

#elif defined(__linux__)
/* Process-local LD_PRELOAD replacement; never installed into the host system. */
int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*body)(void *),
                   void *context) {
    return fail_create(thread, attr, body, context);
}
#else
#error "pthread failure injection needs a reviewed platform interposition mechanism"
#endif
