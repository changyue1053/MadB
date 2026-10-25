#ifndef MADB_KN_TEST_PROVIDER_NAPI_OBSERVER_H
#define MADB_KN_TEST_PROVIDER_NAPI_OBSERVER_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
uint64_t madb_kn_provider_napi_metric(int index);
void madb_kn_provider_napi_bind(void *env);
#ifdef __cplusplus
}
#endif
#endif
