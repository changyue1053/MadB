"""Formal CLI subprocess acceptance; inspect on-disk files independently of reports."""
import json
from pathlib import Path
import tempfile
import os
import shutil
import subprocess
import signal

from cli_fixture import CliWorkspace, HEADER, IMPLEMENTATION, canonical, generation_identity, candidate_identity
from support import PackageFixture, build_binary, tree_bytes, wait_json


class FormalExecution(PackageFixture):
    @classmethod
    def setUpClass(cls):
        cls.binary = build_binary("madb")

    def setUp(self):
        super().setUp()
        self.marker_temp = tempfile.TemporaryDirectory(prefix="madb-cli-markers-")
        self.marker = Path(self.marker_temp.name) / "calls.jsonl"
        self.fixture = CliWorkspace(self.root, self.marker)

    def tearDown(self):
        # Fixture peers never fork background work; all owned CLI processes have finished.
        for call in self.calls():
            run = Path(call["cwd"]).parent
            if run.parent == Path(tempfile.gettempdir()).resolve() and run.name.startswith("madb-"):
                for parent, dirs, files in os.walk(run):
                    Path(parent).chmod(0o700)
                    for name in files:
                        (Path(parent) / name).chmod(0o600)
                shutil.rmtree(run)
        super().tearDown()
        self.marker_temp.cleanup()

    def execute(self, operation, check=False, expected=0, bundle=None, extra=()):
        arguments = [str(self.binary), operation, "--root", str(self.root), "--execution", bundle or operation + ".json", "--frozen", *extra]
        if check:
            arguments.append("--check")
        return self.finish(self.spawn(arguments), expected=expected)

    def calls(self):
        return [json.loads(line) for line in self.marker.read_text().splitlines()] if self.marker.exists() else []

    def generated(self):
        out, err = self.execute("generate")
        self.assertIn("Committed", out)
        self.assertEqual(err, "")
        files = self.fixture.layer("source")
        self.assertEqual((files / "include/api.h").read_bytes(), HEADER)
        self.assertEqual((files / "src/api.c").read_bytes(), IMPLEMENTATION)
        actual = json.loads((files / "outputs.json").read_text())
        self.assertEqual(actual, self.fixture.generation_expected())
        self.assertEqual(actual["generation_id"], generation_identity(actual))
        return files

    def test_generate_commits_independently_expected_bytes_and_identity(self):
        self.generated()
        self.assertEqual([call["operation"] for call in self.calls()], ["generate"])

    def test_check_recomputes_in_distinct_private_run_roots_without_changing_workspace(self):
        self.generated()
        before = tree_bytes(self.root)
        self.execute("generate", check=True)
        self.execute("generate", check=True)
        self.assertEqual(tree_bytes(self.root), before)
        calls = self.calls()
        self.assertEqual(len(calls), 3)
        self.assertEqual(len({call["cwd"] for call in calls}), 3)
        self.assertTrue(all(not Path(call["cwd"]).is_relative_to(self.root) for call in calls[1:]))

    def test_check_without_current_generation_fails_without_creating_domain_or_cache(self):
        # Existing domain is the empty fixture directory, no current batch exists.
        before = tree_bytes(self.root)
        self.execute("generate", check=True, expected=1)
        self.assertEqual(tree_bytes(self.root), before)

    def check_corruption(self, mutate):
        files = self.generated()
        mutate(files)
        before = tree_bytes(self.root)
        _, error = self.execute("generate", check=True, expected=4)
        self.assertIn("STORE004", error)
        self.assertEqual(tree_bytes(self.root), before)

    def test_check_detects_modified_output_without_repair(self):
        def modify(files):
            path = files / "include/api.h"
            path.chmod(0o600)
            path.write_bytes(b"int stale(void);\n")
        self.check_corruption(modify)

    def test_check_detects_missing_output_without_repair(self):
        def remove(files):
            (files / "include").chmod(0o700)
            (files / "include/api.h").unlink()
        self.check_corruption(remove)

    def test_check_detects_extra_output_without_repair(self):
        def extra(files):
            files.chmod(0o700)
            (files / "unexpected.txt").write_bytes(b"extra")
        self.check_corruption(extra)

    def test_check_detects_registration_drift_with_identical_header_and_implementation(self):
        self.generated()
        behavior = json.loads((self.root / "behavior.json").read_text())
        behavior["sources"] = behavior["sources"][:1]
        (self.root / "behavior.json").write_bytes(canonical(behavior) + b"\n")
        self.fixture.refresh_generation()
        before = tree_bytes(self.root)
        self.execute("generate", check=True, expected=1)
        self.assertEqual(tree_bytes(self.root), before)
        self.assertEqual((self.fixture.layer("source") / "include/api.h").read_bytes(), HEADER)

    def test_query_missing_metadata_is_read_only_and_never_invokes_recipe(self):
        self.fixture.metadata()
        before = tree_bytes(self.root)
        for command in ["inspect", "check", "plan"]:
            result = self.finish(self.spawn([str(self.binary), command, "--root", str(self.root), "--project", "project.json", "--request", "read.json", "--snapshot", "contract.json"]), expected=2)
            self.assertIn("MetadataRequired", result[0])
            self.assertEqual(self.calls(), [])
            self.assertEqual(tree_bytes(self.root), before)

    def test_explicit_metadata_preparation_works_without_existing_ir(self):
        self.fixture.metadata()
        out, err = self.execute("build", bundle="metadata.json", extra=("--task", "metadata"))
        self.assertIn("MetadataPrepared", out)
        self.assertEqual(err, "")
        lines = dict(line.split(":", 1) for line in out.splitlines() if ":" in line)
        prepared = Path(lines["Manifest"].strip())
        self.assertEqual(json.loads(prepared.read_text()), self.fixture.values["contract-ir"])
        self.assertFalse((self.root / "domain/current.json").exists())
        self.assertEqual([call["operation"] for call in self.calls()], ["metadata"])

    def test_missing_locked_local_tool_never_downloads_or_invokes_helper(self):
        self.generated()
        (self.root / "tools/python").unlink()
        before = tree_bytes(self.root)
        out, err = self.execute("generate", check=True, expected=2)
        self.assertTrue("prepare" in (out + err).lower() or "missing" in (out + err).lower(), (out, err))
        self.assertEqual(len(self.calls()), 1)
        self.assertEqual(tree_bytes(self.root), before)

    def test_declared_output_inventory_rejects_unregistered_file_without_commit(self):
        behavior = json.loads((self.root / "behavior.json").read_text())
        behavior["undeclared"] = True
        (self.root / "behavior.json").write_bytes(canonical(behavior) + b"\n")
        self.fixture.refresh_generation()
        _, err = self.execute("generate", expected=2)
        self.assertIn("EXEC019", err)
        self.assertFalse((self.root / "domain/current.json").exists())

    def test_compile_link_run_package_and_independent_native_consumer(self):
        source = self.generated()
        self.fixture.build()
        out, err = self.execute("build")
        self.assertIn("Committed", out)
        self.assertEqual(err, "")
        built = self.fixture.layer("build")
        for index, task in enumerate(self.fixture.tasks):
            evidence = json.loads((built / f"evidence/task-{index:04}.json").read_text())
            self.assertEqual(evidence["task_id"], task["id"])
            self.assertEqual(evidence["task_digest"], task["task_digest"])
            self.assertEqual((evidence["status"], evidence["exit_code"]), ("passed", 0))
        self.fixture.package()
        out, err = self.execute("package")
        self.assertIn("Committed", out)
        self.assertEqual(err, "")
        packaged = self.fixture.layer("candidate")
        candidate = json.loads((packaged / "candidate.json").read_text())
        self.assertEqual(candidate["candidate_digest"], candidate_identity(candidate))
        self.assertEqual(candidate["generation_id"], self.fixture.generation_expected()["generation_id"])
        self.assertEqual((packaged / "include/api.h").read_bytes(), HEADER)
        self.assertEqual((packaged / "native/api-linked.o").read_bytes(), (built / "native/api-linked.o").read_bytes())
        # A fresh downstream compilation includes only packaged declarations and links its object.
        consumer = self.root / "independent-consumer.c"
        consumer.write_bytes(b'#include "api.h"\nint main(void) { return madb_answer() == 42 ? 0 : 91; }\n')
        executable = self.root / "independent-consumer"
        subprocess.run([self.fixture.compiler, *self.fixture.link_flags, str(consumer), "-I", str(packaged / "include"), str(packaged / "native/api-linked.o"), "-o", str(executable)], check=True, capture_output=True, timeout=20)
        subprocess.run([str(executable)], check=True, capture_output=True, timeout=10)
        index = json.loads((self.domain / "current.json").read_text())
        build_metadata = json.loads((built.parent / "metadata.json").read_text())
        candidate_metadata = json.loads((packaged.parent / "metadata.json").read_text())
        self.assertEqual(index["revision"], 3)
        self.assertEqual(build_metadata["spec"]["parent"], index["source"])
        self.assertEqual(candidate_metadata["spec"]["parent"], index["build"])
        self.assertEqual(source, self.fixture.layer("source"))
        self.assertEqual([call["operation"] for call in self.calls()], ["generate", "compile", "link", "run", "package"])

    def test_named_compile_subgraph_executes_without_publishing_incomplete_build(self):
        self.generated()
        self.fixture.build()
        before = (self.domain / "current.json").read_bytes()
        out, err = self.execute("build", extra=("--task", "compile"))
        self.assertIn("Executed", out)
        self.assertEqual(err, "")
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual([call["operation"] for call in self.calls()], ["generate", "compile"])

    def test_check_does_not_create_a_missing_cache_directory(self):
        self.generated()
        (self.root / "cache").rmdir()
        before = tree_bytes(self.root)
        self.execute("generate", check=True)
        self.assertEqual(tree_bytes(self.root), before)
        self.assertFalse((self.root / "cache").exists())

    def cancel_running_cli(self, cancellation_signal):
        self.generated()
        before = (self.domain / "current.json").read_bytes()
        source_before = tree_bytes(self.fixture.layer("source"))
        ready = Path(self.marker_temp.name) / "helper-ready.json"
        behavior = json.loads((self.root / "behavior.json").read_text())
        behavior["wait_ready"] = str(ready)
        (self.root / "behavior.json").write_bytes(canonical(behavior) + b"\n")
        self.fixture.refresh_generation()
        process = self.spawn([str(self.binary), "generate", "--root", str(self.root), "--execution", "generate.json", "--frozen"])
        helper = wait_json(ready, process)
        self.assertEqual(helper["phase"], "before-output")
        self.assertEqual(helper["parent"], process.pid)
        process.send_signal(cancellation_signal)
        out, err = process.communicate(timeout=15)
        self.assertGreater(process.returncode, 0, (out, err))
        self.assertIn("Cancelled", err)
        self.assertIn("RETAINED", err)
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual(tree_bytes(self.fixture.layer("source")), source_before)
        with self.assertRaises(ProcessLookupError):
            os.kill(helper["pid"], 0)

    def test_sigint_cancels_running_cli_and_reaps_child_before_rejecting_commit(self):
        self.cancel_running_cli(signal.SIGINT)

    def test_sigterm_cancels_running_cli_and_reaps_child_before_rejecting_commit(self):
        self.cancel_running_cli(signal.SIGTERM)

    def test_failed_generated_syntax_check_never_replaces_valid_source(self):
        self.generated()
        before = (self.domain / "current.json").read_bytes()
        source_before = tree_bytes(self.fixture.layer("source"))
        behavior = json.loads((self.root / "behavior.json").read_text())
        behavior["invalid_syntax"] = True
        (self.root / "behavior.json").write_bytes(canonical(behavior) + b"\n")
        self.fixture.refresh_generation()
        _, err = self.execute("generate", expected=2)
        self.assertIn("RETAINED", err)
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual(tree_bytes(self.fixture.layer("source")), source_before)
        validations = [json.loads(line) for line in Path(str(self.marker) + ".validation.jsonl").read_text().splitlines()]
        self.assertEqual(len(validations), 2)
        self.assertEqual(validations[0]["exit_code"], 0)
        self.assertNotEqual(validations[1]["exit_code"], 0)
        self.assertIn("-fsyntax-only", validations[1]["arguments"])
        self.assertTrue(validations[1]["stderr"])
        locked = next(tool for tool in self.fixture.values["lock"]["tools"] if tool["id"] == "fixture-clang")
        self.assertEqual(validations[1]["sha256"], locked["sha256"])

    def test_build_failure_preserves_process_diagnostic_retained_path_and_current_batch(self):
        self.generated()
        self.fixture.build(mode="fail-build")
        before = (self.domain / "current.json").read_bytes()
        source_before = tree_bytes(self.fixture.layer("source"))
        _, err = self.execute("build", expected=2)
        self.assertIn("PROCESS at compile:", err)
        self.assertIn("27", err)
        retained = next(line.split(": ", 1)[1] for line in err.splitlines() if line.startswith("RETAINED at compile:"))
        self.assertTrue(Path(retained).is_dir())
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual(tree_bytes(self.fixture.layer("source")), source_before)
        self.assertEqual([call["operation"] for call in self.calls()], ["generate", "compile"])

    def test_build_cancellation_preserves_process_diagnostic_and_reaps_owned_child(self):
        self.generated()
        ready = Path(self.marker_temp.name) / "build-ready.json"
        self.fixture.build(mode="wait-build", ready=ready)
        before = (self.domain / "current.json").read_bytes()
        source_before = tree_bytes(self.fixture.layer("source"))
        process = self.spawn([str(self.binary), "build", "--root", str(self.root), "--execution", "build.json", "--frozen"])
        helper = wait_json(ready, process)
        self.assertEqual(helper["phase"], "before-compile")
        self.assertEqual(helper["parent"], process.pid)
        process.send_signal(signal.SIGTERM)
        out, err = process.communicate(timeout=15)
        self.assertGreater(process.returncode, 0, (out, err))
        self.assertIn("Cancelled", err)
        self.assertIn("PROCESS at compile:", err)
        self.assertIn("Cancellation", err)
        retained = next(line.split(": ", 1)[1] for line in err.splitlines() if line.startswith("RETAINED at compile:"))
        self.assertTrue(Path(retained).is_dir())
        self.assertEqual((self.domain / "current.json").read_bytes(), before)
        self.assertEqual(tree_bytes(self.fixture.layer("source")), source_before)
        self.assertEqual([call["operation"] for call in self.calls()], ["generate", "compile"])
        with self.assertRaises(ProcessLookupError):
            os.kill(helper["pid"], 0)
