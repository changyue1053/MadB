"""Real compiler/runtime checks for the finite value renderer, not Lab acceptance."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "adapters/generators/rust_c"))
from support import NativeError
from values_client import render_values_client
from values_codegen import render_values_cbindgen_config, render_values_shim


IDENTITY = {"schema": "madb.native-identity/1", "bootstrap_version": 1,
            "interface_digest": "sha256:" + "11" * 32, "profile_digest": "sha256:" + "22" * 32}
PLAN = {"namespace": "reading_demo", "records": [{"name": "Reading", "fields": [
    {"name": "value", "kind": "i32"}, {"name": "label", "kind": "String"}, {"name": "quality", "kind": "u32"}]}],
    "errors": [{"name": "ReadingError", "variants": [{"name": "Overflow", "value": 1}]}],
    "functions": [
        {"name": "normalize", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}], "error": None},
        {"name": "offset", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}, {"name": "delta", "kind": "i32"}], "error": "ReadingError"},
    ]}
AUTHOR = """pub struct Reading { pub value: i32, pub label: String, pub quality: u32 }
#[repr(i32)]
pub enum ReadingError { Overflow = 1 }
pub fn normalize(mut value: Reading) -> Reading {
    value.label = value.label.trim().to_owned(); value
}
pub fn offset(mut value: Reading, delta: i32) -> Result<Reading, ReadingError> {
    value.value = value.value.checked_add(delta).ok_or(ReadingError::Overflow)?; Ok(value)
}
"""

NATIVE_CONSUMER = r'''#include "reading_demo.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>
#include <limits.h>

_Static_assert(sizeof(MadbValueError) == 8, "error layout");
_Static_assert(sizeof(MadbValueUtf8_64) == 68, "text layout");
_Static_assert(_Alignof(MadbValueUtf8_64) == 4, "text alignment");
_Static_assert(offsetof(MadbValueUtf8_64, bytes) == 4, "text payload offset");
_Static_assert(sizeof(reading_demo_Reading) == 76, "actual record layout");
_Static_assert(offsetof(reading_demo_Reading, quality) == 72, "source field order");

static void zero(const reading_demo_Reading *value) {
    assert(value->value == 0 && value->quality == 0 && value->label.byte_count == 0);
    for (unsigned i=0; i<64; ++i) assert(value->label.bytes[i] == 0);
}
static void failed(reading_demo_ReadingResult value, int32_t status) {
    assert(value.kind == 4 && value.status == status);
    assert(value.error.domain == 0 && value.error.code == 0); zero(&value.value);
}
int main(void) {
    const uint8_t utf8[10] = {32,32,0xe6,0xb8,0xa9,0xe5,0xba,0xa6,32,32};
    reading_demo_Reading input = {0}; input.value = 7; input.quality = 42;
    input.label.byte_count = 10; memcpy(input.label.bytes, utf8, 10);
    memset(input.label.bytes + 10, 0xa5, 54);
    reading_demo_Reading original = input;
    reading_demo_ReadingResult result = reading_demo_normalize(input);
    assert(result.kind == 0 && result.status == 0 && result.error.domain == 0 && result.error.code == 0);
    assert(result.value.value == 7 && result.value.quality == 42);
    assert(result.value.label.byte_count == 6 && memcmp(result.value.label.bytes, utf8+2, 6) == 0);
    for (unsigned i=6; i<64; ++i) assert(result.value.label.bytes[i] == 0);
    assert(memcmp(&input, &original, sizeof input) == 0);
    result = reading_demo_offset(input, 5); assert(result.kind == 0 && result.value.value == 12);
    input.value = INT32_MAX; result = reading_demo_offset(input, 1);
    assert(result.kind == 4 && result.status == 10 && result.error.domain == 1);
    assert(result.error.code == reading_demo_ReadingError_Overflow); zero(&result.value);
    input.value = INT32_MIN; result = reading_demo_offset(input, -1);
    assert(result.kind == 4 && result.status == 10 && result.error.code == 1); zero(&result.value);
    input.value = 7;
    const uint8_t valid[][64] = {{0}, {'a',0,'b'}, {0xf0,0x9f,0x8c,0xa1}};
    const unsigned lengths[] = {0,3,4};
    for (unsigned j=0; j<3; ++j) {
        memset(input.label.bytes, 0xa5, 64); memcpy(input.label.bytes, valid[j], lengths[j]);
        input.label.byte_count = lengths[j]; result = reading_demo_normalize(input);
        assert(result.kind == 0 && result.value.label.byte_count == lengths[j]);
        assert(memcmp(result.value.label.bytes, valid[j], lengths[j]) == 0);
        for (unsigned i=lengths[j]; i<64; ++i) assert(result.value.label.bytes[i] == 0);
    }
    input.label.byte_count=64; memset(input.label.bytes,'x',64); result=reading_demo_normalize(input);
    assert(result.kind==0 && result.value.label.byte_count==64);
    for(unsigned i=0;i<64;++i) assert(result.value.label.bytes[i]=='x');
    input.label.byte_count=65; failed(reading_demo_normalize(input),1);
    const uint8_t invalid[][4]={{0x80},{0xc0,0xaf},{0xed,0xa0,0x80},{0xf4,0x90,0x80,0x80},
        {0xf0,0x80,0x80,0x80},{0xe2,0x82},{0xe0,0x80,0x80},{0xff}};
    const unsigned bad_lengths[]={1,2,3,4,4,2,3,1};
    for(unsigned j=0;j<8;++j) {
        input.label.byte_count=bad_lengths[j]; memcpy(input.label.bytes,invalid[j],bad_lengths[j]);
        failed(reading_demo_normalize(input),1);
    }
    reading_demo_Reading output; memset(&output,0xa5,sizeof output);
    MadbValueError error={9,9};
    assert(reading_demo_raw_normalize(&input, NULL, &error)==1);
    assert(error.domain==0 && error.code==0);
    assert(reading_demo_raw_normalize(&input, &output, NULL)==1); zero(&output);
    memset(&output,0xa5,sizeof output); error=(MadbValueError){9,9};
    assert(reading_demo_raw_normalize(NULL,&output,&error)==1); zero(&output);
    assert(error.domain==0 && error.code==0);
    _Alignas(4) uint8_t misaligned[sizeof(reading_demo_Reading)+4]; memset(misaligned,0x5a,sizeof misaligned);
    assert(reading_demo_raw_normalize((const reading_demo_Reading *)(misaligned+1),&output,&error)==1);
    zero(&output); assert(error.domain==0 && error.code==0);
    memset(&output,0xa5,sizeof output);
    assert(reading_demo_raw_normalize(&input,&output,(MadbValueError *)(misaligned+1))==1); zero(&output);
    for(unsigned i=0;i<sizeof misaligned;++i) assert(misaligned[i]==0x5a);
    uint8_t identity[64]; memset(identity,0xa5,64);
    assert(reading_demo_bootstrap_v1(1,identity,63)==1);
    for(unsigned i=0;i<64;++i) assert(identity[i]==0xa5);
    assert(reading_demo_bootstrap_v1(2,identity,64)==11);
    for(unsigned i=0;i<64;++i) assert(identity[i]==0);
    assert(reading_demo_bootstrap_v1(1,NULL,64)==1);
    assert(reading_demo_bootstrap_v1(1,identity,64)==0);
    for(unsigned i=0;i<64;++i) assert(identity[i] == (i<32 ? 0x11 : 0x22));
    return 0;
}
'''

# The surrounding test crate supplies the observer/allocator. It is not a
# natural-author reader fixture or a published candidate and adds no SDK API.
FAULT_AUTHOR = """pub struct Reading { pub value: i32, pub label: String, pub other: String }
#[repr(i32)]
pub enum ReadingError { Rejected = 7 }
pub fn echo(value: Reading) -> Reading {
    crate::AUTHOR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); value
}
pub fn long(mut value: Reading) -> Reading {
    crate::AUTHOR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    value.label = "x".repeat(65); value
}
pub fn unwind(_value: Reading) -> Reading {
    crate::AUTHOR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); panic!("controlled unwind")
}
pub fn domain_failure(_value: Reading) -> Result<Reading, ReadingError> {
    crate::AUTHOR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); Err(ReadingError::Rejected)
}
pub fn payload_unwind(value: Reading) -> Reading {
    crate::AUTHOR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    struct Primary { _held: String, fatal: bool }
    struct Secondary { _held: String, fatal: bool }
    impl Drop for Primary {
        fn drop(&mut self) {
            crate::PRIMARY_DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::panic::panic_any(Secondary { _held: "secondary allocation".to_owned(), fatal: self.fatal });
        }
    }
    impl Drop for Secondary {
        fn drop(&mut self) {
            crate::SECONDARY_DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fatal {
                eprintln!("MADB_TEST_SECONDARY_PAYLOAD_DROP");
                panic!("terminal cleanup panic");
            }
        }
    }
    std::panic::panic_any(Primary { _held: "primary allocation".to_owned(), fatal: value.value == 99 });
}
"""
FAULT_HARNESS = r'''use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering::SeqCst};
static AUTHOR_CALLS: AtomicUsize = AtomicUsize::new(0);
static PRIMARY_DROPS: AtomicUsize = AtomicUsize::new(0);
static SECONDARY_DROPS: AtomicUsize = AtomicUsize::new(0);
static FAIL_AFTER: AtomicIsize = AtomicIsize::new(-1);
static ALLOCATED: AtomicIsize = AtomicIsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);
struct ControlledAllocator;
unsafe impl GlobalAlloc for ControlledAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let count = FAIL_AFTER.load(SeqCst);
        if count >= 0 {
            FAIL_AFTER.store(count-1, SeqCst);
            if count == 0 { FAILED.fetch_add(1, SeqCst); return std::ptr::null_mut(); }
        }
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() { ALLOCATED.fetch_add(1, SeqCst); }
        result
    }
    unsafe fn dealloc(&self, pointer:*mut u8, layout:Layout) {
        ALLOCATED.fetch_sub(1, SeqCst); unsafe { System.dealloc(pointer, layout); }
    }
}
#[global_allocator] static ALLOCATOR: ControlledAllocator = ControlledAllocator;
#[path="values_shim.rs"] mod generated;
use generated::*;
fn zero(value:&reading_demo_Reading) {
    assert_eq!(value.value,0); assert_eq!(value.label.byte_count,0); assert_eq!(value.other.byte_count,0);
    assert!(value.label.bytes.iter().all(|x|*x==0)); assert!(value.other.bytes.iter().all(|x|*x==0));
}
fn input() -> reading_demo_Reading {
    let mut bytes = [0;64]; bytes[0]=b'x';
    reading_demo_Reading {value:7,label:MadbValueUtf8_64{byte_count:1,bytes},other:MadbValueUtf8_64{byte_count:1,bytes}}
}
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let source=input(); let mut output=input(); let mut error=MadbValueError{domain:9,code:9};
    if std::env::args().any(|argument| argument == "payload-cleanup-abort") {
        let mut fatal_source=input(); fatal_source.value=99;
        unsafe { reading_demo_raw_payload_unwind(&fatal_source,&mut output,&mut error); }
        std::process::exit(97); // This mode must terminate while cleaning the secondary payload.
    }
    // Warm the runtime's panic machinery before measuring per-call allocation balance.
    assert_eq!(unsafe {reading_demo_raw_unwind(&source,&mut output,&mut error)},8);
    AUTHOR_CALLS.store(0,SeqCst);
    let baseline=ALLOCATED.load(SeqCst);
    for fail_after in [0,1] {
        output=input(); error=MadbValueError{domain:9,code:9};
        FAIL_AFTER.store(fail_after,SeqCst);
        let status=unsafe{reading_demo_raw_echo(&source,&mut output,&mut error)};
        FAIL_AFTER.store(-1,SeqCst);
        assert_eq!(status,9); assert_eq!(error.domain,0); assert_eq!(error.code,0); zero(&output);
        assert_eq!(AUTHOR_CALLS.load(SeqCst),0); assert_eq!(ALLOCATED.load(SeqCst),baseline);
    }
    assert_eq!(FAILED.load(SeqCst),2);
    // A later bad text must be rejected before an earlier valid field allocates.
    let mut bad=input(); bad.other.byte_count=65; FAIL_AFTER.store(0,SeqCst);
    let status=unsafe{reading_demo_raw_echo(&bad,&mut output,&mut error)};
    FAIL_AFTER.store(-1,SeqCst);
    assert_eq!(status,1); assert_eq!(FAILED.load(SeqCst),2); assert_eq!(AUTHOR_CALLS.load(SeqCst),0);
    assert_eq!(ALLOCATED.load(SeqCst),baseline); zero(&output);
    assert_eq!(unsafe{reading_demo_raw_echo(&source,&mut output,&mut error)},0);
    assert_eq!(AUTHOR_CALLS.load(SeqCst),1); assert_eq!(ALLOCATED.load(SeqCst),baseline);
    assert_eq!(output.label.byte_count,1); assert_eq!(output.other.byte_count,1);
    assert_eq!(unsafe{reading_demo_raw_long(&source,&mut output,&mut error)},9);
    assert_eq!(AUTHOR_CALLS.load(SeqCst),2); assert_eq!(ALLOCATED.load(SeqCst),baseline); zero(&output);
    assert_eq!(error.domain,0); assert_eq!(error.code,0);
    assert_eq!(unsafe{reading_demo_raw_unwind(&source,&mut output,&mut error)},8);
    assert_eq!(AUTHOR_CALLS.load(SeqCst),3); assert_eq!(ALLOCATED.load(SeqCst),baseline); zero(&output);
    assert_eq!(error.domain,0); assert_eq!(error.code,0);
    assert_eq!(unsafe{reading_demo_raw_domain_failure(&source,&mut output,&mut error)},10);
    assert_eq!(AUTHOR_CALLS.load(SeqCst),4); assert_eq!(ALLOCATED.load(SeqCst),baseline); zero(&output);
    assert_eq!(error.domain,1); assert_eq!(error.code,7);
    output=input(); error=MadbValueError{domain:9,code:9};
    assert_eq!(unsafe{reading_demo_raw_payload_unwind(&source,&mut output,&mut error)},8);
    assert_eq!(AUTHOR_CALLS.load(SeqCst),5); assert_eq!(ALLOCATED.load(SeqCst),baseline); zero(&output);
    assert_eq!(error.domain,0); assert_eq!(error.code,0);
    assert_eq!(PRIMARY_DROPS.load(SeqCst),1); assert_eq!(SECONDARY_DROPS.load(SeqCst),1);
}
'''


class NativeValuesCodegenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rustc = subprocess.check_output(["rustup", "which", "rustc"], cwd=ROOT, text=True).strip()
        cls.cbindgen = shutil.which("cbindgen")
        cls.clang = (subprocess.check_output(["xcrun", "--find", "clang"], text=True).strip()
                     if sys.platform == "darwin" else shutil.which("clang"))
        if not cls.cbindgen or not cls.clang:
            raise RuntimeError("real value codegen checks require installed cbindgen 0.29.4 and Clang")
        assert subprocess.check_output([cls.rustc, "--version"], text=True).startswith("rustc 1.99.1 ")
        assert subprocess.check_output([cls.cbindgen, "--version"], text=True).strip() == "cbindgen 0.29.4"
        cls.c_flags = ["-std=c11", "-Wall", "-Wextra", "-Werror"]
        if sys.platform == "darwin":
            cls.c_flags += ["-isysroot", subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()]

    def setUp(self):
        parent = os.environ.get("MADB_VALUES_CODEGEN_OUTPUT")
        if parent:
            Path(parent).mkdir(parents=True, exist_ok=True)
        self.work = Path(tempfile.mkdtemp(prefix=self._testMethodName + "-", dir=parent))
        self.records = []
        if not parent:
            self.addCleanup(shutil.rmtree, self.work)

    def run_command(self, argv, *, expected_returncode=0):
        run = subprocess.run([str(a) for a in argv], cwd=self.work, capture_output=True, text=True, timeout=60)
        self.records.append({"argv": [str(a) for a in argv], "exit_code": run.returncode, "stdout": run.stdout, "stderr": run.stderr})
        (self.work / "commands.json").write_text(json.dumps(self.records, indent=2) + "\n")
        self.assertEqual(run.returncode, expected_returncode, run.stdout + run.stderr)
        return run

    def write_sources(self, plan=PLAN, author=AUTHOR):
        (self.work / "author.rs").write_text(author)
        (self.work / "plan.json").write_text(json.dumps(plan, indent=2) + "\n")
        (self.work / "values_shim.rs").write_text(render_values_shim(plan, IDENTITY))

    def test_real_cbindgen_layout_and_native_c_calls(self):
        self.write_sources()
        for base_only, header in ((True, "madb_value_abi_v1.h"), (False, "reading_demo_native.h")):
            config = self.work / (header + ".toml")
            config.write_text(render_values_cbindgen_config(PLAN, base_only=base_only))
            self.run_command([self.cbindgen, "--config", config, self.work / "values_shim.rs", "--output", self.work / header,
                              "--depfile", self.work / (header + ".d")])
        (self.work / "reading_demo.h").write_text(render_values_client(PLAN, IDENTITY))
        (self.work / "consumer.c").write_text(NATIVE_CONSUMER)
        library = self.work / ("libreading.dylib" if sys.platform == "darwin" else "libreading.so")
        self.run_command([self.rustc, "--edition=2024", "--crate-type=cdylib", "-C", "panic=unwind", "-C", "opt-level=2",
                          self.work / "values_shim.rs", "-o", library])
        self.run_command([self.clang, *self.c_flags, self.work / "consumer.c", library, "-Wl,-rpath," + str(self.work),
                          "-o", self.work / "consumer"])
        self.run_command([self.work / "consumer"])

    def test_actual_reserve_failure_unwind_and_temporary_cleanup(self):
        plan = {"namespace": "reading_demo", "records": [{"name": "Reading", "fields": [
            {"name": "value", "kind": "i32"}, {"name": "label", "kind": "String"}, {"name": "other", "kind": "String"}]}],
            "errors": [{"name": "ReadingError", "variants": [{"name": "Rejected", "value": 7}]}],
            "functions": [{"name": name, "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}], "error": None}
                                          for name in ("echo", "long", "unwind", "payload_unwind")]}
        plan["functions"].append({"name": "domain_failure", "record": "Reading", "parameters": [{"name": "value", "kind": "Reading"}], "error": "ReadingError"})
        self.write_sources(plan, FAULT_AUTHOR)
        shim = self.work / "values_shim.rs"
        before = hashlib.sha256(shim.read_bytes()).hexdigest()
        (self.work / "harness.rs").write_text(FAULT_HARNESS)
        self.run_command([self.rustc, "--edition=2024", "-C", "panic=unwind", "-C", "opt-level=2",
                          self.work / "harness.rs", "-o", self.work / "harness"])
        self.run_command([self.work / "harness"])
        # A further payload destructor panic is an explicit process-failure
        # boundary, not a recoverable status or an unwind through extern C.
        aborted = self.run_command([self.work / "harness", "payload-cleanup-abort"], expected_returncode=-signal.SIGABRT)
        self.assertIn("MADB_TEST_SECONDARY_PAYLOAD_DROP", aborted.stderr)
        self.assertNotIn("thread caused non-unwinding panic", aborted.stderr)
        self.assertEqual(before, hashlib.sha256(shim.read_bytes()).hexdigest())
        (self.work / "generated-shim-identity.json").write_text(json.dumps({"sha256": before,
            "scope": "Unmodified rendered shim in an independent process-only allocator harness; not reader or Candidate acceptance"}, indent=2) + "\n")

    def test_argument_name_does_not_shadow_generated_rust_locals(self):
        for name in ("source", "owned", "returned", "value_ok", "error_ok", "call", "text_0", "madb_value_argument"):
            plan = copy.deepcopy(PLAN)
            plan["functions"][1]["parameters"][1]["name"] = name
            self.write_sources(plan, AUTHOR.replace("delta", name))
            self.run_command([self.rustc, "--edition=2024", "--crate-type=lib", "--emit=metadata",
                              self.work / "values_shim.rs", "-o", self.work / (name + ".rmeta")])

class ValueRenderingTests(unittest.TestCase):
    """Pure baseline tests; tool-dependent proofs are explicitly separate."""

    def test_identity_and_finite_rendering_reject_invalid_input(self):
        for key in ("interface_digest", "profile_digest"):
            identity = dict(IDENTITY, **{key: "sha256:" + "G" * 64})
            with self.assertRaises(NativeError):
                render_values_shim(PLAN, identity)
        plan = copy.deepcopy(PLAN)
        plan["records"][0]["fields"][1]["kind"] = "Vec"
        with self.assertRaises(NativeError):
            render_values_shim(plan, IDENTITY)
        with self.assertRaises(NativeError):
            render_values_shim(PLAN, dict(IDENTITY, bootstrap_version=True))


if __name__ == "__main__":
    unittest.main()
