"""Install source-verified C06 binding backends into a new, explicit directory."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.generators.uniffi import driver as uniffi
from adapters.generators.gobley import driver as gobley

BACKENDS = ("uniffi", "gobley")


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def install(root, backends=BACKENDS, repo=ROOT):
    """Delegate source verification/builds; never reuse or overwrite a partial root."""
    selected = tuple(backends)
    if not selected or len(selected) != len(set(selected)) or set(selected) - set(BACKENDS):
        raise ValueError("select each supported backend at most once")
    root = Path(root).expanduser().absolute()
    if root.exists() or root.is_symlink():
        raise ValueError("backend installation root must not exist")
    root.mkdir(parents=True, exist_ok=False)
    root = root.resolve(strict=True)
    state = dict(schema="madb.backend-installation/1", status="installing", requested=list(selected), completed=[])
    state_path = root / "installation.json"
    save(state_path, state)
    result = {}
    try:
        for backend in selected:
            destination = root / backend
            print("Installing source-verified " + backend + " in " + str(destination), flush=True)
            if backend == "uniffi":
                installed = uniffi.install(destination, repo=repo)
                receipt = uniffi.verify(installed)
                if receipt.get("metadata_inputrefs") != uniffi.metadata_inputrefs(repo) or receipt.get("metadata_features") != []:
                    raise ValueError("UniFFI author source changed during installation")
            else:
                gobley.install(destination)
                gobley.verify_install(destination)
                installed = dict(root=str(destination), receipt_sha256=hashlib.sha256((destination / "receipt.json").read_bytes()).hexdigest())
            record = root / (backend + "-tools.json")
            save(record, installed)
            state["completed"].append(backend)
            save(state_path, state)
            result[backend] = installed
        state["status"] = "complete"
        save(state_path, state)
    except Exception as error:
        state.update(status="failed", error=str(error))
        save(state_path, state)
        raise
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="new directory; partial installations are retained, never overwritten")
    parser.add_argument("--backends", nargs="+", choices=BACKENDS, default=list(BACKENDS), help="omit for both; SKIE alone needs neither backend")
    args = parser.parse_args(argv)
    if len(args.backends) != len(set(args.backends)):
        parser.error("--backends must not contain duplicates")
    try:
        result = install(args.root, args.backends)
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, "Backend installation failed; retained at " + str(args.root.absolute()) + ": " + str(error) + "\n")
    for name, value in result.items():
        print(name + ": " + value["root"])


if __name__ == "__main__":
    main()
