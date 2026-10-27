"""Reproduce the one installed template, including exact two-block USTAR trailer."""
from pathlib import Path
import argparse
import hashlib
import json
import tarfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "templates/projects/rust-c-sdk-values"
SOURCE = BUNDLE / "source"
MANIFEST = "meta/template.json"


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def identity(name, raw):
    return {"path": name, "sha256": "sha256:" + hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def products():
    files = {}
    for path in sorted(SOURCE.rglob("*")):
        if path.is_symlink() or (not path.is_dir() and not path.is_file()):
            raise ValueError("template sources must be ordinary files/directories")
        if path.is_file():
            name = path.relative_to(SOURCE).as_posix()
            if not name.isascii() or len(name.encode()) > 100:
                raise ValueError("this bundled authoring recipe requires short ASCII paths")
            files[name] = path.read_bytes()
    document = json.loads(files.pop(MANIFEST))
    inventory = {name: identity(name, raw) for name, raw in sorted(files.items())}
    document["files"] = list(inventory.values())
    for asset in document["assets"]:
        if asset["source"] is not None:
            asset["source"] = inventory[asset["source"]["path"]]
    for boundary in document["boundary_refs"]:
        ref = boundary["profile"]
        boundary["profile"] = {**inventory[ref["path"]], "schema": ref["schema"]}
    files[MANIFEST] = encoded(document)
    archive = bytearray()
    for name, raw in sorted(files.items()):
        info = tarfile.TarInfo(name)
        info.size = len(raw)
        info.mode = 0o644
        info.uid = info.gid = info.mtime = 0
        info.uname = info.gname = ""
        archive.extend(info.tobuf(format=tarfile.USTAR_FORMAT))
        archive.extend(raw)
        archive.extend(bytes((-len(raw)) % 512))
    archive.extend(bytes(1024))
    archive = bytes(archive)
    pins = {"archive": identity("template-package.tar", archive),
            "manifest": {**identity(MANIFEST, files[MANIFEST]), "schema": document["schema"]}}
    return {SOURCE / MANIFEST: files[MANIFEST], BUNDLE / "package.tar": archive,
            BUNDLE / "identity.json": encoded(pins)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    for path, raw in products().items():
        if options.check:
            if not path.is_file() or path.read_bytes() != raw:
                raise SystemExit(f"template derivative differs: {path.relative_to(ROOT)}")
        else:
            path.write_bytes(raw)
    print("Installed template derivatives match" if options.check else "Installed template derivatives generated")


if __name__ == "__main__":
    main()
