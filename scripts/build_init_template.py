"""Reproduce versioned installed templates with exact two-block USTAR trailers."""
from pathlib import Path
import argparse
import hashlib
import json
import tarfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "templates/projects/rust-c-sdk-values"
MANIFEST = "meta/template.json"
LEGACY_ARCHIVE = "97d0e9fd5747bd5ceea920574e75c8e17825e379037c94b68e878649a310a850"
LEGACY_MANIFEST = "3b23424744c85311b2b3b1aadb055bf20f72f623aee42696888a85f767fb1eda"


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def identity(name, raw):
    return {"path": name, "sha256": "sha256:" + hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def products(bundle=BUNDLE):
    source = bundle / "source"
    files = {}
    for path in sorted(source.rglob("*")):
        if path.is_symlink() or (not path.is_dir() and not path.is_file()):
            raise ValueError("template sources must be ordinary files/directories")
        if path.is_file():
            name = path.relative_to(source).as_posix()
            if not name.isascii() or len(name.encode()) > 100:
                raise ValueError("this bundled authoring recipe requires short ASCII paths")
            files[name] = path.read_bytes()
    document = json.loads(files.pop(MANIFEST))
    expected_version = "0.1.0" if bundle == BUNDLE else bundle.name
    if document["version"] != expected_version:
        raise ValueError("template version differs from its immutable version directory")
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
    if bundle == BUNDLE and (
        hashlib.sha256(archive).hexdigest() != LEGACY_ARCHIVE
        or hashlib.sha256(files[MANIFEST]).hexdigest() != LEGACY_MANIFEST
    ):
        raise ValueError("template 0.1.0 is immutable; use a new version directory")
    return {source / MANIFEST: files[MANIFEST], bundle / "package.tar": archive,
            bundle / "identity.json": encoded(pins)}


def bundles():
    # Explicit supported versions prevent an unrelated folder from becoming an
    # installed artifact merely because it appeared beside authored sources.
    return [BUNDLE, BUNDLE / "versions/0.1.1"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    for bundle in bundles():
        for path, raw in products(bundle).items():
            if options.check:
                if not path.is_file() or path.read_bytes() != raw:
                    raise SystemExit(f"template derivative differs: {path.relative_to(ROOT)}")
            else:
                path.write_bytes(raw)
    print("Installed template derivatives match" if options.check else "Installed template derivatives generated")


if __name__ == "__main__":
    main()
