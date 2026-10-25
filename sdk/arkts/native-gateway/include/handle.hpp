#pragma once
#include <cstdint>
#include <limits>
#include <string>
#include <string_view>
namespace gateway {
struct Handle {
    uint64_t runtime = 0, generation = 0, id = 0;
    char kind = 'r';
};
inline bool decimal(std::string_view text, size_t &offset, uint64_t &out) {
    const auto start = offset;
    out = 0;
    if (offset == text.size() || text[offset] < '1' || text[offset] > '9')
        return false;
    while (offset < text.size() && text[offset] >= '0' && text[offset] <= '9') {
        const auto digit = static_cast<unsigned>(text[offset++] - '0');
        if (out > (std::numeric_limits<uint64_t>::max() - digit) / 10)
            return false;
        out = out * 10 + digit;
    }
    return offset > start;
}
inline bool parseBoundHandle(std::string_view text, std::string_view identity, Handle &out) {
    if (text.size() <= identity.size() || text.substr(0, identity.size()) != identity ||
        text[identity.size()] != '/')
        return false;
    text.remove_prefix(identity.size() + 1);
    Handle h;
    size_t p = 0;
    if (text.empty() || text[p++] != 'r' || !decimal(text, p, h.runtime))
        return false;
    if (p == text.size()) {
        out = h;
        return true;
    }
    if (text.substr(p, 2) != "/g")
        return false;
    p += 2;
    if (!decimal(text, p, h.generation) || p >= text.size() || text[p++] != '/')
        return false;
    if (p >= text.size())
        return false;
    h.kind = text[p++];
    if (h.kind != 'e' && h.kind != 'o' && h.kind != 's' && h.kind != 'w')
        return false;
    if (h.kind == 'e') {
        if (p != text.size())
            return false;
    } else if (!decimal(text, p, h.id) || p != text.size())
        return false;
    out = h;
    return true;
}
inline std::string encodeBoundHandle(const Handle &h, std::string_view identity) {
    std::string s = std::string(identity) + "/r" + std::to_string(h.runtime);
    if (h.kind == 'r')
        return s;
    s += "/g" + std::to_string(h.generation) + "/" + h.kind;
    if (h.kind != 'e')
        s += std::to_string(h.id);
    return s;
}
} // namespace gateway
