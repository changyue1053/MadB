"""Build the three C05 public candidates through the frozen MadB execution ports."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.rust_c.tools import discover
from adapters.toolchains.rust_c.project import prepare
from adapters.toolchains.rust_c.pipeline import Pipeline
from adapters.toolchains.rust_c.common import sha_file
import hashlib

CASES = {
    "c-to-rust": ["data", "lifecycle", "observe", "bootstrap", "abi-mismatch", "profile-mismatch", "missing-bootstrap"],
    "c-to-rust-test": ["operation-close", "source-lease", "registration-failure", "panic", "diagnostics", "token-exhaustion", "capacity"],
    "rust-to-c": ["data", "bootstrap", "abi-mismatch", "profile-mismatch"],
}


def require_revision(revision):
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, text=True).strip()
    if current != revision or dirty:
        raise RuntimeError("producer checkout changed during formal candidate preparation")


def verify_revision_inputs(root, prepared, revision):
    # Match every copied author/helper/schema byte to the claimed Git commit.
    # backend/toolchain JSON are separately locked observations, not Git inputs.
    references = prepared["read"]["inputs"] + prepared["lock"]["schemas"]
    for item in references:
        name = item["path"]
        if name in ("backend.json", "toolchain.json"):
            continue
        expected = subprocess.check_output(["git", "show", revision + ":" + name], cwd=ROOT)
        expected_digest = "sha256:" + hashlib.sha256(expected).hexdigest()
        if expected_digest != item["sha256"] or sha_file(root / name) != expected_digest:
            raise RuntimeError("frozen author input differs from producer revision: " + name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new output directory; never overwrite a candidate")
    parser.add_argument("--bindgen", type=Path, required=True)
    parser.add_argument("--cbindgen", type=Path, required=True)
    parser.add_argument("--allow-dirty", action="store_true", help="development only: emit provisional candidates that formal Lab acceptance rejects")
    args = parser.parse_args()
    output = args.output.absolute()
    if output.exists():
        parser.error("--output must not exist")
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, text=True).strip())
    if dirty and not args.allow_dirty:
        parser.error("formal candidates require a clean producer checkout; commit changes first or explicitly use --allow-dirty for development")
    subprocess.run(["cargo", "build", "--locked", "-p", "madb-cli", "--bins"], cwd=ROOT, check=True)
    observation = discover(ROOT, args.bindgen, args.cbindgen)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    work = Path(tempfile.mkdtemp(prefix="madb-first-route-")).resolve()
    host_image = work / "madb-host"
    shutil.copyfile(ROOT / "target/debug/madb", host_image)
    host_image.chmod(0o500)
    output.mkdir(parents=True)
    if args.allow_dirty:
        print("PROVISIONAL development candidates: producer checkout contains uncommitted changes", flush=True)
    print("Native execution workspace: " + str(work), flush=True)
    for variant, case_names in CASES.items():
        root = work / variant
        print("Preparing " + variant, flush=True)
        if not args.allow_dirty:
            require_revision(revision)
        prepared = prepare(ROOT, root, observation, variant)
        if not args.allow_dirty:
            verify_revision_inputs(root, prepared, revision)
            require_revision(revision)
        pipeline = Pipeline(ROOT, root, prepared, host_image)
        pipeline.metadata()
        pipeline.generate()
        pipeline.build()
        candidate = pipeline.package(output / variant, revision, ["c05." + variant + "." + case for case in case_names], provisional=args.allow_dirty)
        print(variant + ": " + candidate["candidate_digest"], flush=True)
    if not args.allow_dirty:
        require_revision(revision)
    print("Public candidates: " + str(output), flush=True)


if __name__ == "__main__":
    main()
