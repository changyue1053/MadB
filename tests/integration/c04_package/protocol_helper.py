"""Locked fixture protocol peer. No production code or cache completeness claims."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()


def semantic(domain, value):
    return "sha256:" + hashlib.sha256(domain.encode() + b"\n" + canonical(value)).hexdigest()


def reference(name):
    data = Path(name).read_bytes()
    return dict(path=name, sha256="sha256:" + hashlib.sha256(data).hexdigest(), size=len(data))


def emit(value, path):
    data = canonical(value) + b"\n"
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    os.write(1, data)


def main():
    signal.alarm(25)
    operation, marker = sys.argv[1:3]
    with open(marker, "a") as stream:
        stream.write(json.dumps(dict(operation=operation, cwd=str(Path.cwd()), pid=os.getpid())) + "\n")
    request = json.load(sys.stdin)
    if operation == "metadata":
        emit(json.loads(Path("../inputs/prepared-contract.json").read_text()), "contract.json")
    elif operation == "generate":
        behavior = json.loads(Path("../inputs/behavior.json").read_text())
        if behavior.get("wait_ready"):
            Path(behavior["wait_ready"]).write_text(json.dumps(dict(pid=os.getpid(), parent=os.getppid(), phase="before-output")))
            while True:
                signal.pause()
        Path("include").mkdir()
        Path("src").mkdir()
        Path("include/api.h").write_bytes(b"int madb_answer(void);\n")
        Path("src/api.c").write_bytes(b'#include "api.h"\nint madb_answer(void) { return 42; }\n')
        if behavior.get("invalid_syntax"):
            Path("src/api.c").write_bytes(b"int madb_answer( {\n")
        compiler_spec = json.loads(Path("../inputs/compiler.json").read_text())
        compiler = compiler_spec["path"]
        assert "sha256:" + hashlib.sha256(Path(compiler).read_bytes()).hexdigest() == compiler_spec["sha256"]
        check = Path(".declaration-check.c")
        check.write_bytes(b'#include "api.h"\nint declaration_probe(void) { return madb_answer(); }\n')
        arguments = ["-fsyntax-only", "-Werror", "-std=c11", "-Iinclude", "src/api.c", str(check)]
        checked = subprocess.run([compiler, *arguments], capture_output=True, timeout=15)
        check.unlink()
        with open(marker + ".validation.jsonl", "a") as log:
            log.write(json.dumps(dict(compiler=compiler, sha256=compiler_spec["sha256"], arguments=arguments,
                                      exit_code=checked.returncode, stderr=checked.stderr.decode())) + "\n")
        if checked.returncode:
            os.write(2, checked.stderr)
            raise SystemExit(checked.returncode)
        sources = behavior["sources"]
        files = [dict(reference(name), sources=sources) for name in ["include/api.h", "src/api.c"]]
        value = dict(schema="madb.output-set/1", input_digest=request["input_digest"],
                     task_digest=request["task_digest"], profile_digest=request["selector"]["profile_digest"],
                     manifest_path="outputs.json", files=files)
        identity = {key: value[key] for key in ["input_digest", "task_digest", "profile_digest", "files"]}
        value["generation_id"] = semantic("madb.generation/1", identity)
        if behavior.get("undeclared"):
            Path("unexpected.txt").write_bytes(b"not registered")
        emit(value, "outputs.json")
    elif operation in ["compile", "link", "run"]:
        if operation == "compile" and len(sys.argv) > 3:
            if sys.argv[3] == "fail-build":
                os.write(2, b"fixture intentional compiler failure\n")
                raise SystemExit(27)
            if sys.argv[3] == "wait-build":
                Path(sys.argv[4]).write_text(json.dumps(dict(pid=os.getpid(), parent=os.getppid(), phase="before-compile")))
                while True:
                    signal.pause()
        compiler_spec = json.loads(Path("../inputs/compiler.json").read_text())
        compiler = compiler_spec["path"]
        assert "sha256:" + hashlib.sha256(Path(compiler).read_bytes()).hexdigest() == compiler_spec["sha256"]
        Path("native").mkdir()
        Path("logs").mkdir()
        if operation == "compile":
            command = [compiler, "-nostdinc", "-ffreestanding", "-I../inputs/include", "-c", "../inputs/src/api.c", "-o", "native/api.o"]
            output = "native/api.o"
        elif operation == "link":
            subprocess.run([compiler, "-nostdlib", "-r", "../inputs/native/api.o", "-o", "native/api-linked.o"], check=True, capture_output=True, timeout=15)
            command = [compiler, *compiler_spec["link_flags"], "../inputs/consumer.c", "../inputs/native/api.o", "-o", "native/consumer"]
            output = "native/consumer"
        else:
            shutil.copyfile("../inputs/native/consumer", "native/verified-consumer")
            Path("native/verified-consumer").chmod(0o700)
            command = ["./native/verified-consumer"]
            output = "native/verified-consumer"
        result = subprocess.run(command, capture_output=True, timeout=15)
        log = "logs/" + operation + ".txt"
        Path(log).write_bytes(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError(result.stderr.decode())
        value = dict(schema="madb.build-evidence/1", task_id=request["id"], task_digest=request["task_digest"],
                     input_digest=request["input_digest"], profile_digest=request["selector"]["profile_digest"],
                     generation_id=request["generation_id"], tool=request["tool"], phase=request["phase"],
                     target=request["selector"]["target"], status="passed", exit_code=0,
                     outputs=[reference(output)] + ([reference("native/api-linked.o")] if operation == "link" else []), logs=[reference(log)])
        os.write(1, canonical(value) + b"\n")
    elif operation == "package":
        names = ["include/api.h", "native/api-linked.o"]
        for name in names:
            Path(name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(Path("../inputs") / name, name)
        value = {key: request[key] for key in ["selector", "generation_id", "contract_digest", "policy_digest",
                                               "dependencies", "public_symbols", "required_cases", "build_evidence"]}
        value.update(schema="madb.candidate/1", lock_digest=request["lock"]["sha256"],
                     manifest_path="candidate.json", files=[reference(name) for name in names])
        value["candidate_digest"] = semantic("madb.candidate/1", value)
        emit(value, "candidate.json")
    else:
        raise RuntimeError("unknown fixture operation")


if __name__ == "__main__":
    main()
