"""Host gates reject self-reported tools and actual post-execution byte drift."""
import argparse
from copy import copy
import json
from pathlib import Path
import shutil
import subprocess
import inputs
from check_source import pin,inventory


def main():
    parser=argparse.ArgumentParser()
    for name in ("output","candidate","clang","sdk","bridge","peer","ffigen","libclang","dart","runtime-archives"):
        parser.add_argument("--"+name,required=True,type=Path)
    args=parser.parse_args();output=args.output.absolute()
    protected=[inputs.ROOT.resolve(),*(getattr(args,n.replace('-','_')).resolve() for n in ("candidate","clang","sdk","bridge","peer","ffigen","libclang","dart","runtime-archives"))]
    if output.exists() or output.parent.resolve()!=output.parent or any(output.is_relative_to(p) or p.is_relative_to(output) for p in protected):raise ValueError("fresh independent root required")
    output.mkdir();original_tools=[pin(getattr(args,n))for n in ("bridge","peer","ffigen","dart")];records=[]
    for layer in ("source","build","package","backend-mismatch","sdk-arguments-mismatch"):
        root=output/layer;root.mkdir();(root/"mutable-tools").mkdir();mutable=root/"mutable-tools/ffigen";shutil.copyfile(args.ffigen,mutable);mutable.chmod(0o500)
        options=copy(args);options.ffigen=mutable;inputs.prepare(options,root/"inputs");before=inventory(root/"inputs")
        if layer in ("backend-mismatch","sdk-arguments-mismatch"):
            config=inputs.wire.read_json(root/"inputs/backend.json")
            if layer=="backend-mismatch":
                fake=root/"mutable-tools/dart";data=Path(config["tools"]["dart"]["path"]).read_bytes();data=data[:-1]+bytes([data[-1]^1]);fake.write_bytes(data)
                config["tools"]["dart"].update(path=str(fake),sha256=inputs.wire.digest(data),size=len(data));expected=b"backend tool tuple differs from profile/lock"
            else:
                config["sdk_root"]=str(root);expected=b"backend SDK root differs from locked clang arguments"
            (root/"inputs/backend.json").write_text(inputs.wire.canonical(config)+"\n")
            lock=inputs.wire.read_json(root/"inputs/lock.json");next(f for f in lock["inputs"] if f["path"]=="backend.json").update(inputs.wire.reference(root/"inputs","backend.json"));(root/"inputs/lock.json").write_text(inputs.wire.canonical(lock)+"\n")
            result=subprocess.run([str(args.bridge),"verify",str(root/"inputs")],capture_output=True,timeout=120);(root/"verify.stdout").write_bytes(result.stdout);(root/"verify.stderr").write_bytes(result.stderr)
            assert result.returncode!=0 and expected in result.stderr,result.stderr
            records.append({"case":layer,"expected_rejection":result.stderr.decode()});continue
        (root/"tools").mkdir();shutil.copyfile(args.peer,root/"tools/dart-peer");config=inputs.wire.read_json(root/"inputs/backend.json");shutil.copyfile(config["tools"]["python"]["path"],root/"tools/python")
        for file in (root/"tools").iterdir():file.chmod(0o500)
        commands=[]
        for step in ("source","build","package"):
            if step=="source":parameters=["chain",str(root/"inputs"),str(root/"tools"),str(root/"source-run"),"external-tool-drift" if layer==step else "ok"]
            else:parameters=[step,str(root/"inputs"),str(root/"tools"),str(root/"source-run"),str(root/(step+"-run"))]+(["external-tool-drift"]if layer==step else [])
            argv=[str(args.bridge),*parameters];result=subprocess.run(argv,capture_output=True,timeout=600);(root/(step+".stdout")).write_bytes(result.stdout);(root/(step+".stderr")).write_bytes(result.stderr);commands.append({"argv":argv,"exit_code":result.returncode});(root/"commands.json").write_text(json.dumps(commands,indent=2));assert result.returncode==0,result.stderr.decode()
            value=json.loads(result.stdout)
            if step==layer:
                assert not value["committed"] and value["index_unchanged"],value
                assert "external tool/header bytes differ" in str(value.get("error",value.get("failure"))),value
                fault=inputs.wire.read_json(root/(step+"-run")/"tool-fault.json");assert fault["before"]!=fault["after"]
                records.append({"case":layer,"report":pin(root/(step+".stdout")),"fault":fault});break
            assert value["committed"],value
        assert inventory(root/"inputs")==before
    assert [pin(p["path"])for p in original_tools]==original_tools
    (output/"result.json").write_text(json.dumps({"status":"passed","checks":records,"original_tools_unchanged":original_tools},indent=2));print("passed five tool gates")


if __name__=="__main__":main()
