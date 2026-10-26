"""Build one finite natural-Rust values candidate through the existing frozen ports."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from adapters.toolchains.rust_c.common import sha_file
from adapters.toolchains.rust_c.tools import discover
from adapters.toolchains.rust_c.values_project import prepare
from adapters.toolchains.rust_c.values_pipeline import Pipeline


def revision():
    return subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()


def require_revision(expected):
    dirty=subprocess.check_output(["git","status","--porcelain","--untracked-files=normal"],cwd=ROOT,text=True)
    if revision()!=expected or dirty:raise RuntimeError("producer checkout changed during formal value preparation")


def verify_sources(root, prepared, expected):
    dynamic={"author.rs","backend.json","toolchain.json","values-cbindgen.toml","values-base-cbindgen.toml"}
    for item in prepared["read"]["inputs"]+prepared["lock"]["schemas"]:
        if item["path"] in dynamic:continue
        data=subprocess.check_output(["git","show",expected+":"+item["path"]],cwd=ROOT)
        if "sha256:"+hashlib.sha256(data).hexdigest()!=item["sha256"] or sha_file(root/item["path"])!=item["sha256"]:
            raise RuntimeError("frozen helper/schema differs from producer commit: "+item["path"])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source",type=Path,required=True,help="single natural Rust author module")
    parser.add_argument("--namespace",required=True)
    parser.add_argument("--variant",required=True,help="explicit boundary/profile label")
    parser.add_argument("--required-case",action="append",required=True,help="predefined independent Lab case ID; repeat as needed")
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--bindgen",type=Path,required=True);parser.add_argument("--cbindgen",type=Path,required=True)
    parser.add_argument("--allow-dirty",action="store_true",help="development only; provisional candidate is not formal Lab acceptance")
    args=parser.parse_args();output=args.output.absolute();author=args.source.resolve(strict=True)
    if output.exists() or output.is_symlink():parser.error("output must not exist")
    if len(set(args.required_case))!=len(args.required_case):parser.error("duplicate required case")
    claimed=revision()
    if not args.allow_dirty:require_revision(claimed)
    source_digest=sha_file(author)
    env=dict(os.environ,CARGO_TARGET_DIR=str(ROOT/"target"))
    subprocess.run(["cargo","build","--locked","-p","madb-cli","--bins"],cwd=ROOT,env=env,check=True)
    observation=discover(ROOT,args.bindgen,args.cbindgen)
    work=Path(tempfile.mkdtemp(prefix="madb-native-values-")).resolve();host=work/"madb-host"
    shutil.copyfile(ROOT/"target/debug/madb",host);host.chmod(0o500)
    print("Native values execution workspace: "+str(work),flush=True)
    if args.allow_dirty:print("PROVISIONAL development candidate",flush=True)
    root=work/"managed"
    prepared=prepare(ROOT,root,observation,author,args.namespace,args.variant,ROOT/"target/debug/madb-native-values-read")
    if source_digest!=sha_file(author) or source_digest!=sha_file(root/"author.rs"):
        raise RuntimeError("author input changed while freezing")
    if not args.allow_dirty:verify_sources(root,prepared,claimed);require_revision(claimed)
    pipeline=Pipeline(ROOT,root,prepared,host);pipeline.metadata();pipeline.generate();pipeline.build()
    candidate_root=work/"candidate"
    candidate=pipeline.package(candidate_root,claimed,sorted(args.required_case),provisional=args.allow_dirty)
    if source_digest!=sha_file(author):raise RuntimeError("author input changed during execution")
    if not args.allow_dirty:require_revision(claimed)
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".madb-value-export-",dir=output.parent) as temporary:
        staged=Path(temporary)/"candidate"
        shutil.copytree(candidate_root,staged)
        if source_digest!=sha_file(author):raise RuntimeError("author input changed during export preparation")
        if not args.allow_dirty:require_revision(claimed)
        if output.exists() or output.is_symlink():raise RuntimeError("candidate output appeared during execution")
        staged.rename(output)
    print(args.variant+": "+candidate["candidate_digest"],flush=True)
    print("Public candidate: "+str(output),flush=True)

if __name__=="__main__":main()
