#ifndef MADB_KN_TEST_NAPI_OBSERVER_H
#define MADB_KN_TEST_NAPI_OBSERVER_H
#include "observer_provider.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Test-only pure native functions: no NAPI, allocation or business admission. */
uint64_t madb_kn_bootstrap_napi_metric(int index);
typedef struct madb_kn_napi_entry {
    void *env;
    uintptr_t thread;
    struct madb_kn_napi_entry *previous;
    struct madb_kn_napi_entry *next;
} madb_kn_napi_entry;
void madb_kn_bootstrap_napi_enter(madb_kn_napi_entry *entry, void *env);
void madb_kn_bootstrap_napi_leave(madb_kn_napi_entry *entry);
#ifdef __cplusplus
}
#endif
#endif
