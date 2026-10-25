"""Independent formal CLI fixtures. Protocol peers never mint their own expected oracle."""
from copy import deepcopy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

from support import ROOT, byte_digest
from dependency_fixture import host_target

spec = importlib.util.spec_from_file_location("c04_package_tasks", ROOT / "tests/integration/c04_tasks/fixture.py")
tasks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tasks)
query = tasks.query
canonical, semantic, save = query.canonical, query.semantic, query.save
HEADER = b"int madb_answer(void);\n"
IMPLEMENTATION = b'#include "api.h"\nint madb_answer(void) { return 42; }\n'
CONSUMER = b"int madb_answer(void); int main(void) { return madb_answer() == 42 ? 0 : 77; }\n"


def ref(root, name, schema="madb.source-input/1"):
    return query.ref(root, name, schema)


def generation_task(request, recipe):
    request = deepcopy(request)
    request.pop("task_digest", None)
    request["inputs"].sort(key=lambda item: item["path"])
    execution = {key: deepcopy(recipe[key]) for key in ["id", "tool", "arguments", "environment", "working_directory", "response_path", "inputs"]}
    execution["environment"].sort(key=lambda item: item["name"])
    execution["inputs"].sort(key=lambda item: item["path"])
    return semantic("madb.task/1", dict(operation="generate", request=request, execution=execution))


def generation_identity(manifest):
    values = {key: deepcopy(manifest[key]) for key in ["task_digest", "input_digest", "profile_digest", "files"]}
    values["files"].sort(key=lambda item: item["path"])
    for item in values["files"]:
        item["sources"].sort()
    return semantic("madb.generation/1", values)


def candidate_identity(manifest):
    value = deepcopy(manifest)
    value.pop("candidate_digest")
    for key in ["files", "dependencies", "build_evidence"]:
        value[key].sort(key=lambda item: item["path"])
    for key in ["public_symbols", "required_cases"]:
        value[key].sort()
    return semantic("madb.candidate/1", value)


class CliWorkspace:
    def __init__(self, root, marker):
        self.root, self.marker = root, marker
        self.values = tasks.materialize(root, marker)
        shutil.copyfile(Path(__file__).with_name("protocol_helper.py"), root / "protocol_helper.py")
        (root / "tools").mkdir()
        shutil.copyfile(tasks.PYTHON, root / "tools/python")
        (root / "tools/python").chmod(0o700)
        for name in ["runs", "cache"]:
            (root / name).mkdir()
        self.install_compiler()
        self.tool = {key: self.values["profile"]["tools"][0][key] for key in ["id", "version", "sha256"]}
        self.installed = ref(root, "tools/python", "madb.tool-image/1")
        contract = self.values["contract-ir"]
        self.symbols = sorted(item["public_id"] for item in contract["symbols"])
        save(root, "behavior.json", dict(sources=self.symbols))
        self.refresh_generation()

    def install_compiler(self):
        compiler = shutil.which("clang")
        if os.uname().sysname == "Darwin":
            compiler = subprocess.check_output(["/usr/bin/xcrun", "--find", "clang"], text=True).strip()
        if not compiler:
            raise RuntimeError("required C04 native Clang fixture compiler is unavailable")
        self.compiler = str(Path(compiler).resolve())
        self.link_flags = ["-Wl,--build-id=none"]
        if os.uname().sysname == "Darwin":
            sdk = subprocess.check_output(["/usr/bin/xcrun", "--show-sdk-path"], text=True).strip()
            self.link_flags = ["-isysroot", sdk]
        compiler_hash = byte_digest(Path(self.compiler).read_bytes())
        save(self.root, "compiler.json", dict(path=self.compiler, sha256=compiler_hash, link_flags=self.link_flags))
        tool = dict(self.values["profile"]["tools"][0], id="fixture-clang", version="fixture-native-compiler",
                    sha256=compiler_hash, source="fixture://resolved-clang-image")
        self.values["profile"]["tools"].append(tool)
        self.values["lock"]["tools"].append(deepcopy(tool))
        target = {"arm64-apple-macos11.0": "aarch64-apple-darwin", "x86_64-apple-macos10.15": "x86_64-apple-darwin"}.get(host_target(), host_target())
        self.values["profile"]["target"] = target
        self.values["project"]["boundaries"][0]["target"] = target
        for kind in ["read-request", "contract-ir", "build-task"]:
            self.values[kind]["selector"]["target"] = target
        for locked_tool in self.values["profile"]["tools"] + self.values["lock"]["tools"]:
            locked_tool["host"] = target
            locked_tool["target"] = target
        tasks.persist(self.root, self.values)

    def context(self, metadata=False):
        result = {key: ref(self.root, name, schema) for key, name, schema in [
            ("project", "project.json", "madb.project/1"), ("read_request", "read.json", "madb.read-request/1"),
            ("lock", "madb.lock", "madb.lock/1"), ("profile", "profiles/rust-c.json", "madb.profile/1")]}
        result["contract"] = None if metadata else ref(self.root, "contract.json", "madb.contract-ir/1")
        return result

    def common_inputs(self, metadata=False):
        references = [ref(self.root, "protocol_helper.py")]
        references += self.values["read-request"]["inputs"]
        references += self.values["lock"]["schemas"]
        references += [ref(self.root, "madb.lock", "madb.lock/1"), ref(self.root, "profiles/rust-c.json", "madb.profile/1")]
        if not metadata:
            references.append(ref(self.root, "contract.json", "madb.contract-ir/1"))
        # Older read-only fixture schema refs lack wire schema tags; execution requires them.
        return [dict(item, schema=item.get("schema", "madb.source-input/1")) for item in references]

    def recipe(self, operation, request, inputs, response):
        return dict(id="fixture-" + operation, request=ref(self.root, request, "madb." + ("read-request" if operation == "metadata" else operation + "-request") + "/1"),
                    tool=self.tool, installed_tool=self.installed,
                    arguments=["../inputs/protocol_helper.py", operation, str(self.marker)], environment=[],
                    working_directory="work", response_path=response, inputs=sorted(inputs, key=lambda item: item["path"]))

    def bundle(self, operation, recipe=None, tasks_list=None):
        return dict(schema="madb.execution/1", operation=operation, context=self.context(operation == "metadata"),
                    recipe=recipe, tasks=tasks_list or [], store="domain", run_parent="runs", cache="cache",
                    watch=dict(absent=[], directories=[]), dependencies=None)

    def refresh_generation(self):
        request = deepcopy(self.values["read-request"])
        contract = self.values["contract-ir"]
        request.update(schema="madb.generate-request/1", contract=ref(self.root, "contract.json", "madb.contract-ir/1"),
                       input_digest=contract["source_digest"], task_digest="sha256:" + "0" * 64,
                       contract_digest=contract["structure_digest"], policy_digest=contract["policy_digest"], output_root="staging/generate")
        save(self.root, "generate-request.json", request)
        inputs = self.common_inputs() + [ref(self.root, "behavior.json"), ref(self.root, "compiler.json")]
        recipe = self.recipe("generate", "generate-request.json", inputs, "outputs.json")
        request["task_digest"] = generation_task(request, recipe)
        save(self.root, "generate-request.json", request)
        recipe["request"] = ref(self.root, "generate-request.json", "madb.generate-request/1")
        self.generate_request = request
        self.generate_bundle = self.bundle("generate", recipe)
        save(self.root, "generate.json", self.generate_bundle)
        return self.generate_bundle

    def metadata(self):
        shutil.copyfile(self.root / "contract.json", self.root / "prepared-contract.json")
        recipe = self.recipe("metadata", "read.json", self.common_inputs(True) + [ref(self.root, "prepared-contract.json", "madb.contract-ir/1")], "contract.json")
        bundle = self.bundle("metadata", recipe)
        save(self.root, "metadata.json", bundle)
        (self.root / "contract.json").unlink()
        return bundle

    def layer(self, layer):
        index = json.loads((self.root / "domain/current.json").read_text())
        return self.root / "domain/batches" / index[layer].removeprefix("sha256:") / "files"

    def generation_expected(self):
        request = self.generate_request
        sources = json.loads((self.root / "behavior.json").read_text())["sources"]
        files = [dict(path=name, sha256=byte_digest(data), size=len(data), sources=sources)
                 for name, data in [("include/api.h", HEADER), ("src/api.c", IMPLEMENTATION)]]
        value = dict(schema="madb.output-set/1", input_digest=request["input_digest"], task_digest=request["task_digest"],
                     profile_digest=request["selector"]["profile_digest"], manifest_path="outputs.json", files=files)
        value["generation_id"] = generation_identity(value)
        return value

    def build(self, *, mode=None, ready=None, phases=("compile", "link", "run")):
        flags = self.link_flags
        (self.root / "consumer.c").write_bytes(CONSUMER)
        oracle = self.root / "native-oracle"
        (oracle / "inputs/src").mkdir(parents=True)
        (oracle / "inputs/native").mkdir()
        (oracle / "inputs/include").mkdir()
        (oracle / "inputs/include/api.h").write_bytes(HEADER)
        (oracle / "work/native").mkdir(parents=True)
        (oracle / "inputs/src/api.c").write_bytes(IMPLEMENTATION)
        (oracle / "inputs/consumer.c").write_bytes(CONSUMER)
        subprocess.run([self.compiler, "-nostdinc", "-ffreestanding", "-I../inputs/include", "-c", "../inputs/src/api.c", "-o", "native/api.o"], cwd=oracle / "work", check=True, capture_output=True, timeout=20)
        shutil.copyfile(oracle / "work/native/api.o", oracle / "inputs/native/api.o")
        subprocess.run([self.compiler, *flags, "../inputs/consumer.c", "../inputs/native/api.o", "-o", "native/consumer"], cwd=oracle / "work", check=True, capture_output=True, timeout=20)
        references = {}
        for name, data in [("include/api.h", HEADER), ("src/api.c", IMPLEMENTATION), ("native/api.o", (oracle / "work/native/api.o").read_bytes()),
                           ("native/consumer", (oracle / "work/native/consumer").read_bytes())]:
            references[name] = dict(path=name, sha256=byte_digest(data), size=len(data), schema="madb.source-input/1")
        self.tasks = []
        for operation, dependencies, inputs, outputs in [
            ("compile", [], ["src/api.c", "include/api.h"], ["native/api.o"]),
            ("link", ["compile"], ["native/api.o"], ["native/api-linked.o", "native/consumer"]),
            ("run", ["link"], ["native/consumer"], ["native/verified-consumer"])]:
            if operation not in phases:
                continue
            task = deepcopy(self.values["build-task"])
            task.update(id=operation, phase=operation, dependencies=dependencies, tool=self.tool,
                        generation_id=self.generation_expected()["generation_id"],
                        arguments=["../inputs/protocol_helper.py", operation, str(self.marker)],
                        environment=[dict(name="PATH", value=os.defpath)],
                        inputs=[ref(self.root, name) for name in ["protocol_helper.py", "compiler.json", "consumer.c"]] + [references[name] for name in inputs],
                        outputs=outputs)
            if operation == "compile" and mode:
                task["arguments"].append(mode)
                if ready is not None:
                    task["arguments"].append(str(ready))
            for file in task["inputs"]:
                file.pop("schema", None)
            task["task_digest"] = query.task_digest(task)
            save(self.root, "tasks/" + operation + ".json", task)
            self.tasks.append(task)
        bundle = self.bundle("build", tasks_list=[dict(request=ref(self.root, "tasks/" + task["id"] + ".json", "madb.build-task/1"), installed_tool=self.installed) for task in self.tasks])
        save(self.root, "build.json", bundle)
        return bundle

    def package(self):
        source, build = self.layer("source"), self.layer("build")
        generation = self.generation_expected()["generation_id"]
        evidence = [ref(build, f"evidence/task-{index:04}.json", "madb.build-evidence/1") for index in range(len(self.tasks))]
        contract = self.values["contract-ir"]
        request = dict(schema="madb.package-request/1", selector=deepcopy(self.values["read-request"]["selector"]),
                       output_set=ref(source, "outputs.json", "madb.output-set/1"), build_evidence=evidence,
                       lock=ref(self.root, "madb.lock", "madb.lock/1"), contract_digest=contract["structure_digest"],
                       policy_digest=contract["policy_digest"], generation_id=generation,
                       dependencies=[], public_symbols=self.symbols, required_cases=["DATA-001/positive/rust-c"], output_root="staging/package")
        save(self.root, "package-request.json", request)
        build_metadata = json.loads((build.parent / "metadata.json").read_text())
        evidence_paths = {item["path"] for item in evidence}
        build_inputs = [ref(build, item["path"]) for item in build_metadata["files"] if item["path"] not in evidence_paths]
        inputs = self.common_inputs() + evidence + build_inputs + [request["output_set"], ref(source, "include/api.h"), ref(source, "src/api.c")]
        recipe = self.recipe("package", "package-request.json", inputs, "candidate.json")
        bundle = self.bundle("package", recipe)
        save(self.root, "package.json", bundle)
        return bundle
