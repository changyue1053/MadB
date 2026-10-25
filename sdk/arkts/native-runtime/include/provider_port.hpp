#pragma once
#include "protocol.hpp"
#include "provider_abi.h"
#include <cstdlib>
namespace madb::c07::probe {
// Private assembly port. The provider table, context and DSO outlive all Core
// tokens; callers only pass a complete successful factory result. No C/Rust
// model layout or domain algorithm is known by this adapter.
inline Provider provider_port(mp_provider &table) noexcept {
    return {&table,
            [](void *context, Snapshot snapshot) noexcept -> CloneResult {
                auto &provider = *static_cast<mp_provider *>(context);
                void *owned = nullptr;
                auto status = provider.clone(
                    provider.context,
                    {static_cast<int32_t>(snapshot.value), snapshot.text, snapshot.size}, &owned);
                CloneStatus mapped = CloneStatus::InternalFailure;
                switch (status) {
                case MP_OK:
                    mapped = CloneStatus::Ok;
                    break;
                case MP_INVALID_ARGUMENT:
                    mapped = CloneStatus::InvalidArgument;
                    break;
                case MP_ALLOCATION_FAILURE:
                    mapped = CloneStatus::AllocationFailure;
                    break;
                case MP_INTERNAL_FAILURE:
                    break;
                default:
                    break;
                }
                // Even an error-owned value is handed to Core's cleanup guard.
                return {mapped, owned};
            },
            [](void *context, void *owned) noexcept {
                auto &provider = *static_cast<mp_provider *>(context);
                // A genuine model's declared paired release cannot fail. Failure
                // here is a broken ownership contract, never a released permit.
                if (provider.release(provider.context, owned) != MP_OK)
                    std::abort();
            }};
}
} // namespace madb::c07::probe
