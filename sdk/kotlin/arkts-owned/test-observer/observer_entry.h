#ifndef MADB_KN_TEST_NAPI_OBSERVER_ENTRY_H
#define MADB_KN_TEST_NAPI_OBSERVER_ENTRY_H
#include "observer.h"
/* Stack-only scope for every actual bootstrap NAPI callback, including Init.
 * A Worker callback owns its own valid env; it is not the managed main owner. */
class MadbKnNapiEntry final {
public:
    explicit MadbKnNapiEntry(void *env) noexcept { madb_kn_bootstrap_napi_enter(&entry_, env); }
    ~MadbKnNapiEntry() { madb_kn_bootstrap_napi_leave(&entry_); }
    MadbKnNapiEntry(const MadbKnNapiEntry &) = delete;
    MadbKnNapiEntry &operator=(const MadbKnNapiEntry &) = delete;
private:
    madb_kn_napi_entry entry_{};
};
#endif
