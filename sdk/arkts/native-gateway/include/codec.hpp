#pragma once
#include "handle.hpp"
#include "identity.generated.hpp"
#include "protocol.hpp"
#include <array>
#include <cmath>
#include <napi/native_api.h>
#include <string>
namespace gateway {
inline std::string encode(const Handle &h) {
    return encodeBoundHandle(h, GatewayRouteIdentity);
}
inline napi_value error(napi_env env, const char *code) {
    napi_throw_error(env, code, code);
    return nullptr;
}
inline const char *statusName(madb::c07::Status s) {
    using S = madb::c07::Status;
    switch (s) {
    case S::Ok:
        return "Ok";
    case S::InvalidArgument:
        return "InvalidArgument";
    case S::ResourceExhausted:
        return "ResourceExhausted";
    case S::Closing:
        return "Closing";
    case S::Closed:
        return "Closed";
    case S::Cancelled:
        return "Cancelled";
    case S::AlreadyConsumed:
        return "AlreadyConsumed";
    case S::NotReady:
        return "NotReady";
    case S::DomainFailure:
        return "DomainOverflow";
    case S::CallbackFailure:
        return "CallbackFailure";
    case S::InternalFailure:
        return "InternalFailure";
    }
    return "InternalFailure";
}
inline napi_value error(napi_env env, madb::c07::Status s) {
    return error(env, statusName(s));
}
inline bool arguments(napi_env env, napi_callback_info info, size_t wanted, napi_value *values) {
    size_t count = wanted;
    if (napi_get_cb_info(env, info, &count, values, nullptr, nullptr) != napi_ok ||
        count != wanted) {
        error(env, "InvalidArgument");
        return false;
    }
    return true;
}
inline napi_value undefined(napi_env env) {
    napi_value v = nullptr;
    if (napi_get_undefined(env, &v) != napi_ok)
        return error(env, "InternalFailure");
    return v;
}
inline napi_value string(napi_env env, const std::string &s) {
    napi_value v = nullptr;
    if (napi_create_string_utf8(env, s.data(), s.size(), &v) != napi_ok)
        return error(env, "InternalFailure");
    return v;
}
inline bool handle(napi_env env, napi_value value, Handle &out) {
    // Fixed route names (through rusttest) + identity + decimal IDs fit 288 bytes.
    std::array<char, 288> bytes{};
    size_t size = 0, length = 0;
    if (napi_get_value_string_utf8(env, value, nullptr, 0, &length) != napi_ok ||
        length >= bytes.size() ||
        napi_get_value_string_utf8(env, value, bytes.data(), bytes.size(), &size) != napi_ok ||
        size != length ||
        !parseBoundHandle(std::string_view(bytes.data(), size), GatewayRouteIdentity, out)) {
        error(env, "InvalidHandle");
        return false;
    }
    return true;
}
struct Input {
    int32_t value = 0;
    std::string text;
    madb::c07::Snapshot view() const {
        return {value, reinterpret_cast<const unsigned char *>(text.data()), text.size()};
    }
};
inline bool snapshot(napi_env env, napi_value value, napi_value text, Input &out) {
    double number = 0;
    size_t length = 0, copied = 0;
    std::array<char16_t, 1025> chars{};
    if (napi_get_value_double(env, value, &number) != napi_ok || !std::isfinite(number) ||
        std::trunc(number) != number || number < INT32_MIN || number > INT32_MAX ||
        napi_get_value_string_utf16(env, text, nullptr, 0, &length) != napi_ok || length > 1024 ||
        napi_get_value_string_utf16(env, text, chars.data(), chars.size(), &copied) != napi_ok ||
        copied != length) {
        error(env, "InvalidArgument");
        return false;
    }
    out.value = static_cast<int32_t>(number);
    out.text.clear();
    out.text.reserve(1024);
    for (size_t i = 0; i < length; ++i) {
        uint32_t c = chars[i];
        if (c >= 0xD800 && c <= 0xDBFF) {
            if (++i >= length || chars[i] < 0xDC00 || chars[i] > 0xDFFF) {
                error(env, "InvalidArgument");
                return false;
            }
            c = 0x10000 + ((c - 0xD800) << 10) + (chars[i] - 0xDC00);
        } else if (c >= 0xDC00 && c <= 0xDFFF) {
            error(env, "InvalidArgument");
            return false;
        }
        size_t needed = c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
        if (out.text.size() + needed > 1024) {
            error(env, "InvalidArgument");
            return false;
        }
        if (needed == 1)
            out.text.push_back(static_cast<char>(c));
        else {
            if (needed == 2)
                out.text.push_back(static_cast<char>(0xC0 | (c >> 6)));
            else {
                if (needed == 3)
                    out.text.push_back(static_cast<char>(0xE0 | (c >> 12)));
                else {
                    out.text.push_back(static_cast<char>(0xF0 | (c >> 18)));
                    out.text.push_back(static_cast<char>(0x80 | ((c >> 12) & 63)));
                }
                out.text.push_back(static_cast<char>(0x80 | ((c >> 6) & 63)));
            }
            out.text.push_back(static_cast<char>(0x80 | (c & 63)));
        }
    }
    return true;
}
inline napi_status copy(napi_env env, madb::c07::Status status, int32_t number,
                        const unsigned char *text, size_t size, napi_value &result) {
    napi_value code = nullptr, value = nullptr, content = nullptr;
    auto s = napi_create_object(env, &result);
    if (s == napi_ok)
        s = napi_create_int32(env, static_cast<int32_t>(status), &code);
    if (s == napi_ok)
        s = napi_create_int32(env, number, &value);
    if (s == napi_ok)
        s = napi_create_string_utf8(env, reinterpret_cast<const char *>(text), size, &content);
    napi_property_descriptor fields[] = {
        {"status", nullptr, nullptr, nullptr, nullptr, code, napi_enumerable, nullptr},
        {"value", nullptr, nullptr, nullptr, nullptr, value, napi_enumerable, nullptr},
        {"text", nullptr, nullptr, nullptr, nullptr, content, napi_enumerable, nullptr}};
    if (s == napi_ok)
        s = napi_define_properties(env, result, 3, fields);
    return s;
}
} // namespace gateway
