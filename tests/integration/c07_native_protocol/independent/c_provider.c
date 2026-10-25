#include "c_provider.h"
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#define CP_MAX 8192
#define CP_MAGIC UINT64_C(0x4350524f56494445)
typedef struct cp_model {
    uint64_t magic;
    int64_t value;
    size_t size;
    unsigned char text[];
} cp_model;
typedef struct cp_entry {
    void *pointer;
    int live;
} cp_entry;
struct cp_context {
    pthread_mutex_t mutex;
    cp_counts counts;
    cp_entry entries[CP_MAX];
    size_t used, fail_attempt;
    cp_observer observer;
    void *observer_context;
};
cp_context *cp_create(void) {
    cp_context *c = calloc(1, sizeof(*c));
    if (c)
        pthread_mutex_init(&c->mutex, 0);
    return c;
}
void cp_delete(cp_context *c) {
    if (!c)
        return;
    for (size_t i = 0; i < c->used; i++)
        if (c->entries[i].live)
            free(c->entries[i].pointer);
    pthread_mutex_destroy(&c->mutex);
    free(c);
}
void cp_hook(cp_context *c, cp_observer f, void *arg) {
    pthread_mutex_lock(&c->mutex);
    c->observer = f;
    c->observer_context = arg;
    pthread_mutex_unlock(&c->mutex);
}
void cp_fail_after(cp_context *c, size_t successes) {
    pthread_mutex_lock(&c->mutex);
    c->fail_attempt = c->counts.attempts + successes + 1;
    pthread_mutex_unlock(&c->mutex);
}
void *cp_clone(cp_context *c, int64_t value, const unsigned char *bytes, size_t size) {
    pthread_mutex_lock(&c->mutex);
    size_t attempt = ++c->counts.attempts;
    int fail = attempt == c->fail_attempt;
    cp_observer observer = c->observer;
    void *arg = c->observer_context;
    pthread_mutex_unlock(&c->mutex);
    if (observer)
        observer(arg, 1);
    if (fail)
        return 0;
    cp_model *model = malloc(sizeof(*model) + size);
    if (!model)
        return 0;
    model->magic = CP_MAGIC;
    model->value = value;
    model->size = size;
    if (size)
        memcpy(model->text, bytes, size);
    pthread_mutex_lock(&c->mutex);
    if (c->used == CP_MAX) {
        pthread_mutex_unlock(&c->mutex);
        free(model);
        return 0;
    }
    c->entries[c->used++] = (cp_entry){model, 1};
    c->counts.allocated++;
    c->counts.live++;
    pthread_mutex_unlock(&c->mutex);
    return model;
}
void cp_destroy(cp_context *c, void *value) {
    pthread_mutex_lock(&c->mutex);
    size_t found = c->used;
    for (size_t i = 0; i < c->used; i++)
        if (c->entries[i].pointer == value && c->entries[i].live) {
            found = i;
            break;
        }
    if (found == c->used) {
        c->counts.bad_release++;
        pthread_mutex_unlock(&c->mutex);
        return;
    }
    c->entries[found].live = 0;
    cp_observer observer = c->observer;
    void *arg = c->observer_context;
    pthread_mutex_unlock(&c->mutex);
    if (observer)
        observer(arg, 2);
    cp_model *model = value;
    model->magic = 0;
    free(model);
    pthread_mutex_lock(&c->mutex);
    c->counts.released++;
    c->counts.live--;
    pthread_mutex_unlock(&c->mutex);
}
cp_counts cp_read(cp_context *c) {
    pthread_mutex_lock(&c->mutex);
    cp_counts result = c->counts;
    pthread_mutex_unlock(&c->mutex);
    return result;
}
int cp_value(const void *value, int64_t *number, const unsigned char **text, size_t *size) {
    const cp_model *m = value;
    if (!m || m->magic != CP_MAGIC)
        return 0;
    *number = m->value;
    *text = m->text;
    *size = m->size;
    return 1;
}
int cp_apply(void *value) {
    cp_model *m = value;
    if (!m || m->magic != CP_MAGIC)
        return 0;
    m->value += 7;
    return 1;
}
