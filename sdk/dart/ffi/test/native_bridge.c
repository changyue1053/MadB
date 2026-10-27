/* Runtime test bridge: only the candidate's public C ABI is consumed. */
#include "madb_rust_client.h"
#include <dlfcn.h>
#include <stddef.h>

const char *madb_dart_native_path(uint32_t index) {
    const void *functions[] = {
        (const void *)madb_fixture_bootstrap, (const void *)madb_fixture_create,
        (const void *)madb_fixture_checksum, (const void *)madb_fixture_request_close,
        (const void *)madb_fixture_await_closed, (const void *)madb_fixture_release_handle
    };
    Dl_info info;
    if (index >= 6 || dladdr(functions[index], &info) == 0) return NULL;
    return info.dli_fname;
}
MadbStatus madb_dart_bootstrap(uint8_t *output) {
    return madb_fixture_bootstrap(1, output, 64);
}
void madb_dart_create_checked(MadbCheckedEngineCreateResult *output) {
    *output = madb_engine_create_checked();
}
MadbStatus madb_dart_checksum(MadbHandle handle, const uint8_t *data, uint64_t count,
        uint64_t *output, MadbError *error) {
    return madb_fixture_checksum(handle, data, count, output, error);
}
MadbStatus madb_dart_request_close(MadbHandle handle, MadbError *error) {
    return madb_fixture_request_close(handle, error);
}
MadbStatus madb_dart_await_closed(MadbHandle handle, uint32_t timeout, MadbError *error) {
    return madb_fixture_await_closed(handle, timeout, error);
}
MadbStatus madb_dart_release_handle(MadbHandle handle, MadbError *error) {
    return madb_fixture_release_handle(handle, error);
}
