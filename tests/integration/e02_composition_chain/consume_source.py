"""Independent Source byte consumer; no MadB, compiler or target runtime import."""
from pathlib import Path
import hashlib
import json
import sys

ORDINARY = {
    "identity.json", "declarations.json", "generated/contract/runtime-policy.json",
    "generated/contract/frame-layout.json", "generated/contract/inventory-format.json",
    "generated/notices/native-NOTICE.txt", "generated/notices/llvm-NOTICE.txt",
    "generated/facade/Boundary.ets", "generated/facade/RuntimeFrame.ets",
    "generated/native/exports.map", "generated/native/Index.d.ts",
    "generated/native/exports.generated.inc", "generated/keep-rules.txt",
    "generated/native/frame.generated.hpp", "generated/native/identity.generated.hpp",
}
OPTIONAL = {
    "generated/facade/StateObservation.ets": "1e26f847c8e37d7c65b236188d86c6ee3ad6dcc53c033d4304049a7ab7531534",
    "generated/facade/ArkUIStateModel.ets": "7e62d7352ded16641e560f9230b65576d524caa8ac847dc65715e7e877ad77ef",
}


def consume(root, mode):
    if mode not in ("ordinary", "enhanced", "disabled"):
        raise ValueError("unknown consumer mode")
    expected = ORDINARY | (OPTIONAL.keys() if mode == "enhanced" else set())
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("regular source root required")
    names = set()
    for path in root.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("non-regular source entry")
        if path.is_file():
            names.add(path.relative_to(root).as_posix())
    if names != expected | {"outputs.json"}:
        raise ValueError("source inventory differs")
    manifest = json.loads((root / "outputs.json").read_bytes())
    if manifest["schema"] != "madb.output-set/1" or manifest["manifest_path"] != "outputs.json":
        raise ValueError("public manifest format differs")
    if {f["path"] for f in manifest["files"]} != expected or len(manifest["files"]) != len(expected):
        raise ValueError("manifest payload inventory differs")
    for ref in manifest["files"]:
        data = (root / ref["path"]).read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if len(data) != ref["size"] or "sha256:" + digest != ref["sha256"]:
            raise ValueError("public payload byte identity differs")
        if ref["path"] in OPTIONAL and digest != OPTIONAL[ref["path"]]:
            raise ValueError("fixed optional SDK source differs")
    return {"consumed_payloads": len(expected), "generation_id": manifest["generation_id"],
            "profile_digest": manifest["profile_digest"], "mode": mode,
            "scope": "exported Source data only; no build or runtime assertion"}


if __name__ == "__main__":
    print(json.dumps(consume(sys.argv[1], sys.argv[2])))
