"""Build restricted ArkTS candidates through the existing MadB C04 ports."""

from __future__ import annotations

import sys

if __name__ == "__main__" and (
    not sys.flags.isolated
    or not sys.flags.no_site
    or not sys.flags.dont_write_bytecode
    or sys._xoptions.get("pycache_prefix") != "/dev/null"
):
    raise SystemExit("Use Python -I -S -B -X pycache_prefix=/dev/null for this frozen public entry")

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import traceback

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from adapters.generators.arkts.support import clean_environment, read_json, sha, verify_refs
from adapters.toolchains.arkts.pipeline import Pipeline
from adapters.toolchains.arkts.project import prepare_route
from adapters.toolchains.arkts.requirements import load as load_requirements
from adapters.toolchains.arkts.toolchain import observe


def git(arguments):
    return subprocess.check_output(
        ["/usr/bin/git", "-c", "core.fsmonitor=false", *arguments],
        cwd=ROOT,
        env=clean_environment({"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}),
    )


def checkout_state():
    revision = git(["rev-parse", "HEAD"]).decode().strip()
    status = git(["status", "--porcelain", "--untracked-files=normal"]).decode().strip()
    return revision, bool(status)


def require_revision(revision):
    if checkout_state() != (revision, False):
        raise RuntimeError("producer checkout changed during formal candidate preparation")


def verify_revision_inputs(workspace, prepared, revision):
    references = {r["path"]: r for r in prepared["read"]["inputs"] + prepared["lock"]["schemas"]}
    for logical, source in sorted(prepared["producer_inputs"].items()):
        data = git(["show", revision + ":" + source])
        expected = "sha256:" + hashlib.sha256(data).hexdigest()
        if references[logical]["sha256"] != expected or sha(workspace / logical) != expected:
            raise ValueError("frozen input differs from producer revision: " + source)


def digest_argument(value):
    value = value.removeprefix("sha256:")
    if re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise argparse.ArgumentTypeError("expected a lowercase SHA256 digest")
    return "sha256:" + value


def file_reference(path, logical):
    path = Path(path).absolute()
    if path.is_symlink() or not path.is_file():
        raise ValueError("expected an ordinary reviewed input: " + str(path))
    return {"path": logical, "source": str(path), "sha256": sha(path), "size": path.stat().st_size}


def requirements(args):
    if args.stop_after != "package":
        return [], None
    allocation, selected = load_requirements(
        args.case_allocation,
        args.case_allocation_sha256,
        args.requirements,
        profile_id="c07-" + args.route + "-arkts-aarch64-ohos23",
        route="madb.arkts-" + args.route + "-napi",
    )
    if args.variant == "test":
        selected = next(
            p for p in allocation["profiles"] if p["profile_id"] == selected["relation_profile"]
        )
    files = [
        file_reference(args.case_allocation, "validation/case-allocation.json"),
        file_reference(args.requirements, "validation/route-requirements.json"),
    ]
    return files, selected


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", required=True, type=Path, help="new output root; never overwrite"
    )
    parser.add_argument("--route", choices=("c", "cpp"), default="c")
    parser.add_argument("--variant", choices=("ordinary", "test"), default="ordinary")
    parser.add_argument("--madb-image", required=True, type=Path)
    parser.add_argument("--madb-sha256", required=True, type=digest_argument)
    parser.add_argument("--finalizer-image", required=True, type=Path)
    parser.add_argument("--finalizer-sha256", required=True, type=digest_argument)
    parser.add_argument("--stop-after", choices=("generate", "build", "package"), default="package")
    parser.add_argument("--cli-root", required=True, type=Path)
    parser.add_argument("--cli-inventory", required=True, type=Path)
    parser.add_argument("--cli-inventory-sha256", required=True, type=digest_argument)
    parser.add_argument("--python-image", required=True, type=Path)
    parser.add_argument("--java-home", required=True, type=Path)
    parser.add_argument("--case-allocation", type=Path)
    parser.add_argument("--case-allocation-sha256", type=digest_argument)
    parser.add_argument("--requirements", type=Path)
    parser.add_argument(
        "--author-fixture", type=Path, help="actual template + explicit policy; generate only"
    )
    parser.add_argument(
        "--allow-dirty", action="store_true", help="emit provisional development artifacts"
    )
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("--output must not exist")
    if args.author_fixture is not None and (
        args.stop_after != "generate" or args.variant != "ordinary"
    ):
        parser.error("--author-fixture is restricted to the public generation check")
    if args.stop_after == "package" and any(
        v is None for v in (args.case_allocation, args.case_allocation_sha256, args.requirements)
    ):
        parser.error(
            "Package requires independent --case-allocation, --case-allocation-sha256 and --requirements"
        )
    return args


def main():
    args = arguments()
    revision, dirty = checkout_state()
    if dirty and not args.allow_dirty:
        raise ValueError(
            "formal artifacts require a clean producer checkout; --allow-dirty is provisional only"
        )
    delivery, selected = requirements(args)
    inventory = file_reference(args.cli_inventory, "cli-inventory.json")
    if inventory["sha256"] != args.cli_inventory_sha256:
        raise ValueError("CLI inventory differs from the caller-reviewed SHA256")
    images = {}
    for name, path, expected in (
        ("madb-host", args.madb_image, args.madb_sha256),
        ("madb-finalizer", args.finalizer_image, args.finalizer_sha256),
    ):
        image = file_reference(path, name)
        if image["sha256"] != expected:
            raise ValueError("caller-pinned executable differs: " + name)
        images[name] = image
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    result = {
        "scope": "MadB C04 source/read/generate/compile/link/HAR; independent target verification remains missing",
        "route": args.route,
        "variant": args.variant,
        "producer_revision": revision,
        "provisional": args.allow_dirty,
        "requested_stage": args.stop_after,
        "target_executed": False,
        "verification_status": "missing",
    }
    try:
        # These are caller-pinned tool images, not an implicit Cargo build or
        # a claim that checksum agreement proves their source provenance.
        for name, image in images.items():
            destination = output / name
            shutil.copyfile(image["source"], destination)
            if sha(destination) != image["sha256"] or sha(image["source"]) != image["sha256"]:
                raise ValueError("tool image changed while being frozen: " + name)
            destination.chmod(0o500)
        host = output / "madb-host"
        finalizer = output / "madb-finalizer"
        result["host"] = {"sha256": sha(host), "size": host.stat().st_size}
        result["finalizer"] = {"sha256": sha(finalizer), "size": finalizer.stat().st_size}
        inventory_ref = {k: inventory[k] for k in ("sha256", "size")} | {
            "path": str(args.cli_inventory.absolute())
        }
        observation = observe(
            ROOT, args.cli_root, inventory_ref, args.python_image, args.java_home, finalizer, host
        )
        (output / "tool-observation.json").write_text(json.dumps(observation, indent=2) + "\n")
        if not args.allow_dirty:
            require_revision(revision)
        workspace = output / "workspace"
        prepared = prepare_route(
            ROOT,
            workspace,
            observation,
            route=args.route,
            variant=args.variant,
            author_fixture=args.author_fixture,
            delivery_inputs=delivery,
        )
        if not args.allow_dirty:
            verify_revision_inputs(workspace, prepared, revision)
            require_revision(revision)
        result["input_manifest"] = str(workspace / "madb.lock")
        result["read_request"] = str(workspace / "read.json")
        pipeline = Pipeline(ROOT, workspace, prepared, host)
        pipeline.metadata()
        result["metadata"] = "host-checked"
        result["contract"] = str(workspace / "contract.json")
        pipeline.generate()
        result.update(source=str(pipeline.source), generation_id=pipeline.outputs["generation_id"])
        if args.stop_after != "generate":
            pipeline.build()
            result["build"] = str(pipeline.built)
        if args.stop_after == "package":
            verify_refs(workspace, delivery)
            _, current = requirements(args)
            if current != selected:
                raise ValueError("independent case allocation changed before Package")
            candidate = pipeline.package(
                output / "candidate",
                revision,
                selected["required_cases"],
                provisional=args.allow_dirty,
            )
            result.update(
                candidate_digest=candidate["candidate_digest"],
                required_cases=selected["required_cases"],
            )
            result["related_pair_requirement"] = (
                "ordinary five plus separate companion twenty-five; this candidate alone does not certify C07"
            )
        if not args.allow_dirty:
            verify_revision_inputs(workspace, prepared, revision)
            require_revision(revision)
        result["completed_requested_stage"] = True
    except Exception:
        result["completed_requested_stage"] = False
        result["error"] = traceback.format_exc()
        raise
    finally:
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print("ArkTS build failed: " + str(error), file=sys.stderr)
        raise SystemExit(1) from error
