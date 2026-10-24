"""C04 package port: copy a selected, frozen public SDK inventory."""
import json
from pathlib import Path
import shutil
import sys

ROOT = Path.cwd().parent / "inputs"
sys.path.insert(0, str(ROOT / "adapters/toolchains/rust_c"))
from common import canonical, ref, semantic, sha_file


def main():
    data = sys.stdin.buffer.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024:
        raise ValueError("PackageRequest exceeds 1 MiB")
    request = json.loads(data)
    inventory = json.loads((ROOT / "package-files.json").read_text())
    if request["schema"] != "madb.package-request/1":
        raise ValueError("expected PackageRequest")
    names = [item["path"] for item in inventory]
    if len(names) != len(set(names)) or "candidate.json" in names:
        raise ValueError("duplicate or recursive candidate inventory")
    for item in inventory:
        name = item["path"]
        if Path(name).is_absolute() or any(part in ("", ".", "..") for part in name.split("/")):
            raise ValueError("invalid logical package path")
        source = ROOT / name
        if source.is_symlink() or not source.resolve().is_relative_to(ROOT.resolve()):
            raise ValueError("package source escapes the frozen input root")
        if source.stat().st_size != item["size"] or sha_file(source) != item["sha256"]:
            raise ValueError("package source changed: " + name)
        target = Path.cwd() / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    value = {key: request[key] for key in ["selector", "generation_id", "contract_digest", "policy_digest",
                                         "dependencies", "public_symbols", "required_cases", "build_evidence"]}
    value.update(schema="madb.candidate/1", lock_digest=request["lock"]["sha256"],
                 manifest_path="candidate.json", files=[ref(Path.cwd(), name) for name in sorted(names)])
    for key in ["dependencies", "build_evidence"]:
        value[key].sort(key=lambda item: item["path"])
    for key in ["public_symbols", "required_cases"]:
        value[key].sort()
    value["candidate_digest"] = semantic("madb.candidate/1", value)
    output = (canonical(value) + "\n").encode()
    Path("candidate.json").write_bytes(output)
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        sys.stderr.write("native package: " + str(error) + "\n")
        raise SystemExit(2) from error
