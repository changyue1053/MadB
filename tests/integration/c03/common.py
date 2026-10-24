"""Small test-only helpers. This is not the MadB process executor."""
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "tests/fixtures/native-probe"


def digest(path):
    with Path(path).open("rb") as stream:
        return "sha256:" + hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def file_ref(path, root, schema=None):
    path = Path(path)
    result = {"path": path.relative_to(root).as_posix(), "sha256": digest(path), "size": path.stat().st_size}
    if schema:
        result["schema"] = schema
    return result


class Commands:
    def __init__(self, output, env, substitutions):
        self.output, self.env, self.substitutions = output, env, substitutions
        self.entries = []

    def normalize(self, text):
        for original, replacement in self.substitutions:
            text = text.replace(str(original), replacement)
        return text

    def run(self, args, *, cwd=None, expected_error=None, timeout=60):
        args = list(map(str, args))
        result = subprocess.run(args, cwd=cwd or self.output, env=self.env,
                                text=True, capture_output=True, timeout=timeout)
        entry = {"argv": [self.normalize(a) for a in args], "cwd": self.normalize(str(cwd or self.output)),
                 "exit_code": result.returncode, "stdout": self.normalize(result.stdout),
                 "stderr": self.normalize(result.stderr), "expected_error": expected_error}
        self.entries.append(entry)
        write_json(self.output / "commands.json", self.entries)
        if expected_error:
            if result.returncode == 0 or expected_error not in result.stderr:
                raise RuntimeError(f"Expected rejection {expected_error}: {entry}")
        elif result.returncode:
            raise RuntimeError(f"Command exited {result.returncode}: {entry['argv']}: {entry['stderr'][-2000:]}")
        return result.stdout.strip()
