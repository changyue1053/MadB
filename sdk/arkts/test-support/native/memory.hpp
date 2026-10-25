#pragma once
#include "memory_hook.h"
#include <array>
#include <cstdint>
#include <mutex>
#include <string>
namespace gateway {
enum class MemoryField : size_t {
    Format,
    CoreRetainedCurrent,
    CoreRetainedPeak,
    PublicationStagingCurrent,
    PublicationStagingPeak,
    ProviderObjectCurrent,
    ProviderObjectPeak,
    ProviderTextCurrent,
    ProviderTextPeak,
    ProviderStoragePeak,
    ProviderStorageAcquired,
    ProviderStorageReleased,
    Utf8BackingCurrent,
    Utf8BackingPeak,
    Utf8BackingAcquired,
    Utf8BackingReleased,
    InputObjectCurrent,
    InputObjectPeak,
    Utf16ArrayCurrent,
    Utf16ArrayPeak,
    InputScopesEntered,
    InputScopesExited,
    Utf16ScopesEntered,
    Utf16ScopesExited,
    ModelObjectSize,
    InputObjectSize,
    Utf16ArraySize,
    Count
};
using MemoryValues = std::array<int64_t, static_cast<size_t>(MemoryField::Count)>;
struct MemorySnapshot {
    MemoryValues values{};
    bool valid = true;
};
class MemoryState {
  public:
    void reset() noexcept;
    MemorySnapshot snapshot() const noexcept;
    void provider(uint64_t object, uint64_t text, uint32_t action) noexcept;
    void ledger(uint64_t bytes) noexcept;
    void staging(uint64_t bytes, bool acquire) noexcept;
    void input(uint64_t bytes, uint64_t backing, bool acquire) noexcept;
    void utf16(uint64_t bytes, bool acquire) noexcept;
    void backing(uint64_t bytes) noexcept;
    void invalidate() noexcept;

  private:
    static constexpr uint64_t Limit = 9007199254740991ULL;
    mutable std::mutex mutex_;
    MemorySnapshot data_;
    void add(MemoryField, uint64_t) noexcept;
    void subtract(MemoryField, uint64_t) noexcept;
    void peak(MemoryField current, MemoryField maximum) noexcept;
    void layout(MemoryField, uint64_t) noexcept;
};
void memoryProviderNotify(void *, uint64_t, uint64_t, uint32_t) noexcept;
// Original Core callback context is the existing testProvider Runtime pointer.
void memoryCoreLedger(void *, uint64_t) noexcept;
void memoryCoreStaging(void *, uint64_t, bool) noexcept;
size_t externalStringBytes(const std::string &) noexcept;
class MemoryInputScope {
  public:
    MemoryInputScope(MemoryState &, uint64_t inputSize) noexcept;
    ~MemoryInputScope();
    static MemoryInputScope *current() noexcept;
    void backing(const std::string &) noexcept;
    MemoryState &state;

  private:
    MemoryInputScope *previous_;
    uint64_t inputSize_, backing_ = 0;
};
class MemoryUtf16Scope {
  public:
    explicit MemoryUtf16Scope(uint64_t bytes) noexcept;
    ~MemoryUtf16Scope();

  private:
    MemoryState *state_;
    uint64_t bytes_;
};
} // namespace gateway
