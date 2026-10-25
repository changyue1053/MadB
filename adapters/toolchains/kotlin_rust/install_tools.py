"""Acquire fixed C06 compiler distributions and exact Maven files before execution."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import tarfile
import tempfile
import urllib.request
import zipfile

RECIPE = Path(__file__).with_name("toolchain.json")
MAX_DOWNLOAD = 768 * 1024 * 1024


def digest(path):
    with Path(path).open("rb") as stream:
        return "sha256:" + hashlib.file_digest(stream, "sha256").hexdigest()


def relative(value):
    path = PurePosixPath(value)
    if not value or path.is_absolute() or any(p in ("", ".", "..") for p in value.split("/")):
        raise ValueError("invalid installation path: " + value)
    return Path(*path.parts)


def acquire(spec, cache):
    """The cache is only a transport optimization; every hit is rehashed."""
    expected = "sha256:" + spec["sha256"]
    if not spec["source"].startswith("https://") or len(spec["sha256"]) != 64:
        raise ValueError("source must be HTTPS with an exact SHA-256")
    cache = Path(cache)
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / spec["sha256"]
    limit = min(spec.get("size", MAX_DOWNLOAD), MAX_DOWNLOAD)
    if not archive.exists():
        with tempfile.NamedTemporaryFile(dir=cache, prefix="download-", delete=False) as stream:
            temporary = Path(stream.name)
            try:
                with urllib.request.urlopen(spec["source"], timeout=120) as response:
                    total = 0
                    while block := response.read(1024 * 1024):
                        total += len(block)
                        if total > limit:
                            raise ValueError("download byte budget exceeded: " + spec["source"])
                        stream.write(block)
                stream.flush()
                if digest(temporary) != expected:
                    raise ValueError("download SHA-256 mismatch: " + spec["source"])
                temporary.replace(archive)
            finally:
                temporary.unlink(missing_ok=True)
    if archive.is_symlink() or digest(archive) != expected:
        raise ValueError("cached source SHA-256 mismatch: " + str(archive))
    if archive.stat().st_size > limit:
        raise ValueError("cached source exceeds byte budget")
    if "size" in spec and archive.stat().st_size != spec["size"]:
        raise ValueError("source size mismatch: " + spec["source"])
    return archive


def extract(archive, spec, destination):
    with tempfile.TemporaryDirectory(prefix="madb-managed-extract-") as temporary:
        stage = Path(temporary)
        if spec["format"] == "tar.gz":
            with tarfile.open(archive) as package:
                package.extractall(stage, filter="data")
        elif spec["format"] == "zip":
            with zipfile.ZipFile(archive) as package:
                for item in package.infolist():
                    name = item.filename.rstrip("/")
                    relative(name)
                    mode = item.external_attr >> 16
                    if stat.S_ISLNK(mode):
                        raise ValueError("ZIP symlinks are unsupported")
                    package.extract(item, stage)
                    path = stage / name
                    if path.is_file() and mode & 0o111:
                        path.chmod(0o755)
        else:
            raise ValueError("unsupported source archive")
        source = stage / relative(spec["archive_root"])
        if not source.is_dir() or source.is_symlink():
            raise ValueError("missing distribution root")
        target = destination / relative(spec["path"])
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target, symlinks=True)


def inventory(root):
    result = []
    for path in sorted(root.rglob("*")):
        name = path.relative_to(root).as_posix()
        if path.is_symlink():
            if not path.resolve().is_relative_to(root.resolve()):
                raise ValueError("installed symlink escapes distribution: " + name)
            result.append(dict(path=name, symlink=os.readlink(path)))
        elif path.is_file():
            result.append(dict(path=name, sha256=digest(path), size=path.stat().st_size,
                               executable=bool(path.stat().st_mode & 0o111)))
    return result


def install(destination, cache=None, components=None):
    recipe = json.loads(RECIPE.read_text())
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        raise ValueError("C06 managed compiler profile requires macOS Arm64")
    names = sorted(components or recipe["archives"])
    if not names or len(set(names)) != len(names) or set(names) - recipe["archives"].keys():
        raise ValueError("unknown or duplicate compiler component")
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=False)
    cache = Path(cache or destination / "downloads").resolve()
    specs = [(name, recipe["archives"][name]) for name in names]
    with ThreadPoolExecutor(max_workers=3) as pool:
        archives = list(pool.map(lambda item: acquire(item[1], cache), specs))
    records = {}
    for (name, spec), archive in zip(specs, archives, strict=True):
        extract(archive, spec, destination)
        root = destination / spec["path"]
        records[name] = dict(source=spec, files=inventory(root))
    native = sorted(spec["archive_root"] for _, spec in specs if spec["path"].startswith("konan/dependencies/"))
    if native:
        (destination / "konan/dependencies/.extracted").write_text("\n".join(native) + "\n")
    receipt = dict(schema="madb.managed-installation/1", recipe_sha256=digest(RECIPE), components=records)
    (destination / "receipt.json").write_text(json.dumps(receipt, sort_keys=True, indent=2) + "\n")
    verify(destination)
    return destination


def verify(destination, receipt_sha256=None):
    """Call before/after each compiler phase; bind the receipt hash in madb.lock."""
    destination = Path(destination).resolve()
    receipt_path = destination / "receipt.json"
    if receipt_sha256 is not None and digest(receipt_path) != receipt_sha256:
        raise ValueError("installation receipt changed")
    receipt = json.loads(receipt_path.read_text())
    recipe = json.loads(RECIPE.read_text())
    if receipt.get("schema") != "madb.managed-installation/1" or receipt["recipe_sha256"] != digest(RECIPE):
        raise ValueError("installation recipe mismatch")
    if not receipt["components"] or set(receipt["components"]) - recipe["archives"].keys():
        raise ValueError("unknown installed component")
    paths = {}
    for name, record in receipt["components"].items():
        spec = recipe["archives"][name]
        if record["source"] != spec:
            raise ValueError("installed source identity changed: " + name)
        root = destination / relative(spec["path"])
        if not root.is_dir() or root.is_symlink() or not record["files"] or inventory(root) != record["files"]:
            raise ValueError("installed distribution changed: " + name)
        paths[name] = str(root)
    native = sorted(recipe["archives"][name]["archive_root"] for name in receipt["components"]
                    if recipe["archives"][name]["path"].startswith("konan/dependencies/"))
    if native and (destination / "konan/dependencies/.extracted").read_text() != "\n".join(native) + "\n":
        raise ValueError("native dependency registry changed")
    return dict(root=str(destination), receipt_sha256=digest(receipt_path), paths=paths)


def install_dependencies(lock, destination, cache=None):
    """Fetch an explicit, reviewed Maven file closure. Never resolve new versions."""
    lock = Path(lock)
    data = json.loads(lock.read_text())
    if data.get("schema") != "madb.maven-files/1" or not data.get("files"):
        raise ValueError("expected nonempty madb.maven-files/1 lock")
    names = [item["path"] for item in data["files"]]
    if len(names) != len(set(names)):
        raise ValueError("duplicate Maven file")
    for name in names:
        relative(name)
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=False)
    cache = Path(cache or destination / ".downloads").resolve()
    for spec in data["files"]:
        source = acquire(spec, cache)
        target = destination / relative(spec["path"])
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return dict(lock_sha256=digest(lock), files=[dict(path=item["path"], sha256=digest(destination / item["path"]))
                                               for item in data["files"]])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--cache", type=Path)
    parser.add_argument("--components", nargs="+")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--receipt-sha256")
    parser.add_argument("--maven-lock", type=Path)
    args = parser.parse_args(argv)
    if args.maven_lock:
        if args.verify or args.components or args.receipt_sha256:
            parser.error("Maven installation does not accept compiler verification flags")
        print(json.dumps(install_dependencies(args.maven_lock, args.root, args.cache), indent=2))
    elif args.verify:
        print(json.dumps(verify(args.root, args.receipt_sha256), indent=2))
    else:
        install(args.root, args.cache, args.components)
        print(json.dumps(verify(args.root), indent=2))


if __name__ == "__main__":
    main()
