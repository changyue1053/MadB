"""Pure gates by discovery; explicit --tool-observation enables real reader gates."""

from contextlib import redirect_stderr, redirect_stdout
from hashlib import sha256
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import declaration_diagnostics as diagnostics
from adapters.generators.arkts import driver, support


def reference(data, path="author/Boundary.ets"):
    return {"path": path, "sha256": "sha256:" + sha256(data).hexdigest(), "size": len(data)}


class BindingTests(unittest.TestCase):
    def test_bom_keeps_raw_bytes_and_reader_offsets_distinct(self):
        for prefix in ("\ufeff", "\ufeff\ufeff"):
            text = prefix + "/* x */export"
            data = text.encode()
            ref = reference(data)
            reader_prefix = text.removeprefix("\ufeff").split("export")[0]
            point = diagnostics.bind_location(ref, data, {
                "path": ref["path"], "line": 1, "column": len(reader_prefix) + 1,
            })
            self.assertEqual(point.byte_offset, data.index(b"export"))
            self.assertEqual(point.utf16_offset, len(reader_prefix))

    def test_binding_checks_bytes_path_size_and_utf16_boundaries(self):
        data = "/* 🍎 */ export".encode()
        ref = reference(data)
        location = {"path": ref["path"], "line": 1, "column": 10}
        point = diagnostics.bind_location(ref, data, location)
        self.assertEqual(point.byte_offset, data.index(b"export"))
        self.assertEqual(point.utf16_offset, 9)
        for bad_ref, bad_data, bad_location in (
            ({**ref, "sha256": "sha256:" + "0" * 64}, data, location),
            ({**ref, "size": len(data) + 1}, data, location),
            (ref, data + b" ", location),
            (ref, data, {**location, "path": "other.ets"}),
            (ref, data, {**location, "line": 2}),
            (ref, data, {**location, "column": 5}),  # inside the surrogate pair
            (ref, data, {**location, "column": 999}),
            (ref, data, {**location, "line": True}),
            (reference(b"\xff"), b"\xff", location),
        ):
            with self.subTest(ref=bad_ref, location=bad_location), self.assertRaises(diagnostics.SourceBindingError):
                diagnostics.bind_location(bad_ref, bad_data, bad_location)
        for path in ("/absolute.ets", "../outside.ets", "a//b.ets", "a\\b.ets", "a\nb.ets"):
            with self.subTest(path=path), self.assertRaises(diagnostics.SourceBindingError):
                diagnostics.bind_location(reference(data, path), data, {**location, "path": path})

    def test_unknown_code_and_uncoded_reader_failure_are_distinct(self):
        data = b"export"
        ref = reference(data)
        with self.assertRaises(diagnostics.UnknownDiagnosticCode):
            diagnostics.explain("ARKTS_DECL_FUTURE")
        with self.assertRaises(diagnostics.UnknownDiagnosticCode):
            diagnostics.explain_reader_failure("author/Boundary.ets:1:1: ARKTS_DECL_FUTURE: future\n", ref, data)
        with self.assertRaises(diagnostics.UnsupportedReaderDiagnostic):
            diagnostics.explain_reader_failure("author/Boundary.ets:1:1: unsupported public type: AnyKeyword\n", ref, data)
        info = diagnostics.explain("ARKTS_DECL_CALLBACK_RESULT")
        with self.assertRaises(AttributeError):
            info.meaning = "silently accepted"

    def test_lookup_and_binding_do_not_probe_build_or_mutate_input(self):
        data = b"export"
        ref = reference(data)
        saved = ref.copy()
        message = "author/Boundary.ets:1:1: ARKTS_DECL_CALLBACK_RESULT: callback must return void\n"
        with patch("builtins.open", side_effect=AssertionError("unexpected I/O")), patch("pathlib.Path.open", side_effect=AssertionError("unexpected I/O")), patch("subprocess.run", side_effect=AssertionError("unexpected process")):
            result = diagnostics.explain_reader_failure(message, ref, data)
            self.assertEqual(result.explanation.code, "ARKTS_DECL_CALLBACK_RESULT")
            note = diagnostics.note_for_command_failure(RuntimeError("node failed: " + message), ref, data)
            self.assertIn(ref["sha256"], note)
            for text in (
                "node could not complete: timeout", "node failed: missing TypeScript",
                "node failed: " + message.replace("callback must return void", "changed meaning"),
                "node failed: " + message + message,
                "node failed: " + message.replace(":1:1:", ":" + "9" * 5000 + ":1:"),
            ):
                self.assertIsNone(diagnostics.note_for_command_failure(RuntimeError(text), ref, data))
        self.assertEqual(ref, saved)

    def test_actual_cli_failure_display_preserves_error_and_appends_notes(self):
        # Exercise the real __main__ catch, failing at argument parsing before I/O.
        error = RuntimeError("original reader failure")
        error.add_note("reviewed human explanation")
        output, errors = io.StringIO(), io.StringIO()
        with patch("argparse.ArgumentParser.parse_args", side_effect=error), redirect_stdout(output), redirect_stderr(errors), self.assertRaises(SystemExit) as raised:
            runpy.run_path(str(ROOT / "adapters/generators/arkts/driver.py"), run_name="__main__")
        self.assertEqual(raised.exception.code, 2)
        self.assertEqual(output.getvalue(), "")
        self.assertEqual(errors.getvalue(), "C07 declaration: original reader failure\nreviewed human explanation\n")


def reader_suite(config, observations):
    """Define tool-dependent cases only for the explicit integration command."""
    class ReaderTests(unittest.TestCase):
        def setUp(self):
            self.temp = tempfile.TemporaryDirectory(prefix="madb-l02-reader-")
            self.addCleanup(self.temp.cleanup)
            self.work = Path(self.temp.name)

        def invoke(self, data, logical, *, accepted):
            commands = []
            try:
                output = support.command(config, "node", [
                    ROOT / "adapters/generators/arkts/read_facade.cjs",
                    config["tools"]["typescript"]["path"], logical, "source",
                ], cwd=self.work, stdin=data, capture=commands)
            except RuntimeError as error:
                self.assertFalse(accepted, str(error))
                self.assertEqual(commands[-1]["exit_code"], 1)
                self.assertEqual(commands[-1]["stdout"], "")
                return error, commands[-1]["stderr"]
            else:
                self.assertTrue(accepted, "reader unexpectedly accepted negative input")
                return json.loads(output)
            finally:
                observations.append({"source": reference(data, logical), "commands": commands})

        def source(self, relative):
            return driver.render_template((ROOT / relative).read_bytes(), "libl02reader.so", "l02-reader-only")

        def test_real_descriptor_reuses_source_and_lowered_symbol_locations(self):
            logical = "sdk/arkts/facade/Boundary.ets.in"
            data = self.source(logical)
            descriptor = self.invoke(data, logical, accepted=True)
            self.assertEqual(descriptor["source"], reference(data, logical))
            policy = support.read_json(ROOT / "adapters/generators/arkts/profiles/snapshot-v1.json")
            contract, _ = driver.lowering.lower_public(descriptor, policy, {"selector": {}}, {
                "features": driver.lowering.FEATURES, "boundary_id": "c-arkts",
                "source_package": "l02-reader-only", "policy": "runtime-policy.json",
            })
            callables = descriptor["functions"] + [method for resource in descriptor["resources"] for method in resource["methods"]]
            by_name = {f"{item['owner']}.{item['name']}" if item["owner"] else item["name"]: item for item in callables}
            for symbol in contract["symbols"]:
                original = by_name[symbol["source_key"]["qualified_name"]]
                self.assertEqual(symbol["precision"], "compiler-derived")
                point = diagnostics.bind_location(descriptor["source"], data, symbol["location"])
                self.assertEqual(point.utf16_offset, original["location"]["start"])
                self.assertEqual(symbol["location"], {key: original["location"][key] for key in ("path", "line", "column")})

        def test_three_existing_negative_inputs_receive_exact_explanations(self):
            for fixture, code in (
                ("reject-async", "ARKTS_DECL_UNSUPPORTED_MODIFIERS"),
                ("reject-async-listener", "ARKTS_DECL_CALLBACK_RESULT"),
                ("reject-callback-result", "ARKTS_DECL_CALLBACK_RESULT"),
            ):
                logical = f"tests/fixtures/c07_arkts_generation/{fixture}/Boundary.ets.in"
                data = self.source(logical)
                with self.subTest(fixture=fixture):
                    error, stderr = self.invoke(data, logical, accepted=False)
                    bound = diagnostics.explain_reader_failure(stderr, reference(data, logical), data)
                    self.assertEqual(bound.explanation.code, code)
                    self.assertIn(code, diagnostics.note_for_command_failure(error, reference(data, logical), data))
                    self.assertIsNone(diagnostics.note_for_command_failure(error, reference(data + b" ", logical), data))
                    self.assertIsNone(diagnostics.note_for_command_failure(error, reference(data, "different.ets"), data))

        def test_real_non_bmp_and_line_break_positions_bind_to_original_bytes(self):
            for bom, separator in (
                ("", ""), ("", "\n"), ("", "\r\n"), ("", "\r"),
                ("", "\u2028"), ("", "\u2029"), ("\ufeff", ""), ("\ufeff\ufeff", ""),
                ("\ufeff", "\r\n"),
            ):
                prefix = bom + "/* header */" + separator + "/* 🍎 */ "
                data = (prefix + "export async function changed(): Promise<void> {}\n").encode()
                logical = "author/unicode.ets"
                with self.subTest(bom=repr(bom), separator=repr(separator)):
                    _, stderr = self.invoke(data, logical, accepted=False)
                    bound = diagnostics.explain_reader_failure(stderr, reference(data, logical), data)
                    self.assertEqual(bound.source.byte_offset, data.index(b"export"))
                    self.assertEqual(bound.source.utf16_offset, len(prefix.removeprefix("\ufeff").encode("utf-16-le")) // 2)

        def test_real_uncoded_rejection_is_not_assigned_an_invented_code(self):
            logical = "docs/examples/language-api/second-business.ets"
            data = (ROOT / logical).read_bytes()
            error, stderr = self.invoke(data, logical, accepted=False)
            self.assertIn("public declaration counts outside restricted profile", stderr)
            with self.assertRaises(diagnostics.UnsupportedReaderDiagnostic):
                diagnostics.explain_reader_failure(stderr, reference(data, logical), data)
            self.assertIsNone(diagnostics.note_for_command_failure(error, reference(data, logical), data))

        def test_actual_driver_adds_note_only_to_matching_real_command_failure(self):
            known = self.source("tests/fixtures/c07_arkts_generation/reject-async/Boundary.ets.in")
            uncoded = (ROOT / "docs/examples/language-api/second-business.ets").read_bytes()
            for data, mismatched, expects_note in ((known, False, True), (uncoded, False, False), (known, True, False)):
                logical = "author/Boundary.ets"
                path = self.work / logical
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(data)
                ref = reference(data + b" " if mismatched else data, logical)
                params = {**config, "provider": "kotlin", "variant": "ordinary", "public_source": logical}
                original_errors = []
                original_command = driver.support.command
                def capture_command(*args, **kwargs):
                    try:
                        return original_command(*args, **kwargs)
                    except RuntimeError as error:
                        original_errors.append(error)
                        raise
                with self.subTest(mismatched=mismatched, expects_note=expects_note), patch.object(driver, "verify", return_value={logical: ref}), patch.object(driver.support, "command", side_effect=capture_command), self.assertRaises(RuntimeError) as raised:
                    driver._read({}, params, self.work, self.work)
                self.assertIs(raised.exception, original_errors[0])
                self.assertTrue(str(raised.exception).startswith("node failed: " + logical + ":"))
                notes = getattr(raised.exception, "__notes__", [])
                self.assertEqual(len(notes), int(expects_note))
                if expects_note:
                    self.assertIn("L02 declaration hint [ARKTS_DECL_UNSUPPORTED_MODIFIERS]", notes[0])
                observations.append({"source": reference(data, logical), "driver_error": str(raised.exception), "notes": notes, "source_mismatch_injected": mismatched})

    return unittest.defaultTestLoader.loadTestsFromTestCase(ReaderTests)


def run_explicit_reader_checks():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--tool-observation", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        parser.error("report must be a new file")
    raw = args.tool_observation.read_bytes()
    observed = json.loads(raw)["config"]["tools"]
    config = {"tools": {name: observed[name] for name in ("node", "typescript")}}
    for item in config["tools"].values():
        support.verify_tool(item)
    version_commands = []
    with tempfile.TemporaryDirectory(prefix="madb-l02-versions-") as work:
        node_version = support.command(config, "node", ["--version"], cwd=work, capture=version_commands).decode().strip()
        compiler_version = support.command(config, "node", [
            "-e", "process.stdout.write(require(process.argv[1]).version)",
            config["tools"]["typescript"]["path"],
        ], cwd=work, capture=version_commands).decode().strip()
    if node_version != "v" + config["tools"]["node"]["version"]:
        raise RuntimeError("observed Node version differs from tool record")
    tracked = [ROOT / name for name in (
        "tests/integration/l02_diagnostics/test_declaration_diagnostics.py",
        "adapters/generators/arkts/read_facade.cjs",
        "adapters/generators/arkts/declaration_diagnostics.py",
        "adapters/generators/arkts/driver.py",
        "adapters/generators/arkts/support.py",
        "adapters/generators/arkts/lowering.py",
        "adapters/generators/arkts/profiles/snapshot-v1.json",
        "sdk/arkts/facade/Boundary.ets.in",
        "docs/examples/language-api/second-business.ets",
        *(f"tests/fixtures/c07_arkts_generation/{name}/Boundary.ets.in" for name in (
            "reject-async", "reject-async-listener", "reject-callback-result",
        )),
    )]
    sources = [reference(path.read_bytes(), str(path.relative_to(ROOT))) for path in tracked]
    observations = []
    suite = unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(BindingTests), reader_suite(config, observations)])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    for item in config["tools"].values():
        support.verify_tool(item)
    if sources != [reference(path.read_bytes(), str(path.relative_to(ROOT))) for path in tracked]:
        raise RuntimeError("reviewed source changed while checks ran")
    if args.tool_observation.read_bytes() != raw:
        raise RuntimeError("tool observation changed while checks ran")
    report = {
        "scope": "L02 host declaration/source binding and failure explanation; no SDK generation, target execution, Lab acceptance or public explain wire",
        "tool_observation": reference(raw, str(args.tool_observation)), "tools": config["tools"],
        "versions": {"node": node_version, "compiler_api": compiler_version},
        "version_commands": version_commands,
        "sources": sources, "observations": observations,
        "tests_run": result.testsRun, "successful": result.wasSuccessful(),
        "errors": len(result.errors), "failures": len(result.failures),
    }
    with args.report.open("x") as output:
        json.dump(report, output, indent=2, ensure_ascii=False)
        output.write("\n")
    raise SystemExit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    if "--tool-observation" in sys.argv:
        run_explicit_reader_checks()
    else:
        unittest.main()
