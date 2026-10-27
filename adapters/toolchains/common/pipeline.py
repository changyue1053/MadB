"""Shared frozen C04 metadata, generation, build and package orchestration."""
from copy import deepcopy
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile

from .documents import ref, save, unique, generation_task, task_digest, sha_file, profile_digest


EXTERNAL_RUNTIME_VERIFICATION = "core.external-runtime-verification@1"


def committed_build_evidence(metadata):
    """Retain the pinned batch's exact evidence references, never glob work files."""
    files = metadata.get("files")
    if not isinstance(files, list) or not files:
        raise ValueError("committed build metadata lacks its file inventory")
    evidence, seen = [], set()
    for item in files:
        if not isinstance(item, dict) or not isinstance(item.get("path"), str):
            raise ValueError("invalid committed build file reference")
        name = item["path"]
        if name in seen:
            raise ValueError("duplicate committed build file reference: " + name)
        seen.add(name)
        if not name.startswith("evidence/task-"):
            continue
        if not re.fullmatch(r"evidence/task-[0-9]{4}\.json", name):
            raise ValueError("malformed committed evidence number: " + name)
        if (not isinstance(item.get("sha256"), str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", item["sha256"])
                or type(item.get("size")) is not int or item["size"] < 0):
            raise ValueError("invalid committed evidence reference: " + name)
        evidence.append(dict(item, schema="madb.build-evidence/1"))
    evidence.sort(key=lambda item: item["path"])
    if not evidence:
        raise ValueError("committed build lacks its evidence inventory")
    if any(item["path"] != f"evidence/task-{index:04}.json" for index, item in enumerate(evidence)):
        raise ValueError("committed evidence inventory has a gap")
    return evidence



def load(path):
    return json.loads(Path(path).read_text())


def copy_refs(source, destination, references):
    for item in references:
        name = item["path"]
        path = source / name
        if path.stat().st_size != item["size"] or sha_file(path) != item["sha256"]:
            raise ValueError("frozen input changed: " + name)
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)


def verify_refs(source, references):
    for item in references:
        path = source / item["path"]
        if path.stat().st_size != item["size"] or sha_file(path) != item["sha256"]:
            raise ValueError("frozen input changed: " + item["path"])


class FrozenPipeline:
    """Route adapters supply helpers and public inventory, never host commit logic."""
    driver: str
    worker: str
    packer: str

    def public_generated(self):
        return self.outputs["files"]

    def public_built(self):
        raise NotImplementedError("the route must declare its public build output inventory")

    def build_phases(self):
        """The shared helper builds one complete linear chain, not an arbitrary DAG."""
        return ("compile", "link", "run")

    def __init__(self, repo, root, prepared, host_image):
        self.repo, self.root = repo, root
        self.config = prepared["config"]
        self.read = prepared["read"]
        self.lock = prepared["lock"]
        self.binary = Path(host_image)
        self.host_digest = sha_file(self.binary)
        self.host_size = self.binary.stat().st_size
        python = self.config["tools"]["python"]
        self.python_arguments = python.get("invocation_arguments", [])
        self.tool = {key: python[key] for key in ("id", "version", "sha256")}
        self.installed = dict(path=python["path"].lstrip("/"), sha256=python["sha256"], size=python["size"], schema="madb.tool-image/1")
        self.environment = [dict(name="PYTHONDONTWRITEBYTECODE", value="1")]
        self._generated_context = None
        (root / "runs").mkdir(exist_ok=True)

    def context(self, metadata=False):
        result = {key: ref(self.root, name, schema) for key, name, schema in [
            ("project", "project.json", "madb.project/1"), ("read_request", "read.json", "madb.read-request/1"),
            ("lock", "madb.lock", "madb.lock/1"), ("profile", "profile.json", "madb.profile/1")]}
        result["contract"] = None if metadata else ref(self.root, "contract.json", "madb.contract-ir/1")
        return result

    def common_inputs(self, metadata=False):
        result = list(self.read["inputs"]) + list(self.lock["schemas"])
        result += [ref(self.root, "madb.lock", "madb.lock/1"), ref(self.root, "profile.json", "madb.profile/1"),
                   ref(self.root, "project.json", "madb.project/1")]
        if not metadata:
            result.append(ref(self.root, "contract.json", "madb.contract-ir/1"))
        return unique([dict(item, schema=item.get("schema", "madb.source-input/1")) for item in result])

    def recipe(self, operation, request, inputs, response):
        if operation == "package":
            arguments = ["../inputs/" + self.packer]
        else:
            arguments = ["../inputs/" + self.driver, operation, "--config", "../inputs/backend.json"]
        return dict(id="native-" + operation, request=ref(self.root, request, "madb." + ("read-request" if operation == "metadata" else operation + "-request") + "/1"),
                    tool=self.tool, installed_tool=self.installed, arguments=self.python_arguments + arguments, environment=self.environment,
                    working_directory="work", response_path=response, inputs=unique(inputs))

    def bundle(self, operation, recipe=None, tasks=None):
        return dict(schema="madb.execution/1", operation=operation, context=self.context(operation == "metadata"),
                    recipe=recipe, tasks=tasks or [], store="domain", run_parent="runs", cache=None,
                    watch=dict(absent=[], directories=[]), dependencies=None)

    def owner_environment(self):
        """Routes may isolate owner launch without duplicating the watchdog."""
        return None

    def execute(self, command, bundle, *extra):
        save(self.root, command + "-execution.json", bundle)
        args = [str(self.binary), command, "--root", str(self.root), "--execution", command + "-execution.json",
                "--frozen", "--tool-root", "/", *extra]
        timeout = self.config.get("phase_timeout_seconds")
        if timeout is not None:
            if type(timeout) is not int or not 1 <= timeout <= 3600:
                raise ValueError("phase timeout must be between 1 and 3600 seconds")
            args.extend(["--process-timeout-seconds", str(timeout)])
        grace = self.config.get("phase_termination_grace_ms")
        if grace is not None:
            if type(grace) is not int or not 0 <= grace <= 30000:
                raise ValueError("phase termination grace must be between 0 and 30000 milliseconds")
            args.extend(["--process-termination-grace-ms", str(grace)])
        # The owner applies each process deadline. This outer watchdog allows the
        # full graph plus cleanup, and first interrupts the owner cooperatively.
        budget = (timeout or 300) * max(1, len(bundle.get("tasks", []))) + 60
        self.verify_host()
        process = subprocess.Popen(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   env=self.owner_environment())
        try:
            stdout, stderr = process.communicate(timeout=budget)
        except subprocess.TimeoutExpired as error:
            process.send_signal(signal.SIGINT)
            try:
                stdout, stderr = process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                try:
                    stdout, stderr = process.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    stdout, stderr = "", "Owner cleanup could not be confirmed; retain the execution workspace."
                    process.stdout.close()
                    process.stderr.close()
            (self.root / (command + "-execution.log")).write_text(stdout + stderr)
            raise RuntimeError("MadB owner exceeded the graph budget; inspect the retained execution workspace") from error
        result = subprocess.CompletedProcess(args, process.returncode, stdout, stderr)
        self.verify_host()
        (self.root / (command + "-execution.log")).write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError("MadB " + command + " rejected the route:\n" + result.stdout + result.stderr)
        return result.stdout

    def verify_host(self):
        if self.binary.stat().st_size != self.host_size or sha_file(self.binary) != self.host_digest:
            raise RuntimeError("locked MadB host image changed during the run")

    def layer(self, layer):
        index = load(self.root / "domain/current.json")
        return self.root / "domain/batches" / index[layer].removeprefix("sha256:") / "files"

    def plan_metadata(self):
        """Prepare the existing execution document; only the Host may execute it."""
        recipe = self.recipe("metadata", "read.json", self.common_inputs(True), "contract.json")
        return self.bundle("metadata", recipe)

    def metadata(self):
        output = self.execute("build", self.plan_metadata(), "--task", "metadata")
        manifest = next(line.split(":", 1)[1].strip() for line in output.splitlines() if line.startswith("Manifest:"))
        shutil.copyfile(manifest, self.root / "contract.json")
        self.contract = load(self.root / "contract.json")

    def plan_generation(self):
        """Prepare a generation without invoking another MadB process."""
        request = deepcopy(self.read)
        request.update(schema="madb.generate-request/1", contract=ref(self.root, "contract.json", "madb.contract-ir/1"),
                       input_digest=self.contract["source_digest"], task_digest="sha256:" + "0" * 64,
                       contract_digest=self.contract["structure_digest"], policy_digest=self.contract["policy_digest"],
                       output_root="staging/generate")
        save(self.root, "generate-request.json", request)
        recipe = self.recipe("generate", "generate-request.json", self.common_inputs(), "outputs.json")
        request["task_digest"] = generation_task(request, recipe)
        save(self.root, "generate-request.json", request)
        recipe["request"] = ref(self.root, "generate-request.json", "madb.generate-request/1")
        return self.bundle("generate", recipe)

    def generate(self):
        execution = self.plan_generation()
        self.execute("generate", execution)
        self.accept_generation(execution["recipe"]["inputs"])

    def accept_generation(self, preparation_inputs):
        """Retain a Host-committed generation for checked build preparation.

        This restores preparation data only, never a commit capability. The
        caller must have completed the matching Host generation. Preparation
        and the subsequent Host execution independently validate its lineage.
        """
        self.source = self.layer("source")
        self.outputs = load(self.source / "outputs.json")
        # Only a successful host generation establishes this preparation context.
        # Rust remains the authority for schema/capability checks and commits.
        self._generated_context = dict(
            selector=deepcopy(self.read["selector"]), source=self.source,
            profile=ref(self.root, "profile.json", "madb.profile/1"),
            contract=ref(self.root, "contract.json", "madb.contract-ir/1"),
            output_set=ref(self.source, "outputs.json"),
            preparation_inputs=deepcopy(preparation_inputs),
            metadata_digest=sha_file(self.source.parent / "metadata.json"))

    def checked_build_phases(self):
        context = self._generated_context
        if context is None:
            raise ValueError("build preparation requires a successful host generation")
        if (self.read["selector"] != context["selector"] or self.source != context["source"]
                or self.source != self.layer("source")):
            raise ValueError("build preparation context differs from the committed generation")
        if (self.read["profile"] != context["profile"]
                or ref(self.root, "profile.json", "madb.profile/1") != context["profile"]):
            raise ValueError("build preparation profile differs from the read profile reference")
        if (ref(self.root, "contract.json", "madb.contract-ir/1") != context["contract"]
                or load(self.root / "contract.json") != self.contract):
            raise ValueError("build preparation contract differs from the host-checked context")
        if (ref(self.source, "outputs.json") != context["output_set"]
                or sha_file(self.source.parent / "metadata.json") != context["metadata_digest"]
                or load(self.source / "outputs.json") != self.outputs):
            raise ValueError("build preparation source generation identity changed")
        profile = load(self.root / "profile.json")
        digest = profile_digest(profile)
        metadata = load(self.source.parent / "metadata.json")
        specification = metadata.get("spec", {})
        if (digest != self.read["selector"]["profile_digest"]
                or self.contract["selector"] != self.read["selector"]
                or metadata.get("schema") != "madb.storage-batch/1"
                or metadata.get("id") != "sha256:" + self.source.parent.name
                or metadata.get("files", []).count(context["output_set"]) != 1
                or specification.get("layer") != "source"
                or specification.get("profile_digest") != digest
                or specification.get("input_digest") != self.contract["source_digest"]
                or specification.get("generation_id") != self.outputs["generation_id"]
                or specification.get("task_digest") != self.outputs["task_digest"]
                or self.outputs["profile_digest"] != digest
                or self.outputs["input_digest"] != self.contract["source_digest"]):
            raise ValueError("build preparation profile/source lineage does not match")
        external = EXTERNAL_RUNTIME_VERIFICATION in profile["required_features"]
        if external and EXTERNAL_RUNTIME_VERIFICATION not in profile["supported_features"]:
            raise ValueError("external runtime verification is required but not supported")
        expected = ("compile", "link") if external else ("compile", "link", "run")
        phases = tuple(self.build_phases())
        if phases != expected:
            raise ValueError("build phase plan must match the profile's complete linear chain: " + ", ".join(expected))
        return phases

    def plan_build(self):
        # Preparation executes code, including at import time. Validate the whole
        # plan and its already host-checked lineage before loading any helper.
        phases = self.checked_build_phases()
        self.verify_host()
        base = unique(self.common_inputs())
        if base != self._generated_context["preparation_inputs"]:
            raise ValueError("preparation inputs differ from the host-checked generation")
        if self.worker not in {item["path"] for item in base}:
            raise ValueError("preparation worker is not a host-checked input")
        verify_refs(self.root, base)
        generated = [{key: item[key] for key in ("path", "sha256", "size")} for item in self.outputs["files"]]
        verify_refs(self.source, generated)
        # Explicit preparation discovers native output identities required by the
        # static C04 DAG. C04 then recompiles independently and verifies every byte.
        # Preparation is never used as acceptance evidence or a semantic oracle.
        spec = importlib.util.spec_from_file_location("madb_native_worker", self.root / self.worker)
        worker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(worker)
        previous = []
        previous_root = None
        tasks = []
        # Each preparation attempt retains its own diagnostics. Rebuilding the
        # same managed root must neither reuse stale previews nor delete them.
        prepare_root = self.root / "prepare"
        prepare_root.mkdir(exist_ok=True)
        attempt = Path(tempfile.mkdtemp(prefix="build-", dir=prepare_root))
        for index, phase in enumerate(phases):
            dependencies = [] if index == 0 else [phases[index - 1]]
            preview = attempt / phase
            inputs, work = preview / "inputs", preview / "work"
            inputs.mkdir(parents=True)
            work.mkdir()
            copy_refs(self.root, inputs, base)
            copy_refs(self.source, inputs, generated)
            if previous:
                copy_refs(previous_root, inputs, previous)
            output_names = worker.run_phase(self.config, phase, inputs, work)
            task_inputs = unique(base + generated + previous)
            for item in task_inputs:
                item.pop("schema", None)
            task = dict(schema="madb.build-task/1", id=phase, selector=deepcopy(self.read["selector"]),
                        dependencies=dependencies, input_digest=self.contract["source_digest"],
                        task_digest="sha256:" + "0" * 64, generation_id=self.outputs["generation_id"], tool=self.tool,
                        arguments=self.python_arguments + ["../inputs/" + self.worker, "--config", "../inputs/backend.json"],
                        environment=self.environment, working_directory="work", phase=phase,
                        inputs=task_inputs, outputs=output_names)
            task["task_digest"] = task_digest(task)
            save(self.root, "tasks/" + phase + ".json", task)
            tasks.append(dict(request=ref(self.root, "tasks/" + phase + ".json", "madb.build-task/1"), installed_tool=self.installed))
            previous = [ref(work, name) for name in output_names]
            previous_root = work
        return self.bundle("build", tasks=tasks)

    def build(self):
        self.execute("build", self.plan_build())
        self.built = self.layer("build")

    def plan_package(self, producer_revision, required_cases, provisional=False):
        build_metadata = load(self.built.parent / "metadata.json")
        evidence = committed_build_evidence(build_metadata)
        request = dict(schema="madb.package-request/1", selector=self.read["selector"],
                       output_set=ref(self.source, "outputs.json", "madb.output-set/1"), build_evidence=evidence,
                       lock=ref(self.root, "madb.lock", "madb.lock/1"), contract_digest=self.contract["structure_digest"],
                       policy_digest=self.contract["policy_digest"], generation_id=self.outputs["generation_id"],
                       dependencies=[], public_symbols=sorted(s["public_id"] for s in self.contract["symbols"]),
                       required_cases=sorted(required_cases), output_root="staging/package")
        save(self.root, "package-request.json", request)
        save(self.root, "provenance.json", dict(schema="madb.native-provenance/1", producer_revision=producer_revision,
              input_digest=self.contract["source_digest"], tool_digest=self.host_digest, provisional=provisional))
        for name in ("LICENSE", "NOTICE"):
            # Installed recipes already carry these frozen bytes in their
            # private root. Producer workspaces still copy the source files.
            if (self.repo / name).resolve() != (self.root / name).resolve():
                shutil.copyfile(self.repo / name, self.root / name)
        names = ["toolchain.json", "profile.json", "project.json", "madb.lock", "contract.json", "provenance.json", "LICENSE", "NOTICE"]
        inventory = [ref(self.root, name) for name in names]
        inventory += [ref(self.source, item["path"]) for item in self.public_generated()]
        inventory += [ref(self.built, name) for name in self.public_built()] + evidence
        save(self.root, "package-files.json", inventory)
        build_inputs = [dict(item) for item in build_metadata["files"]]
        source_inputs = [ref(self.source, item["path"]) for item in self.outputs["files"]]
        inputs = unique(self.common_inputs() + inventory + build_inputs + source_inputs +
                        [request["output_set"], ref(self.root, "package-files.json")])
        inputs = [dict(item, schema=item.get("schema", "madb.source-input/1")) for item in inputs]
        recipe = self.recipe("package", "package-request.json", inputs, "candidate.json")
        return self.bundle("package", recipe)

    def package(self, destination, producer_revision, required_cases, provisional=False):
        self.execute("package", self.plan_package(producer_revision, required_cases, provisional))
        candidate = self.layer("candidate")
        manifest = load(candidate / "candidate.json")
        destination.mkdir()
        copy_refs(candidate, destination, manifest["files"])
        shutil.copyfile(candidate / "candidate.json", destination / "candidate.json")
        return manifest
