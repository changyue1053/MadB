#include "memory.hpp"
#include "runtime.hpp"
#include <algorithm>
namespace gateway {
namespace {
thread_local MemoryInputScope *inputScope = nullptr;
}
void MemoryState::reset() noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    data_ = {};
    data_.values[static_cast<size_t>(MemoryField::Format)] = 1;
}
MemorySnapshot MemoryState::snapshot() const noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    auto out = data_;
    out.values[static_cast<size_t>(MemoryField::Format)] = 1;
    return out;
}
void MemoryState::add(MemoryField field, uint64_t bytes) noexcept {
    auto &value = data_.values[static_cast<size_t>(field)];
    if (bytes > Limit || static_cast<uint64_t>(value) > Limit - bytes)
        data_.valid = false;
    else
        value += static_cast<int64_t>(bytes);
}
void MemoryState::subtract(MemoryField field, uint64_t bytes) noexcept {
    auto &value = data_.values[static_cast<size_t>(field)];
    if (bytes > static_cast<uint64_t>(value))
        data_.valid = false;
    else
        value -= static_cast<int64_t>(bytes);
}
void MemoryState::peak(MemoryField current, MemoryField maximum) noexcept {
    auto &value = data_.values[static_cast<size_t>(maximum)];
    value = std::max(value, data_.values[static_cast<size_t>(current)]);
}
void MemoryState::layout(MemoryField field, uint64_t bytes) noexcept {
    auto &value = data_.values[static_cast<size_t>(field)];
    if (!bytes || bytes > Limit || (value && static_cast<uint64_t>(value) != bytes))
        data_.valid = false;
    else
        value = static_cast<int64_t>(bytes);
}
void MemoryState::invalidate() noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    data_.valid = false;
}
void MemoryState::provider(uint64_t object, uint64_t text, uint32_t action) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    if (action == 0) {
        if (text)
            data_.valid = false;
        layout(MemoryField::ModelObjectSize, object);
        return;
    }
    if (object > Limit || text > Limit - object) {
        data_.valid = false;
        return;
    }
    if (action == 1) {
        add(MemoryField::ProviderObjectCurrent, object);
        add(MemoryField::ProviderTextCurrent, text);
        add(MemoryField::ProviderStorageAcquired, object + text);
        peak(MemoryField::ProviderObjectCurrent, MemoryField::ProviderObjectPeak);
        peak(MemoryField::ProviderTextCurrent, MemoryField::ProviderTextPeak);
        const auto total = data_.values[static_cast<size_t>(MemoryField::ProviderObjectCurrent)] +
                           data_.values[static_cast<size_t>(MemoryField::ProviderTextCurrent)];
        if (total > static_cast<int64_t>(Limit))
            data_.valid = false;
        auto &maximum = data_.values[static_cast<size_t>(MemoryField::ProviderStoragePeak)];
        maximum = std::max(maximum, total);
    } else if (action == 2) {
        subtract(MemoryField::ProviderObjectCurrent, object);
        subtract(MemoryField::ProviderTextCurrent, text);
        add(MemoryField::ProviderStorageReleased, object + text);
    } else
        data_.valid = false;
}
void MemoryState::ledger(uint64_t bytes) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    if (bytes > 4096) {
        data_.valid = false;
        return;
    }
    data_.values[static_cast<size_t>(MemoryField::CoreRetainedCurrent)] =
        static_cast<int64_t>(bytes);
    peak(MemoryField::CoreRetainedCurrent, MemoryField::CoreRetainedPeak);
}
void MemoryState::staging(uint64_t bytes, bool acquire) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    if (acquire)
        add(MemoryField::PublicationStagingCurrent, bytes);
    else
        subtract(MemoryField::PublicationStagingCurrent, bytes);
    peak(MemoryField::PublicationStagingCurrent, MemoryField::PublicationStagingPeak);
}
void MemoryState::input(uint64_t bytes, uint64_t backing, bool acquire) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    layout(MemoryField::InputObjectSize, bytes);
    if (acquire) {
        add(MemoryField::InputObjectCurrent, bytes);
        add(MemoryField::InputScopesEntered, 1);
    } else {
        subtract(MemoryField::InputObjectCurrent, bytes);
        add(MemoryField::InputScopesExited, 1);
        subtract(MemoryField::Utf8BackingCurrent, backing);
        add(MemoryField::Utf8BackingReleased, backing);
    }
    peak(MemoryField::InputObjectCurrent, MemoryField::InputObjectPeak);
}
void MemoryState::utf16(uint64_t bytes, bool acquire) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    layout(MemoryField::Utf16ArraySize, bytes);
    if (acquire) {
        add(MemoryField::Utf16ArrayCurrent, bytes);
        add(MemoryField::Utf16ScopesEntered, 1);
    } else {
        subtract(MemoryField::Utf16ArrayCurrent, bytes);
        add(MemoryField::Utf16ScopesExited, 1);
    }
    peak(MemoryField::Utf16ArrayCurrent, MemoryField::Utf16ArrayPeak);
}
void MemoryState::backing(uint64_t bytes) noexcept {
    std::lock_guard<std::mutex> lock(mutex_);
    add(MemoryField::Utf8BackingCurrent, bytes);
    add(MemoryField::Utf8BackingAcquired, bytes);
    peak(MemoryField::Utf8BackingCurrent, MemoryField::Utf8BackingPeak);
}
void memoryProviderNotify(void *context, uint64_t object, uint64_t text, uint32_t action) noexcept {
    static_cast<MemoryState *>(context)->provider(object, text, action);
}
void memoryCoreLedger(void *context, uint64_t bytes) noexcept {
    static_cast<Runtime *>(context)->audit.memory.ledger(bytes);
}
void memoryCoreStaging(void *context, uint64_t bytes, bool acquire) noexcept {
    static_cast<Runtime *>(context)->audit.memory.staging(bytes, acquire);
}
size_t externalStringBytes(const std::string &text) noexcept {
    const auto first = reinterpret_cast<uintptr_t>(&text),
               data = reinterpret_cast<uintptr_t>(text.data());
    return data >= first && data - first < sizeof(text) ? 0 : text.capacity() + 1;
}
MemoryInputScope::MemoryInputScope(MemoryState &value, uint64_t size) noexcept
    : state(value), previous_(inputScope), inputSize_(size) {
    inputScope = this;
    state.input(size, 0, true);
}
MemoryInputScope::~MemoryInputScope() {
    // Declared before Input: the original string is destroyed before this retires.
    state.input(inputSize_, backing_, false);
    inputScope = previous_;
}
MemoryInputScope *MemoryInputScope::current() noexcept {
    return inputScope;
}
void MemoryInputScope::backing(const std::string &text) noexcept {
    const auto bytes = externalStringBytes(text);
    if (backing_) {
        state.invalidate();
        return;
    }
    backing_ = bytes;
    state.backing(bytes);
}
MemoryUtf16Scope::MemoryUtf16Scope(uint64_t bytes) noexcept
    : state_(inputScope ? &inputScope->state : nullptr), bytes_(bytes) {
    if (state_)
        state_->utf16(bytes, true);
}
MemoryUtf16Scope::~MemoryUtf16Scope() {
    if (state_)
        state_->utf16(bytes_, false);
}
} // namespace gateway
