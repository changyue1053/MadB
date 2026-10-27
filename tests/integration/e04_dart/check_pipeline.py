"""Exercise one real Source -> Build -> Package candidate; never updates catalog."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from datetime import datetime, timezone
import inputs
from check_source import pin, inventory


def main():
    parser = argparse.ArgumentParser()
    for name in ("output", "candidate", "clang", "sdk", "bridge", "peer", "ffigen", "libclang", "dart", "runtime-archives"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    root = args.output.absolute()
    protected = [inputs.ROOT.resolve(), *(getattr(args,n.replace('-','_')).resolve() for n in ("candidate", "clang", "sdk", "bridge", "peer", "ffigen", "libclang", "dart", "runtime-archives"))]
    if root.exists() or root.is_symlink() or root.parent.resolve()!=root.parent or any(root.is_relative_to(p) or p.is_relative_to(root) for p in protected):
        raise ValueError("fresh independent output required")
    root.mkdir(); started=datetime.now(timezone.utc).isoformat()
    inputs.prepare(args,root/"inputs"); (root/"tools").mkdir()
    config=json.loads((root/"inputs/backend.json").read_text())
    for source,dest in ((args.peer,"dart-peer"),(config["tools"]["python"]["path"],"python")):
        shutil.copyfile(source,root/"tools"/dest);(root/"tools"/dest).chmod(0o500)
    before={"inputs":inventory(root/"inputs"),"tools":[pin(args.bridge),pin(args.peer),*[pin(t["path"]) for t in config["tools"].values()]]}
    (root/"before.json").write_text(json.dumps(before,indent=2))
    commands=[]
    modes=[("source",["chain",str(root/"inputs"),str(root/"tools"),str(root/"source-run"),"ok"]),
           ("build",["build",str(root/"inputs"),str(root/"tools"),str(root/"source-run"),str(root/"build-run")]),
           ("package",["package",str(root/"inputs"),str(root/"tools"),str(root/"source-run"),str(root/"package-run")])]
    for name,parameters in modes:
        argv=[str(args.bridge),*parameters];result=subprocess.run(argv,capture_output=True,timeout=600)
        (root/(name+".stdout")).write_bytes(result.stdout);(root/(name+".stderr")).write_bytes(result.stderr)
        commands.append({"argv":argv,"exit_code":result.returncode,"stdout":pin(root/(name+".stdout")),"stderr":pin(root/(name+".stderr"))})
        (root/"commands.json").write_text(json.dumps(commands,indent=2))
        if result.returncode:raise AssertionError(result.stderr.decode())
        record=json.loads(result.stdout)
        if not record["committed"]:raise AssertionError(f"{name} not committed; inspect retained raw report")
        print(name,"committed",flush=True)
    after={"inputs":inventory(root/"inputs"),"tools":[pin(p["path"]) for p in before["tools"]]}
    (root/"after.json").write_text(json.dumps(after,indent=2));assert before==after
    (root/"result.json").write_text(json.dumps({"status":"passed","started_at":started,"finished_at":datetime.now(timezone.utc).isoformat(),"scope":"Source+Build+Package developer checks","not_claimed":["independent Lab","whole E04"]},indent=2))


if __name__=="__main__":main()
