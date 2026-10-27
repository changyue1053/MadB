"""Standalone source-data consumer. No MadB imports or compiler/runtime claim."""
from pathlib import Path
import hashlib
import json
import sys

EXPECTED = {
    "identity.json", "declarations.json", "generated/contract/runtime-policy.json",
    "generated/contract/frame-layout.json", "generated/contract/inventory-format.json",
    "generated/notices/native-NOTICE.txt", "generated/notices/llvm-NOTICE.txt",
    "generated/facade/Boundary.ets", "generated/facade/RuntimeFrame.ets",
    "generated/native/exports.map", "generated/native/Index.d.ts",
    "generated/native/exports.generated.inc", "generated/keep-rules.txt",
    "generated/native/frame.generated.hpp", "generated/native/identity.generated.hpp",
}

def consume(root):
    root=Path(root)
    if root.is_symlink() or not root.is_dir(): raise ValueError("regular source root required")
    names=set()
    for file in root.rglob("*"):
        if file.is_symlink(): raise ValueError("symbolic source file")
        if file.is_file(): names.add(file.relative_to(root).as_posix())
    if names!=EXPECTED|{"outputs.json"}: raise ValueError("ordinary source inventory differs")
    manifest=json.loads((root/"outputs.json").read_bytes())
    if manifest["schema"]!="madb.output-set/1" or manifest["manifest_path"]!="outputs.json":
        raise ValueError("public manifest format differs")
    if {f["path"] for f in manifest["files"]}!=EXPECTED or len(manifest["files"])!=15:
        raise ValueError("manifest payload inventory differs")
    for ref in manifest["files"]:
        data=(root/ref["path"]).read_bytes()
        if len(data)!=ref["size"] or "sha256:"+hashlib.sha256(data).hexdigest()!=ref["sha256"]:
            raise ValueError("public payload byte identity differs")
    return {"consumed_payloads":15,"generation_id":manifest["generation_id"],
            "profile_digest":manifest["profile_digest"],"scope":"exported Source data only"}

if __name__=="__main__": print(json.dumps(consume(sys.argv[1])))
