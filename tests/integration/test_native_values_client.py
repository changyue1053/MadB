"""Independent C provider tests for the generated ordinary value wrapper.

These test protocol handling, not the Rust shim or a frozen SDK candidate.
"""
from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "adapters/generators/rust_c"))
from support import NativeError
from values_client import render_values_client


IDENTITY = {"schema": "madb.native-identity/1", "bootstrap_version": 1,
            "interface_digest": "sha256:" + "11" * 32, "profile_digest": "sha256:" + "22" * 32}
PLAN = {
    "namespace": "reading_demo",
    "records": [{"name": "Reading", "fields": [{"name": "value", "kind": "i32"},
                 {"name": "label", "kind": "String"}, {"name": "quality", "kind": "u32"}]},
                {"name": "Count", "fields": [{"name": "value", "kind": "u32"}]}],
    "errors": [{"name": "ReadingError", "variants": [{"name": "Overflow", "value": 1}]},
               {"name": "DifferentError", "variants": [{"name": "Missing", "value": 7}]}],
    "functions": [
        {"name": "normalize", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}], "error": None},
        {"name": "offset", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}, {"name": "delta", "kind": "i32"}], "error": "ReadingError"},
        {"name": "alternative", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}], "error": "DifferentError"},
        {"name": "count", "record": "Count", "parameters": [{"name": "value", "kind": "Count"}, {"name": "madb_value_result", "kind": "i32"}], "error": None},
    ],
}
BASE_HEADER = r"""
#ifndef TEST_VALUE_BASE_H
#define TEST_VALUE_BASE_H
#include <stdint.h>
typedef int32_t MadbValueStatus;
typedef uint32_t MadbValueCallKind;
typedef struct { uint32_t domain; int32_t code; } MadbValueError;
typedef struct { uint32_t byte_count; uint8_t bytes[64]; } MadbValueUtf8_64;
#define MADB_VALUE_CALL_OK 0u
#define MADB_VALUE_CALL_BOOTSTRAP_FAILED 1u
#define MADB_VALUE_CALL_INTERFACE_MISMATCH 2u
#define MADB_VALUE_CALL_PROFILE_MISMATCH 3u
#define MADB_VALUE_CALL_OPERATION_FAILED 4u
#define MADB_VALUE_STATUS_OK 0
#define MADB_VALUE_STATUS_INVALID_ARGUMENT 1
#define MADB_VALUE_STATUS_INTERNAL_FAILURE 8
#define MADB_VALUE_STATUS_RESOURCE_EXHAUSTED 9
#define MADB_VALUE_STATUS_DOMAIN_ERROR 10
#define MADB_VALUE_STATUS_UNSUPPORTED 11
#endif
"""
NATIVE_HEADER = r"""
#ifndef TEST_VALUE_NATIVE_H
#define TEST_VALUE_NATIVE_H
#include "madb_value_abi_v1.h"
typedef struct { int32_t value; MadbValueUtf8_64 label; uint32_t quality; } reading_demo_Reading;
typedef struct { uint32_t value; } reading_demo_Count;
#define reading_demo_ReadingError_Overflow 1
#define reading_demo_DifferentError_Missing 7
MadbValueStatus reading_demo_bootstrap_v1(uint32_t, uint8_t *, uint64_t);
MadbValueStatus reading_demo_raw_normalize(const reading_demo_Reading *, reading_demo_Reading *, MadbValueError *);
MadbValueStatus reading_demo_raw_offset(const reading_demo_Reading *, int32_t, reading_demo_Reading *, MadbValueError *);
MadbValueStatus reading_demo_raw_alternative(const reading_demo_Reading *, reading_demo_Reading *, MadbValueError *);
MadbValueStatus reading_demo_raw_count(const reading_demo_Count *, int32_t, reading_demo_Count *, MadbValueError *);
#endif
"""
PROGRAM = r"""
#include "reading_demo.h"
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>

static int boot_calls, raw_calls, boot_status, mismatch, raw_status, mode;
static uint32_t raw_domain;
static int32_t raw_code;
static reading_demo_Reading expected;

static void reset(void) {
    boot_calls = raw_calls = boot_status = mismatch = raw_status = mode = 0;
    raw_domain = 0u; raw_code = 0;
    memset(&expected, 0, sizeof expected);
    expected.value = 7; expected.quality = 42u;
    expected.label.byte_count = 6u;
    memcpy(expected.label.bytes, "\xe6\xb8\xa9\xe5\xba\xa6", 6);
}
MadbValueStatus reading_demo_bootstrap_v1(uint32_t version, uint8_t *identity, uint64_t capacity) {
    ++boot_calls;
    assert(version == 1u && capacity == 64u);
    for (unsigned i = 0; i < 64; ++i) assert(identity[i] == 0u);
    memset(identity, 0x11, 32);
    memset(identity + 32, 0x22, 32);
    if (mismatch & 1) identity[0] ^= 1;
    if (mismatch & 2) identity[32] ^= 1;
    return boot_status;
}
static MadbValueStatus provider(const reading_demo_Reading *input, reading_demo_Reading *output, MadbValueError *error) {
    ++raw_calls;
    assert(output->value == 0 && output->quality == 0u && output->label.byte_count == 0u);
    for (unsigned i = 0; i < 64; ++i) assert(output->label.bytes[i] == 0u);
    assert(error->domain == 0u && error->code == 0);
    assert(input->value == 7);
    if (raw_status == 0 || mode == 1) *output = expected;
    if (mode == 2) output->label.bytes[63] = 1u;
    if (mode == 3) output->label.byte_count = 1u;
    if (mode == 4) output->quality = 1u;
    error->domain = raw_domain; error->code = raw_code;
    return raw_status;
}
MadbValueStatus reading_demo_raw_normalize(const reading_demo_Reading *input, reading_demo_Reading *output, MadbValueError *error) {
    return provider(input, output, error);
}
MadbValueStatus reading_demo_raw_offset(const reading_demo_Reading *input, int32_t delta, reading_demo_Reading *output, MadbValueError *error) {
    assert(delta == 3);
    return provider(input, output, error);
}
MadbValueStatus reading_demo_raw_alternative(const reading_demo_Reading *input, reading_demo_Reading *output, MadbValueError *error) {
    return provider(input, output, error);
}
MadbValueStatus reading_demo_raw_count(const reading_demo_Count *input, int32_t delta, reading_demo_Count *output, MadbValueError *error) {
    ++raw_calls; assert(input->value == 4u && delta == 3); assert(output->value == 0u && error->domain == 0u && error->code == 0);
    output->value = 7u;
    return 0;
}
static void failure(reading_demo_ReadingResult result, unsigned kind, int status, unsigned domain, int code) {
    assert(result.kind == kind && result.status == status);
    assert(result.error.domain == domain && result.error.code == code);
    assert(result.value.value == 0 && result.value.quality == 0u && result.value.label.byte_count == 0u);
    for (unsigned i = 0; i < 64; ++i) assert(result.value.label.bytes[i] == 0u);
    assert(boot_calls == 1);
}
static reading_demo_Reading input_value(void) {
    reading_demo_Reading input = {0};
    input.value = 7; input.quality = 42u;
    input.label.byte_count = 65u; /* wrapper must leave input admission to raw */
    return input;
}
static void bootstrap_order(void) {
    reading_demo_Reading input = input_value();
    const int statuses[] = {1,8,9,11,2,5,10,-1,INT_MAX};
    for (unsigned i = 0; i < sizeof statuses / sizeof statuses[0]; ++i) {
        reset(); boot_status = statuses[i]; mismatch = 3;
        int wanted = i < 4 ? statuses[i] : 8;
        failure(reading_demo_normalize(input), 1u, wanted, 0u, 0); assert(raw_calls == 0);
    }
    reset(); mismatch = 3;
    failure(reading_demo_normalize(input), 2u, 11, 0u, 0); assert(raw_calls == 0);
    reset(); mismatch = 2;
    failure(reading_demo_normalize(input), 3u, 11, 0u, 0); assert(raw_calls == 0);
    reset(); raw_status = 1;
    failure(reading_demo_normalize(input), 4u, 1, 0u, 0); assert(raw_calls == 1);
}
static void success_values(void) {
    reading_demo_Reading input = input_value();
    reset();
    reading_demo_ReadingResult result = reading_demo_normalize(input);
    assert(result.kind == 0u && result.status == 0 && result.error.domain == 0u && result.error.code == 0);
    assert(result.value.value == 7 && result.value.quality == 42u && result.value.label.byte_count == 6u);
    assert(memcmp(result.value.label.bytes, expected.label.bytes, 64) == 0);
    assert(input.label.byte_count == 65u && boot_calls == 1 && raw_calls == 1);
    const unsigned char texts[][8] = {{0}, {'a',0,'b'}, {0xf0,0x9f,0x98,0x80}, {0xc2,0x80}, {0xe0,0xa0,0x80}, {0xed,0x9f,0xbf}, {0xf4,0x8f,0xbf,0xbf}};
    const unsigned sizes[] = {0,3,4,2,3,3,4};
    for (unsigned i = 0; i < sizeof sizes / sizeof sizes[0]; ++i) {
        reset(); memset(&expected.label, 0, sizeof expected.label);
        expected.label.byte_count = sizes[i]; memcpy(expected.label.bytes, texts[i], sizes[i]);
        result = reading_demo_normalize(input);
        assert(result.kind == 0u && result.status == 0 && result.value.label.byte_count == sizes[i]);
        assert(memcmp(result.value.label.bytes, texts[i], sizes[i]) == 0 && raw_calls == 1);
    }
    reset(); expected.label.byte_count = 64; memset(expected.label.bytes, 'x', 64);
    result = reading_demo_normalize(input); assert(result.kind == 0u && result.value.label.byte_count == 64u);
    reset(); reading_demo_Count count = {4};
    reading_demo_CountResult counted = reading_demo_count(count, 3);
    assert(counted.kind == 0u && counted.status == 0 && counted.value.value == 7u && boot_calls == 1 && raw_calls == 1);
}
static void invalid_outputs(void) {
    reading_demo_Reading input = input_value();
    const unsigned char bad[][5] = {{0xc0,0x80}, {0xc1,0xbf}, {0x80}, {0xe2,0x82}, {0xe0,0x80,0x80}, {0xed,0xa0,0x80}, {0xf0,0x80,0x80,0x80}, {0xf4,0x90,0x80,0x80}, {0xf5,0x80,0x80,0x80}, {0xff}, {0xe2,0x41,0x80}};
    const unsigned sizes[] = {2,2,1,2,3,3,4,4,4,1,3};
    for (unsigned i = 0; i < sizeof sizes / sizeof sizes[0]; ++i) {
        reset(); memset(&expected.label, 0, sizeof expected.label);
        expected.label.byte_count = sizes[i]; memcpy(expected.label.bytes, bad[i], sizes[i]);
        failure(reading_demo_normalize(input), 4u, 8, 0u, 0); assert(raw_calls == 1);
    }
    reset(); expected.label.byte_count = 65;
    failure(reading_demo_normalize(input), 4u, 8, 0u, 0); assert(raw_calls == 1);
    reset(); expected.label.bytes[63] = 1;
    failure(reading_demo_normalize(input), 4u, 8, 0u, 0);
    reset(); raw_domain = 1;
    failure(reading_demo_normalize(input), 4u, 8, 0u, 0);
    reset(); raw_code = 1;
    failure(reading_demo_normalize(input), 4u, 8, 0u, 0);
}
static void failure_protocol(void) {
    reading_demo_Reading input = input_value();
    const int valid[] = {1,8,9};
    for (unsigned i = 0; i < sizeof valid / sizeof valid[0]; ++i) {
        reset(); raw_status = valid[i];
        failure(reading_demo_normalize(input), 4u, valid[i], 0u, 0); assert(raw_calls == 1);
        reset(); raw_status = valid[i]; raw_domain = 1;
        failure(reading_demo_normalize(input), 4u, 8, 0u, 0);
        for (int corruption = 1; corruption <= 4; ++corruption) {
            reset(); raw_status = valid[i]; mode = corruption;
            failure(reading_demo_normalize(input), 4u, 8, 0u, 0);
        }
    }
    const int invalid[] = {2,3,4,5,6,7,11,-1,INT_MIN,INT_MAX};
    for (unsigned i = 0; i < sizeof invalid / sizeof invalid[0]; ++i) {
        reset(); raw_status = invalid[i];
        failure(reading_demo_normalize(input), 4u, 8, 0u, 0); assert(raw_calls == 1);
    }
    reset(); raw_status = 10; raw_domain = 1; raw_code = 1;
    failure(reading_demo_normalize(input), 4u, 8, 0u, 0); assert(raw_calls == 1);
    reset(); raw_status = 10; raw_domain = 1; raw_code = 1;
    failure(reading_demo_offset(input, 3), 4u, 10, 1u, 1); assert(raw_calls == 1);
    reset(); raw_status = 10; raw_domain = 1; raw_code = 7;
    failure(reading_demo_offset(input, 3), 4u, 8, 0u, 0); assert(raw_calls == 1);
    reset(); raw_status = 10; raw_domain = 1; raw_code = 7;
    failure(reading_demo_alternative(input), 4u, 10, 1u, 7); assert(raw_calls == 1);
    reset(); raw_status = 10; raw_domain = 0; raw_code = 1;
    failure(reading_demo_offset(input, 3), 4u, 8, 0u, 0);
    reset(); raw_status = 10; raw_domain = 1; raw_code = 0;
    failure(reading_demo_offset(input, 3), 4u, 8, 0u, 0);
    reset(); raw_status = 10; raw_domain = 1; raw_code = 1; mode = 1;
    failure(reading_demo_offset(input, 3), 4u, 8, 0u, 0);
}
int main(void) {
    bootstrap_order(); success_values(); invalid_outputs(); failure_protocol();
    puts("value wrapper: bootstrap, identity order, UTF-8, zero outputs, errors, single calls passed");
    return 0;
}
"""


class ValuesClientTests(unittest.TestCase):
    def test_identity_is_not_caller_supplied(self):
        for key, value in (("schema", "madb.native-identity/2"), ("bootstrap_version", True),
                           ("bootstrap_version", 2), ("interface_digest", "sha256:" + "11" * 31),
                           ("profile_digest", "sha256:" + "AA" * 32)):
            with self.subTest(key=key, value=value):
                identity = dict(IDENTITY, **{key: value})
                with self.assertRaises(NativeError):
                    render_values_client(PLAN, identity)

    def test_invalid_internal_plans_are_not_rendered(self):
        mutations = [
            lambda p: p.update(namespace='bad"include'),
            lambda p: p["records"][0]["fields"][0].update(kind="f64"),
            lambda p: p["records"][0].update(fields=[]),
            lambda p: p["errors"][0]["variants"][0].update(value=True),
            lambda p: p["functions"][0].update(error="Unknown"),
            lambda p: p["functions"][1]["parameters"][1].update(name="input"),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                plan = deepcopy(PLAN); mutate(plan)
                with self.assertRaises(NativeError):
                    render_values_client(plan, IDENTITY)

    def test_real_c_provider_protocol(self):
        clang = shutil.which("clang")
        self.assertIsNotNone(clang, "real C11 compiler required for native value wrapper tests")
        with tempfile.TemporaryDirectory(prefix="madb-values-client-") as temporary:
            work = Path(temporary)
            (work / "madb_value_abi_v1.h").write_text(BASE_HEADER)
            (work / "reading_demo_native.h").write_text(NATIVE_HEADER)
            (work / "reading_demo.h").write_text(render_values_client(PLAN, IDENTITY))
            (work / "consumer.c").write_text(PROGRAM)
            command = [clang, "-std=c11", "-Wall", "-Wextra", "-Werror", "consumer.c", "-o", "consumer"]
            compiled = subprocess.run(command, cwd=work, text=True, capture_output=True, timeout=30)
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            result = subprocess.run([str(work / "consumer")], cwd=work, text=True, capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("single calls passed", result.stdout)


if __name__ == "__main__":
    unittest.main()
