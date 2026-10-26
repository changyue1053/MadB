"""Finite value recipe over the existing bounded C04 compile/link/run worker mechanics."""
from __future__ import annotations
import argparse
import os
from pathlib import Path
import shutil
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from worker import (Commands, LOG_PATH, MAX_JSON, canonical, check_symbols, clang_flags, decode_json,
                    file_reference, input_path, load_json, platform_kind, verify_task_inputs, verify_tools)


def namespace(input_root):
    return load_json(input_path(input_root, "reader-plan.json"))["namespace"]


def phase_outputs(config, phase, ns):
    if phase == "compile": return ["objects/libvalues.a"]
    if phase == "link": return ["lib/lib" + ns + (".dylib" if platform_kind(config) == "apple" else ".so"), "native/smoke"]
    if phase == "run": return ["native/run.json"]
    raise ValueError("finite value phase must be compile, link or run")


def compile_provider(config, root, work, commands):
    input_path(root, "author.rs")
    source = input_path(root, "values_shim.rs")
    commands.run("rustc", ["--target", config["rust_target"], "--edition=2024", "--crate-name=madb_native_values",
        "--crate-type=staticlib", "-Copt-level=2", "-Cpanic=unwind", "-Cdebuginfo=0", "-Crelocation-model=pic", "-Ccodegen-units=1",
        "--remap-path-prefix", str(root)+"=/madb/inputs", "--remap-path-prefix", str(work)+"=/madb/work", source, "-o", work/"objects/libvalues.a"])


def link_provider(config, root, work, scratch, commands, ns):
    plan = load_json(input_path(root, "reader-plan.json"))
    symbols = [ns+"_bootstrap_v1", *[ns+"_raw_"+f["name"] for f in plan["functions"]]]
    identity = load_json(input_path(root, "identity.json"))
    platform = platform_kind(config); library = work/phase_outputs(config,"link",ns)[0]
    archive = input_path(root,"objects/libvalues.a"); exports = scratch/"exports.txt"
    if platform == "apple":
        exports.write_text("".join("_"+s+"\n" for s in sorted(symbols)))
        link = ["-dynamiclib", "-Wl,-force_load,"+str(archive), "-Wl,-exported_symbols_list,"+str(exports),
                "-Wl,-install_name,@rpath/"+library.name, "-Wl,-reproducible", "-Wl,-S", "-Wl,-dead_strip", "-framework", "Security", "-liconv", "-lresolv"]
    else:
        exports.write_text("{ global: "+" ".join(s+";" for s in sorted(symbols))+" local: *; };\n")
        link = ["-shared", "-Wl,--whole-archive", archive, "-Wl,--no-whole-archive", "-Wl,--version-script="+str(exports),
                "-Wl,-soname,"+library.name, "-Wl,--build-id=none", "-lpthread", "-ldl", "-lm", "-lrt", "-lutil"]
    flags = clang_flags(config,root,work)
    commands.run("clang", [*flags,*link,"-o",library])
    check_symbols(config,commands,library,symbols,platform)
    payload = bytes.fromhex(identity["interface_digest"][7:])+bytes.fromhex(identity["profile_digest"][7:])
    if len(payload)!=64: raise ValueError("invalid finite bootstrap identity")
    smoke = scratch/"smoke.c"
    smoke.write_text('#include "'+ns+'_native.h"\n#include <stdio.h>\n#include <string.h>\n'
        +'int main(void) { const uint8_t expected[64] = {'+','.join(map(str,payload))+'}; uint8_t actual[64] = {0};\n'
        +'if ('+ns+'_bootstrap_v1(1,actual,64)!=0 || memcmp(actual,expected,64)!=0) return 1;\n'
        +'puts("MADB_NATIVE_VALUES_SMOKE_OK"); return 0; }\n')
    flags += ["-I",root/"include",smoke,"-L",work/"lib","-l"+ns]
    flags += ["-Wl,-reproducible","-Wl,-S","-Wl,-rpath,@loader_path/../lib"] if platform=="apple" else ["-Wl,--build-id=none","-Wl,-rpath,$ORIGIN/../lib"]
    commands.run("clang",[*flags,"-o",work/"native/smoke"])


def run_smoke(config, root, work, scratch, commands, ns):
    exe=scratch/"native/smoke";exe.parent.mkdir();shutil.copyfile(input_path(root,"native/smoke"),exe);exe.chmod(0o700)
    name=phase_outputs(config,"link",ns)[0]; target=scratch/name;target.parent.mkdir();shutil.copyfile(input_path(root,name),target)
    if commands.run("smoke",[],executable=exe,cwd=scratch)!=b"MADB_NATIVE_VALUES_SMOKE_OK\n":
        raise RuntimeError("finite value bootstrap smoke marker differs")
    (work/"native/run.json").write_bytes(canonical(dict(exit_code=0, status="passed", target=config["rust_target"], scope="bootstrap-only", namespace=ns))+b"\n")


def run_phase(config,phase,input_root,work):
    root,work=Path(input_root).resolve(strict=True),Path(work).resolve(strict=True)
    if root==work or root.is_relative_to(work) or work.is_relative_to(root) or any(work.iterdir()):
        raise ValueError("finite build requires disjoint frozen inputs and empty work")
    if config.get("semantics")!="madb.native-values/1" or config.get("provider")!="rust":
        raise ValueError("incorrect finite value backend")
    ns=namespace(root);outputs=phase_outputs(config,phase,ns);verify_tools(config)
    try:
        for n in outputs:(work/n).parent.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="value-build-",dir=work) as temp:
            scratch=Path(temp);commands=Commands(config,root,work,scratch)
            if phase=="compile":compile_provider(config,root,work,commands)
            elif phase=="link":link_provider(config,root,work,scratch,commands,ns)
            else:run_smoke(config,root,work,scratch,commands,ns)
        actual={p.relative_to(work).as_posix() for p in work.rglob('*') if p.is_file()}
        if actual!={*outputs,LOG_PATH}:raise RuntimeError("finite worker produced unexpected files")
        return outputs
    finally:verify_tools(config)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--config",required=True,type=Path);args=parser.parse_args()
    try:
        data=sys.stdin.buffer.read(MAX_JSON+1)
        if len(data)>MAX_JSON:raise ValueError("BuildTask exceeds 1 MiB")
        task=decode_json(data);work=Path.cwd().resolve();root=(work.parent/"inputs").resolve(strict=True)
        path=args.config.resolve(strict=True);script=Path(__file__).resolve(strict=True)
        if not path.is_relative_to(root) or not script.is_relative_to(root):raise ValueError("worker/config must be frozen inputs")
        expected=[os.path.relpath(script,work),"--config",os.path.relpath(path,work)]
        config=load_json(path)
        if task.get("schema")!="madb.build-task/1" or task["arguments"]!=expected or task["selector"]["target"]!=config["rust_target"]:
            raise ValueError("finite task arguments/target differ")
        if sorted(task["outputs"])!=sorted(phase_outputs(config,task["phase"],namespace(root))):raise ValueError("finite declared outputs differ")
        declared={r["path"] for r in task["inputs"]}
        if not {path.relative_to(root).as_posix(),script.relative_to(root).as_posix()}<=declared:raise ValueError("missing finite worker/config reference")
        verify_task_inputs(task,root);outputs=run_phase(config,task["phase"],root,work);verify_task_inputs(task,root)
        result={k:task[k] for k in ("input_digest","task_digest","generation_id","tool","phase")}
        result.update(schema="madb.build-evidence/1",task_id=task["id"],profile_digest=task["selector"]["profile_digest"],target=task["selector"]["target"],
                      status="passed",exit_code=0,outputs=[file_reference(work/n,work) for n in outputs],logs=[file_reference(work/LOG_PATH,work)])
        sys.stdout.buffer.write(canonical(result)+b"\n")
    except (KeyError,OSError,RuntimeError,TypeError,ValueError) as error:
        sys.stderr.write("native values build: "+str(error)+"\n");raise SystemExit(2) from error

if __name__=="__main__":main()
