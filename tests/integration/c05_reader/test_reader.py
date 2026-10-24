"""Real, locked native reader probes; these are not C05 SDK/Lab certification.

Run explicitly on the C03 locked macOS host:
  .venv/bin/python -m unittest discover -s tests/integration/c05_reader -v
The prerequisite tools may be selected with MADB_C03_TOOLS. Missing or mismatched
tools fail this focused suite instead of turning an unexecuted probe into a pass.
"""
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
ADAPTER = ROOT / "adapters/generators/rust_c"
sys.path.insert(0, str(ADAPTER))

C_HEADER = """#include <stdint.h>
typedef int32_t MadbStatus;
typedef struct MadbError { uint32_t domain; int32_t code; } MadbError;
MadbStatus madb_c_fixture_bootstrap(uint32_t requested_version, uint8_t *out_identity, uint64_t capacity);
MadbStatus madb_c_fixture_checksum(const uint8_t *data, uint64_t length_bytes, uint64_t *out_sum, MadbError *out_error);
#define MADB_OK 0
#define MADB_INVALID_ARGUMENT 1
#define MADB_INVALID_HANDLE 2
#define MADB_CLOSING 3
#define MADB_CLOSED 4
#define MADB_BUSY 5
#define MADB_TIMEOUT 6
#define MADB_WOULD_DEADLOCK 7
#define MADB_INTERNAL_FAILURE 8
#define MADB_RESOURCE_EXHAUSTED 9
#define MADB_DOMAIN_ERROR 10
#define MADB_UNSUPPORTED 11
"""


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reference(path):
    path = Path(path).resolve()
    data = path.read_bytes()
    return dict(path=str(path), sha256=digest(data), size=len(data))


def command(argv, *, env=None, cwd=None):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, env=env,
                            text=True, capture_output=True, timeout=60)
    if result.returncode:
        raise AssertionError(f"native command failed ({result.returncode}): {argv}\n{result.stderr}")
    return result.stdout.strip()


class NativeReader(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import native_ast
        import lowering
        import driver
        cls.native = native_ast
        cls.lowering = lowering
        cls.driver = driver
        recipe = json.loads((ROOT / "tests/integration/c03/toolchain.json").read_text())
        tool_root = Path(os.environ.get("MADB_C03_TOOLS", "/private/tmp/madb-c03-verified-tools"))
        receipt = json.loads((tool_root / "receipt.json").read_text())
        if receipt["rust"] != recipe["rust"]:
            raise AssertionError("generator receipt Rust version differs from C03 lock")
        cls.tools = {}
        for name in ("bindgen", "cbindgen"):
            cls.tools[name] = reference(tool_root / "bin" / name)
            record = receipt["generators"][name]
            if any(record.get(key) != value for key, value in recipe["generators"][name].items()):
                raise AssertionError(f"{name} source receipt differs from C03 lock")
            if cls.tools[name]["sha256"] != record["binary_sha256"]:
                raise AssertionError(f"{name} binary differs from source receipt")
            if command([cls.tools[name]["path"], "--version"]) != f"{name} {record['version']}":
                raise AssertionError(f"{name} executable reports unexpected version")
        cls.tools["clang"] = reference(command(["xcrun", "--find", "clang"]))
        cls.tools["clang"].update(id="reader-clang", version=recipe["clang"])
        cls.tools["rustc"] = reference(command(["rustup", "which", "--toolchain", recipe["rust"], "rustc"]))
        cls.tools["libclang"] = reference(Path(cls.tools["clang"]["path"]).parent.parent / "lib/libclang.dylib")
        cls.tools["finalizer"] = reference(ROOT / "target/debug/madb-native-finalize")
        if command([cls.tools["clang"]["path"], "--version"]).splitlines()[0] != recipe["clang"]:
            raise AssertionError("Clang differs from C03 lock")
        for argv, expected in [(["xcodebuild", "-version"], recipe["xcode"]),
                               (["xcrun", "--sdk", "macosx", "--show-sdk-version"], recipe["sdk_version"]),
                               (["xcrun", "--sdk", "macosx", "--show-sdk-build-version"], recipe["sdk_build"])]:
            if command(argv) != expected:
                raise AssertionError(f"native SDK prerequisite mismatch: {argv}")
        cls.sdk = command(["xcrun", "--sdk", "macosx", "--show-sdk-path"])
        resource = command([cls.tools["clang"]["path"], "--print-resource-dir"])
        cls.cflags = ["-target", "arm64-apple-macosx14.0", "-isysroot", cls.sdk,
                      "-resource-dir", resource, "-std=c11", "-Wall", "-Wextra", "-Werror"]
        cls.environment = {"PATH": os.defpath, "LC_ALL": "C", "SDKROOT": cls.sdk,
                           "MACOSX_DEPLOYMENT_TARGET": "14.0",
                           "LIBCLANG_PATH": str(Path(cls.tools["clang"]["path"]).parent.parent / "lib")}

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="madb-c05-reader-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source/provider.h"
        self.source.parent.mkdir()
        self.source.write_text(C_HEADER)
        self.config = dict(schema="madb.rust-c-backend/1", provider="c",
                           semantics="madb.first-route/1", source="source/provider.h",
                           exports=[dict(name="madb_c_fixture_bootstrap", role="bootstrap", public_id="reader.bootstrap.v1"),
                                    dict(name="madb_c_fixture_checksum", role="checksum", public_id="reader.checksum.v1")],
                           tools=deepcopy(self.tools), clang_arguments=list(self.cflags),
                           environment=dict(self.environment), rust_target="aarch64-apple-darwin")
        self.project = dict(schema="madb.project/1", project="reader-sdk", sdk_namespace="reader")

    def request(self):
        data = self.source.read_bytes()
        return dict(schema="madb.read-request/1", boundary_id="c-to-rust",
                    selector=dict(route="RUST-C-ABI", profile_id="reader-c-rust",
                                  profile_digest=digest(b"locked native reader test profile"),
                                  target="aarch64-apple-darwin", direction="consumer-to-provider"),
                    inputs=[dict(path=self.config["source"], sha256=digest(data), size=len(data),
                                 schema="madb.source-input/1")])

    def surface(self):
        return self.native.read_header(self.source, self.config)

    def lower(self):
        return self.lowering.lower(self.surface(), self.config, self.request(), self.project)

    def reject(self, code, operation=None):
        with self.assertRaises(self.native.NativeError) as caught:
            (operation or self.lower)()
        self.assertEqual(caught.exception.code, code)
        return str(caught.exception)

    def test_c_declarations_are_read_by_real_clang(self):
        surface = self.surface()
        self.assertEqual(set(surface["functions"]), {"madb_c_fixture_bootstrap", "madb_c_fixture_checksum"})
        checksum = surface["functions"]["madb_c_fixture_checksum"]
        self.assertEqual(checksum["result"], "i32")
        self.assertEqual(checksum["parameters"], [dict(name="data", type="const u8 *"),
                                                dict(name="length_bytes", type="u64"),
                                                dict(name="out_sum", type="u64 *"),
                                                dict(name="out_error", type="MadbError *")])
        self.assertEqual(surface["records"]["MadbError"], [dict(name="domain", type="u32"), dict(name="code", type="i32")])
        self.assertEqual(checksum["location"]["line"], 5)
        seed = self.lower()
        self.assertEqual({symbol["shape"]["link_name"] for symbol in seed["symbols"]}, set(surface["functions"]))
        self.assertEqual({symbol["precision"] for symbol in seed["symbols"]}, {"compiler-derived"})

    def test_changed_parameter_width_is_rejected(self):
        self.source.write_text(C_HEADER.replace("uint64_t length_bytes", "uint32_t length_bytes"))
        self.assertIn("madb_c_fixture_checksum", self.reject("NATIVE005"))

    def test_changed_pointer_constness_is_rejected(self):
        self.source.write_text(C_HEADER.replace("const uint8_t *data", "uint8_t *data"))
        self.assertIn("madb_c_fixture_checksum", self.reject("NATIVE005"))

    def test_changed_return_type_is_rejected(self):
        self.source.write_text(C_HEADER.replace("MadbStatus madb_c_fixture_checksum", "uint64_t madb_c_fixture_checksum"))
        self.assertIn("madb_c_fixture_checksum", self.reject("NATIVE005"))

    def test_missing_export_is_rejected(self):
        self.source.write_text("\n".join(line for line in C_HEADER.splitlines() if "madb_c_fixture_checksum" not in line) + "\n")
        self.assertIn("madb_c_fixture_checksum", self.reject("NATIVE004"))

    def test_extra_export_is_rejected_even_without_madb_prefix(self):
        self.source.write_text(C_HEADER + "uint64_t silently_ignored(uint64_t value);\n")
        self.assertIn("silently_ignored", self.reject("NATIVE004"))

    def test_public_function_macro_is_not_silently_omitted(self):
        self.source.write_text(C_HEADER + "#define MADB_CHECKSUM(x) (x)\n")
        self.assertIn("function-like macros", self.reject("NATIVE004"))

    def test_unmapped_pointer_cannot_enter_safe_lowering(self):
        self.source.write_text(C_HEADER.replace("const uint8_t *data", "void *data"))
        self.assertIn("madb_c_fixture_checksum", self.reject("NATIVE005"))

    def test_pointer_depth_cannot_collapse_to_a_supported_signature(self):
        for original, changed in (("uint8_t *out_identity", "uint8_t **out_identity"),
                                  ("uint64_t *out_sum", "uint64_t **out_sum")):
            with self.subTest(declaration=changed):
                self.source.write_text(C_HEADER.replace(original, changed))
                self.assertIn("ABI signature mismatch", self.reject("NATIVE005"))

    def test_moving_lines_preserves_identity_but_changes_locations(self):
        before = self.lower()
        self.source.write_text("\n\n\n" + C_HEADER)
        after = self.lower()
        self.assertNotEqual(before["source_digest"], after["source_digest"])
        self.assertEqual(len(before["symbols"]), len(after["symbols"]))
        for old, new in zip(before["symbols"], after["symbols"]):
            self.assertEqual(old["source_key"], new["source_key"])
            self.assertEqual(old["source_id"], new["source_id"])
            self.assertEqual(old["public_id"], new["public_id"])
            self.assertEqual(old["shape"], new["shape"])
            self.assertEqual(old["location"]["line"] + 3, new["location"]["line"])

    def test_changed_locked_tool_identity_is_rejected_before_clang(self):
        self.config["tools"]["clang"]["sha256"] = digest(b"not the locked clang binary")
        self.reject("NATIVE002", self.surface)

    def test_status_and_layout_drift_are_rejected(self):
        self.source.write_text(C_HEADER.replace("#define MADB_BUSY 5", "#define MADB_BUSY 55"))
        self.assertIn("status", self.reject("NATIVE005"))
        self.source.write_text(C_HEADER.replace("uint32_t domain; int32_t code;", "int32_t code; uint32_t domain;"))
        self.assertIn("MadbError", self.reject("NATIVE005"))

    def test_interface_identity_excludes_location_but_includes_abi(self):
        before = self.lowering.interface_identity(self.surface(), self.config, self.request()["selector"]["profile_digest"])
        self.source.write_text("\n\n" + C_HEADER)
        after = self.lowering.interface_identity(self.surface(), self.config, self.request()["selector"]["profile_digest"])
        self.assertEqual(before, after)
        changed = deepcopy(self.surface())
        changed["functions"]["madb_c_fixture_checksum"]["parameters"][0]["name"] = "bytes"
        another = self.lowering.interface_identity(changed, self.config, self.request()["selector"]["profile_digest"])
        self.assertNotEqual(before["interface_digest"], another["interface_digest"])

    def _c_generation_inputs(self):
        from support import canonical, semantic
        self.config.update(project="project.json", profile="profile.json", wrapper={
            "manifest": "wrapper/Cargo.toml", "source": "wrapper/src/lib.rs", "build_script": "wrapper/build.rs"})
        for key, path in self.config["wrapper"].items():
            original = {"manifest": "Cargo.toml", "source": "src/lib.rs", "build_script": "build.rs"}[key]
            destination = self.root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / "sdk/rust/c-client" / original, destination)
        profile = dict(schema="madb.profile/1", id="reader-c-rust", route="RUST-C-ABI",
                       provider=dict(language="c", runtime="native"), consumer=dict(language="rust", runtime="native"),
                       direction="consumer-to-provider", target="aarch64-apple-darwin", abi="c11",
                       artifact_kind="static-library", build_system="rustc-clang-probe", deployment_target="14.0",
                       tools=[dict(id="reader-clang", version=self.tools["clang"]["version"], sha256=self.tools["clang"]["sha256"],
                                   host="aarch64-apple-darwin", target="aarch64-apple-darwin", source="https://developer.apple.com/download/all/", arguments=self.cflags)],
                       sdk=dict(path="sdk.json", sha256=digest(b"locked-sdk"), size=10, schema="madb.source-input/1"),
                       macros=[], required_features=["core.values@1", "core.exact-numbers@1"], supported_features=["core.values@1", "core.exact-numbers@1"],
                       restrictions=[], defaults=dict(values=dict(ownership="copy"), origin=dict(authority="profile-default", id="copy-default", location=dict(path="profile.json", line=1, column=1))), backend_config=None)
        projection = deepcopy(profile)
        projection["defaults"] = projection["defaults"]["values"]
        for field in ("required_features", "supported_features"):
            projection[field].sort()
        profile_hash = semantic("madb.profile/1", projection)
        self.project.update(boundaries=[dict(id="c-to-rust", route="RUST-C-ABI", direction="consumer-to-provider", source=dict(kind="c-header", path=self.config["source"]), profile=None, target="aarch64-apple-darwin", output="generated")],
                            policies=[], policy_families=[], artifacts=dict(package="reader-test", manifest="candidate.json"))
        for index, export in enumerate(self.config["exports"]):
            self.project["policies"].append(dict(selector=dict(kind="public_id", id=export["public_id"]), slot=dict(kind="call"), families=[],
                values=dict(threading="caller", blocking="bounded", delivery="work-completed", cancellation="none", cancellation_confirmation="not-supported"),
                origin=dict(authority="project-policy", id="call-" + export["role"], location=dict(path="project.json", line=index + 1, column=1))))
            self.project["policies"].append(dict(selector=dict(kind="public_id", id=export["public_id"]), slot=dict(kind="return"), families=[],
                values=dict(ownership="copy"), origin=dict(authority="project-policy", id="return-" + export["role"], location=dict(path="project.json", line=index + 1, column=1))))
        (self.root / "project.json").write_bytes(canonical(self.project))
        (self.root / "profile.json").write_bytes(canonical(profile))
        request = self.request()
        request["selector"]["profile_digest"] = profile_hash
        for path in self.config["wrapper"].values():
            data = (self.root / path).read_bytes()
            request["inputs"].append(dict(path=path, sha256=digest(data), size=len(data), schema="madb.source-input/1"))
        self._sdk_inputs(self.source)
        return request

    def _sdk_inputs(self, header):
        import shlex
        output = command([self.tools["clang"]["path"], *self.cflags, "-M", "-x", "c", header])
        names = shlex.split(output.replace("\\\n", " ").partition(":")[2])
        self.config["sdk_inputs"] = [reference(Path(name)) for name in names if Path(name).resolve() != Path(header).resolve()]

    def test_c_generation_is_real_compiled_and_deterministic(self):
        from support import canonical
        request = self._c_generation_inputs()
        work = self.root / "metadata-work"
        work.mkdir()
        contract = self.driver.metadata(request, self.config, self.root, work)
        (self.root / "contract.json").write_bytes(canonical(contract))
        request.update(schema="madb.generate-request/1", contract=dict(path="contract.json"),
                       contract_digest=contract["structure_digest"], policy_digest=contract["policy_digest"],
                       input_digest=contract["source_digest"], task_digest=digest(b"independent fixed task identity"))
        generated = []
        for name in ("first", "second"):
            output = self.root / name
            output.mkdir()
            manifest = self.driver.generate(request, self.config, self.root, output)
            generated.append((manifest, {item["path"]: (output / item["path"]).read_bytes() for item in manifest["files"]}))
        self.assertEqual(generated[0], generated[1])
        files = generated[0][1]
        self.assertIn(b"unsafe extern \"C\"", files["rust/madb-c-client/src/raw.rs"])
        self.assertIn(b"madb_c_fixture_checksum", files["rust/madb-c-client/src/raw.rs"])
        self.assertEqual(files["include/madb_c.h"], self.source.read_bytes())
        self.assertEqual(set(files), {"include/madb_c.h", "identity.json", "identity.h", "rust/madb-c-client/Cargo.toml", "rust/madb-c-client/build.rs", "rust/madb-c-client/src/lib.rs", "rust/madb-c-client/src/raw.rs", "rust/madb-c-client/src/identity.rs"})

    def test_tool_image_cannot_add_unvalidated_arguments(self):
        request = self._c_generation_inputs()
        self.config["tools"]["clang"]["arguments"] = ["-target", "x86_64-apple-macosx14.0"]
        self.reject("NATIVE001", lambda: self.driver._verify_context(request, self.config, self.root))

    def test_compiler_flags_cannot_override_the_declared_target(self):
        request = self._c_generation_inputs()
        for flags in (("-m32",), ("-arch", "i386"), ("--target=i386-unknown-linux-gnu",), ("-Xclang", "-triple", "-Xclang", "i386-unknown-linux-gnu")):
            with self.subTest(flags=flags):
                self.config["clang_arguments"] = [*self.cflags, *flags]
                self.reject("NATIVE001", lambda: self.driver._verify_context(request, self.config, self.root))

    def test_real_engine_cbindgen_surface_is_deterministic(self):
        from support import canonical
        source_root = ROOT / "sdk/rust/engine/src"
        for original in source_root.glob("*.rs"):
            destination = self.root / "engine" / original.name
            destination.parent.mkdir(exist_ok=True)
            shutil.copyfile(original, destination)
        configuration = self.root / "cbindgen.toml"
        shutil.copyfile(ROOT / "sdk/rust/engine/cbindgen.toml", configuration)
        self.config.update(provider="rust", source="engine/lib.rs", cbindgen_config="cbindgen.toml", project="project.json", exports=[
            dict(name="madb_fixture_" + role, role=role, public_id="reader." + role + ".v1")
            for role in ["bootstrap", "create", "checksum", "request_close", "await_closed", "release_handle", "observe"]])
        (self.root / "project.json").write_bytes(canonical(self.project))
        request = self.request()
        request["inputs"] = []
        for path in [*sorted((self.root / "engine").glob("*.rs")), configuration]:
            data = path.read_bytes()
            request["inputs"].append(dict(path=path.relative_to(self.root).as_posix(), sha256=digest(data), size=len(data), schema="madb.source-input/1"))
        discovery = self.root / "discovery"
        discovery.mkdir()
        header = self.driver._header(request, self.config, self.root, discovery)
        self._sdk_inputs(header)
        results = []
        for name in ("rust-one", "rust-two"):
            work = self.root / name
            work.mkdir()
            results.append(self.driver.metadata_seed(request, self.config, self.root, work))
        self.assertEqual(results[0], results[1])
        self.assertEqual({s["precision"] for s in results[0]["symbols"]}, {"artifact-derived"})
        self.assertEqual({s["location"]["path"] for s in results[0]["symbols"]}, {"engine/lib.rs"})
        self.assertEqual(len(results[0]["symbols"]), 7)

    def test_prepared_observe_policy_is_unbounded_and_finalizes(self):
        sys.path.insert(0, str(ROOT))
        try:
            from adapters.toolchains.rust_c.project import prepare
            from adapters.toolchains.rust_c.tools import discover
            observation = discover(ROOT, self.tools["bindgen"]["path"], self.tools["cbindgen"]["path"])
            original = deepcopy(observation)
            workspace = self.root / "prepared-engine"
            prepared = prepare(ROOT, workspace, observation, "c-to-rust")
        finally:
            sys.path.pop(0)
        self.assertEqual(observation, original)
        work = self.root / "prepared-metadata"
        work.mkdir()
        contract = self.driver.metadata(prepared["read"], prepared["config"], workspace, work)
        policies = [item for item in prepared["project"]["policies"]
                    if item["selector"]["id"] == "madb.engine.observe.v1" and item["slot"]["kind"] == "call"]
        self.assertEqual(len(policies), 1)
        self.assertEqual(policies[0]["values"]["blocking"], "unbounded")
        self.assertEqual(len(contract["symbols"]), 7)
        self.assertTrue(contract["policies"])
        self.assertIn("sdk/rust/engine/runtime-protocol.json", {item["path"] for item in prepared["read"]["inputs"]})


if __name__ == "__main__":
    unittest.main()
