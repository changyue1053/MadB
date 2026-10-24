"""Independent ASCII wire fixtures and known expected bytes for a static DAG."""
from copy import deepcopy
import importlib.util
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("c04_task_fixture", ROOT / "tests/integration/c04_tasks/fixture.py")
tasks_fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tasks_fixture)
raw, ref, save, task_digest = (tasks_fixture.raw, tasks_fixture.ref, tasks_fixture.save,
                              tasks_fixture.task_digest)
PYTHON = tasks_fixture.PYTHON
SEED = b"independent graph source"
LATER = b"source needed by later node"


def expected(inputs, node_id):
    return b"".join(value for _, value in sorted(inputs.items())) + b"|" + node_id.encode()


def reference(path, data):
    return {"path": path, "sha256": raw(data), "size": len(data)}


def save_tasks(root, tasks):
    for task in tasks:
        task["task_digest"] = task_digest(task)
        save(root, "tasks/" + task["id"] + ".json", task)


def materialize(root, markers, fork=False):
    values = tasks_fixture.materialize(root, markers / "unused.json")
    shutil.copyfile(Path(__file__).with_name("helper.py"), root / "helper.py")
    (root / "seed.bin").write_bytes(SEED)
    (root / "later.bin").write_bytes(LATER)
    specifications = [("a", [], {"seed.bin": SEED})]
    outputs = {"a.bin": expected(specifications[0][2], "a")}
    if fork:
        for name in ["b", "c"]:
            data = {"a.bin": outputs["a.bin"]}
            specifications.append((name, ["a"], data))
            outputs[name + ".bin"] = expected(data, name)
        data = {"b.bin": outputs["b.bin"], "c.bin": outputs["c.bin"]}
        specifications.append(("d", ["b", "c"], data))
        outputs["d.bin"] = expected(data, "d")
    else:
        data = {"a.bin": outputs["a.bin"], "later.bin": LATER}
        specifications.append(("b", ["a"], data))
        outputs["b.bin"] = expected(data, "b")
    tasks = []
    for name, dependencies, inputs in specifications:
        task = deepcopy(values["build-task"])
        task.update(id=name, dependencies=dependencies, phase="compile",
                    arguments=["../inputs/helper.py", "normal", str(markers / (name + ".json"))],
                    inputs=[ref(root, "helper.py")] + [reference(path, data) for path, data in inputs.items()],
                    outputs=[name + ".bin"])
        tasks.append(task)
    save_tasks(root, tasks)
    return tasks, outputs
