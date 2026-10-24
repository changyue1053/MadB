"""Independent ASCII fixture projections; no production Rust hashing is invoked."""
from copy import deepcopy
import importlib.util
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("c04_query_fixture", ROOT / "tests/integration/c04/fixture.py")
query = importlib.util.module_from_spec(spec)
spec.loader.exec_module(query)
raw, ref, save, task_digest = query.raw, query.ref, query.save, query.task_digest
PYTHON = Path(sys.executable).resolve()
VALUE = b"independent captured source"
EXPECTED = VALUE.upper() + b"\n"
LOG = b"fixture recipe completed\n"


def profile_digest(profile):
    value = deepcopy(profile)
    value.pop("extensions", None)
    value["defaults"] = value["defaults"]["values"]
    for field, key in [("tools", "id"), ("macros", "name"), ("restrictions", "feature")]:
        value[field].sort(key=lambda item: item[key])
    for field in ["required_features", "supported_features"]:
        value[field].sort()
    return query.semantic("madb.profile/1", value)


def persist(root, values):
    """Apply the independent profile/project/lock/request reference ordering."""
    digest = profile_digest(values["profile"])
    for kind in ["read-request", "contract-ir", "build-task"]:
        values[kind]["selector"]["profile_digest"] = digest
    save(root, "profiles/rust-c.json", values["profile"])
    profile_ref = ref(root, "profiles/rust-c.json", "madb.profile/1")
    values["project"]["boundaries"][0]["profile"] = profile_ref
    values["lock"]["profiles"][0].update(semantic_digest=digest, file=profile_ref)
    values["lock"]["project_digest"] = query.project_digest(values["project"])
    save(root, "project.json", values["project"])
    save(root, "madb.lock", values["lock"])
    values["read-request"]["profile"] = profile_ref
    values["read-request"]["lock"] = ref(root, "madb.lock", "madb.lock/1")
    save(root, "read.json", values["read-request"])
    save(root, "contract.json", values["contract-ir"])
    save_task(root, values["build-task"])


def save_task(root, task, recompute=True):
    if recompute:
        task["task_digest"] = task_digest(task)
    save(root, "task.json", task)


def materialize(root, marker, mode="normal"):
    values = query.materialize(root)
    shutil.copyfile(Path(__file__).with_name("helper.py"), root / "helper.py")
    (root / "value").write_bytes(VALUE)
    tool = dict(values["profile"]["tools"][0], id="fixture-python", version="fixture-runtime",
                sha256=raw(PYTHON.read_bytes()), source="fixture://resolved-python-image", arguments=[])
    values["profile"]["tools"] = [tool]
    values["lock"]["tools"] = [deepcopy(tool)]
    task = values["build-task"]
    task.update(id="real-recipe", dependencies=[], working_directory="work",
                tool={key: tool[key] for key in ["id", "version", "sha256"]},
                arguments=["../inputs/helper.py", mode, str(marker)], environment=[],
                inputs=[ref(root, name) for name in ["helper.py", "value"]],
                outputs=["output.bin"], input_digest=values["contract-ir"]["source_digest"])
    persist(root, values)
    return values
