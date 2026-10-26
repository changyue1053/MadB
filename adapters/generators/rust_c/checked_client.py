"""Emit the finite C constructor from the same generated native identity."""
import re

from support import NativeError


def render_checked_client(identity):
    if identity.get("schema") != "madb.native-identity/1" or identity.get("bootstrap_version") != 1:
        raise NativeError("NATIVE001", "checked constructor requires native identity v1")
    expected = b""
    for field in ("interface_digest", "profile_digest"):
        value = identity.get(field)
        if not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None:
            raise NativeError("NATIVE001", "checked constructor requires a SHA-256 identity: " + field)
        expected += bytes.fromhex(value[7:])
    values = ", ".join(str(value) for value in expected)
    return """/* Generated from this SDK's native identity. Do not edit. */
#ifndef MADB_RUST_CLIENT_H
#define MADB_RUST_CLIENT_H
#include "madb_rust.h"

typedef uint32_t MadbCheckedEngineCreateKind;
enum {
    MADB_CHECKED_ENGINE_OK = 0,
    MADB_CHECKED_ENGINE_BOOTSTRAP_FAILED = 1,
    MADB_CHECKED_ENGINE_INTERFACE_MISMATCH = 2,
    MADB_CHECKED_ENGINE_PROFILE_MISMATCH = 3,
    MADB_CHECKED_ENGINE_CREATE_FAILED = 4
};
typedef struct MadbCheckedEngineCreateResult {
    MadbCheckedEngineCreateKind kind;
    MadbStatus native_status;
    MadbError native_error;
    MadbHandle handle;
} MadbCheckedEngineCreateResult;

/* A successful result transfers one handle. Close, wait, and release explicitly. */
static inline MadbCheckedEngineCreateResult madb_engine_create_checked(void) {
    static const uint8_t expected[64] = {""" + values + """};
    uint8_t actual[64] = {0};
    MadbCheckedEngineCreateResult result = {MADB_CHECKED_ENGINE_BOOTSTRAP_FAILED, 0, {0, 0}, 0};
    result.native_status = madb_fixture_bootstrap(1, actual, 64);
    if (result.native_status != MADB_OK) return result;
    for (uint32_t index = 0; index < 32; ++index) {
        if (actual[index] != expected[index]) {
            result.kind = MADB_CHECKED_ENGINE_INTERFACE_MISMATCH;
            return result;
        }
    }
    for (uint32_t index = 32; index < 64; ++index) {
        if (actual[index] != expected[index]) {
            result.kind = MADB_CHECKED_ENGINE_PROFILE_MISMATCH;
            return result;
        }
    }
    result.native_status = madb_fixture_create(&result.handle, &result.native_error);
    if (result.native_status != MADB_OK) {
        result.kind = MADB_CHECKED_ENGINE_CREATE_FAILED;
        result.handle = 0;
        return result;
    }
    result.kind = MADB_CHECKED_ENGINE_OK;
    return result;
}
#endif
"""
