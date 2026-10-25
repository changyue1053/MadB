#pragma once
#include "provider_abi.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Test-only non-owning accounting sink. The provider/context lifetime already
 * lies inside its Runtime/Audit lifetime; this callback creates no new owner.
 * action0 reports sizeof(Model); action1 acquires actual object/backing storage;
 * action2 follows real destruction. Both byte components update one event. */
typedef struct {
    void *context;
    void (*notify)(void *, uint64_t object_bytes, uint64_t text_bytes, uint32_t action);
} mp_test_memory_observer;
mp_status mp_c_test_memory_bind(void *, mp_test_memory_observer) MP_NOEXCEPT;
mp_status mp_cpp_test_memory_bind(void *, mp_test_memory_observer) MP_NOEXCEPT;
mp_status mp_rust_test_memory_bind(void *, mp_test_memory_observer) MP_NOEXCEPT;
#ifdef __cplusplus
}
#endif
