"""Shared frozen C04 metadata, generation, build and package orchestration."""
from copy import deepcopy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess

from .documents import ref, save, unique, generation_task, task_digest, sha_file



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


class FrozenPipeline:
    """Route adapters supply helpers and public inventory, never host commit logic."""
    driver: str
    worker: str
    packer: str

    def public_generated(self):
        return self.outputs["files"]

    def public_built(self):
        raise NotImplementedError("the route must declare its public build output inventory")

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
        process = subprocess.Popen(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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

    def metadata(self):
        recipe = self.recipe("metadata", "read.json", self.common_inputs(True), "contract.json")
        output = self.execute("build", self.bundle("metadata", recipe), "--task", "metadata")
        manifest = next(line.split(":", 1)[1].strip() for line in output.splitlines() if line.startswith("Manifest:"))
        shutil.copyfile(manifest, self.root / "contract.json")
        self.contract = load(self.root / "contract.json")

    def generate(self):
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
        self.execute("generate", self.bundle("generate", recipe))
        self.source = self.layer("source")
        self.outputs = load(self.source / "outputs.json")

    def build(self):
        # Explicit preparation discovers native output identities required by the
        # static C04 DAG. C04 then recompiles independently and verifies every byte.
        # Preparation is never used as acceptance evidence or a semantic oracle.
        spec = importlib.util.spec_from_file_location("madb_native_worker", self.root / self.worker)
        worker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(worker)
        previous = []
        previous_root = None
        tasks = []
        base = self.common_inputs()
        generated = [ref(self.source, item["path"]) for item in self.outputs["files"]]
        for phase, dependencies in [("compile", []), ("link", ["compile"]), ("run", ["link"])]:
            preview = self.root / "prepare" / phase
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
        self.execute("build", self.bundle("build", tasks=tasks))
        self.built = self.layer("build")

    def package(self, destination, producer_revision, required_cases, provisional=False):
        evidence = [ref(self.built, f"evidence/task-{index:04}.json", "madb.build-evidence/1") for index in range(3)]
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
            shutil.copyfile(self.repo / name, self.root / name)
        names = ["toolchain.json", "profile.json", "project.json", "madb.lock", "contract.json", "provenance.json", "LICENSE", "NOTICE"]
        inventory = [ref(self.root, name) for name in names]
        inventory += [ref(self.source, item["path"]) for item in self.public_generated()]
        inventory += [ref(self.built, name) for name in self.public_built()] + evidence
        save(self.root, "package-files.json", inventory)
        build_metadata = load(self.built.parent / "metadata.json")
        build_inputs = [ref(self.built, item["path"]) for item in build_metadata["files"]]
        source_inputs = [ref(self.source, item["path"]) for item in self.outputs["files"]]
        inputs = unique(self.common_inputs() + inventory + build_inputs + source_inputs +
                        [request["output_set"], ref(self.root, "package-files.json")])
        inputs = [dict(item, schema=item.get("schema", "madb.source-input/1")) for item in inputs]
        recipe = self.recipe("package", "package-request.json", inputs, "candidate.json")
        self.execute("package", self.bundle("package", recipe))
        candidate = self.layer("candidate")
        manifest = load(candidate / "candidate.json")
        destination.mkdir()
        copy_refs(candidate, destination, manifest["files"])
        shutil.copyfile(candidate / "candidate.json", destination / "candidate.json")
        return manifest
