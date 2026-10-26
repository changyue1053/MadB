"""Compile the generated helper against the actual header and finite C stubs."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

import test_reader as reader


class CheckedClient(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        reader.NativeReader.setUpClass.__func__(cls)

    def test_five_outcomes_call_counts_identity_priority_and_layout(self):
        from checked_client import render_checked_client
        from support import run_tool
        identity = dict(schema="madb.native-identity/1", bootstrap_version=1,
                        interface_digest="sha256:" + "01" * 32, profile_digest="sha256:" + "02" * 32)
        with tempfile.TemporaryDirectory(prefix="madb-l02-checked-") as temporary:
            root = Path(temporary)
            config = dict(tools=deepcopy(self.tools), environment=dict(self.environment))
            run_tool(config, "cbindgen", ["--config", reader.ROOT / "sdk/rust/engine/cbindgen.toml",
                reader.ROOT / "sdk/rust/engine/src/lib.rs", "--output", root / "madb_rust.h"], cwd=root)
            (root / "madb_rust_client.h").write_text(render_checked_client(identity))
            source = root / "probe.c"
            source.write_text(r'''#include "madb_rust_client.h"
#include <assert.h>
#include <stddef.h>
_Static_assert(sizeof(MadbCheckedEngineCreateResult) == 24, "result size");
_Static_assert(_Alignof(MadbCheckedEngineCreateResult) == 8, "result alignment");
_Static_assert(offsetof(MadbCheckedEngineCreateResult, kind) == 0, "kind offset");
_Static_assert(offsetof(MadbCheckedEngineCreateResult, native_status) == 4, "status offset");
_Static_assert(offsetof(MadbCheckedEngineCreateResult, native_error) == 8, "error offset");
_Static_assert(offsetof(MadbCheckedEngineCreateResult, handle) == 16, "handle offset");
static int mode, bootstrap_calls, create_calls;
MadbStatus madb_fixture_bootstrap(uint32_t version, uint8_t *out, uint64_t capacity) {
    ++bootstrap_calls;
    assert(version == 1 && capacity == 64);
    if (mode == 1) return MADB_UNSUPPORTED;
    for (uint32_t index = 0; index < 64; ++index) out[index] = index < 32 ? 1 : 2;
    if (mode == 2 || mode == 5) out[0] = 3;
    if (mode == 3 || mode == 5) out[32] = 3;
    return MADB_OK;
}
MadbStatus madb_fixture_create(MadbHandle *out, MadbError *error) {
    ++create_calls;
    *out = 0;
    if (mode == 4) { error->domain = 13; error->code = -17; return MADB_RESOURCE_EXHAUSTED; }
    error->domain = 0; error->code = 0; *out = 100 + (uint64_t)create_calls;
    return MADB_OK;
}
int main(void) {
    const uint32_t expected[6] = {0, 1, 2, 3, 4, 2};
    for (mode = 0; mode < 6; ++mode) {
        bootstrap_calls = create_calls = 0;
        MadbCheckedEngineCreateResult result = madb_engine_create_checked();
        assert(result.kind == expected[mode] && bootstrap_calls == 1);
        assert(create_calls == (mode == 0 || mode == 4));
        assert(result.native_status == (mode == 1 ? MADB_UNSUPPORTED : mode == 4 ? MADB_RESOURCE_EXHAUSTED : MADB_OK));
        assert(result.native_error.domain == (mode == 4 ? 13u : 0u));
        assert(result.native_error.code == (mode == 4 ? -17 : 0));
        assert(result.handle == (mode == 0 ? 101u : 0u));
    }
    mode = 0; bootstrap_calls = create_calls = 0;
    MadbCheckedEngineCreateResult first = madb_engine_create_checked();
    MadbCheckedEngineCreateResult second = madb_engine_create_checked();
    assert(first.handle == 101 && second.handle == 102 && bootstrap_calls == 2 && create_calls == 2);
    return 0;
}
''')
            binary = root / "probe"
            object_file = root / "probe.o"
            run_tool(config, "clang", [*self.cflags, "-c", source, "-o", object_file])
            run_tool(config, "clang", [*self.cflags, object_file, "-o", binary])
            reader.command([binary])

    def test_identity_fields_are_validated_and_embedded_deterministically(self):
        from checked_client import render_checked_client
        from support import NativeError
        identity = dict(schema="madb.native-identity/1", bootstrap_version=1,
                        interface_digest="sha256:" + "01" * 32, profile_digest="sha256:" + "02" * 32)
        self.assertEqual(render_checked_client(identity), render_checked_client(dict(identity)))
        for field, value in (("schema", "madb.native-identity/2"), ("bootstrap_version", 2),
                             ("interface_digest", "sha256:" + "g" * 64), ("profile_digest", "sha256:123")):
            with self.subTest(field=field), self.assertRaises(NativeError):
                render_checked_client(dict(identity, **{field: value}))


if __name__ == "__main__":
    unittest.main()
