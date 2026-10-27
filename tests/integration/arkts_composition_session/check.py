"""Actual ordinary /2 package, JSONL generation and Source transaction checks.

Generated Source consumption is not HAR/target-runtime certification.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
BASE = ROOT / "tests/integration/arkts_composition_recipe"
sys.path.insert(0, str(BASE))
import check_generate_inputs as fixture
from adapters.toolchains.common import documents as d

ENTRY = "crates/madb-cli/examples/arkts_composition_peer.rs"
SOURCES = [ENTRY, "crates/madb-cli/examples/arkts_composition_peer/business.rs", "adapters/generators/arkts/composition_peer.py"]
TOOL = "madb-arkts-composition-peer"
SOURCE = "local-fixed-composition-source"
OUTPUT = "outputs/request-1"


def command(argv, *, cwd, input=None, reject=None):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, input=input, capture_output=True, timeout=240,
                            env=fixture.support.clean_environment())
    if reject is not None:
        if result.returncode == 0 or reject not in result.stderr.decode(errors="replace"):
            raise AssertionError(f"expected {reject}: {result.returncode} {result.stderr.decode(errors='replace')}")
    elif result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def save(root, name, value):
    d.save(root, name, value)


def prepare(args, root):
    config, read = fixture.prepare(root, args.observation, args.bridge)
    for name in SOURCES:
        fixture.support.copy_new(ROOT / name, root / name)
        fixture.support.copy_new(ROOT / name, root / fixture.PREFIX / name)
    manifest = fixture.support.read_json(root / fixture.PREFIX / "extension.json")
    manifest["components"][0]["entrypoint"] = {"kind": "process", "path": ENTRY, "args": []}
    save(root / fixture.PREFIX, "extension.json", manifest)
    composition = fixture.support.read_json(root / "composition.json")
    package = composition["packages"][0]
    names = sorted({r["path"] for r in package["source"]["files"]} | set(SOURCES))
    package["source"]["files"] = [d.ref(root / fixture.PREFIX, n) for n in names]
    package["manifest"] = d.ref(root / fixture.PREFIX, "extension.json", "madb.extension/1")
    package["source_digest"] = d.semantic("madb.source/1", package["source"]["files"])
    composition["components"][0]["sha256"] = package["source_digest"]
    save(root, "composition.json", composition)
    image = args.peer.read_bytes()
    tool = {"id": TOOL, "version": "0.1.0", "sha256": "sha256:" + hashlib.sha256(image).hexdigest(),
            "host": "aarch64-apple-darwin", "target": "aarch64-linux-ohos", "source": SOURCE, "arguments": []}
    profile = fixture.support.read_json(root / "profile.json")
    profile["composition"]["file"] = d.ref(root, "composition.json", "madb.composition/1")
    profile["composition"]["semantic_digest"] = fixture.bridge_command(args.bridge, "composition-digest", root).decode().strip()
    profile["tools"].append(tool)
    # Same installed bytes do not make a different logical host/source tool admissible.
    profile["tools"].append(dict(tool, id="unobserved-peer-alias", version="9.9.9",
                                 host="x86_64-unknown-linux-gnu", source="unobserved-source"))
    save(root, "profile.json", profile)
    profile_digest = fixture.bridge_command(args.bridge, "profile-digest", root).decode().strip()
    project = fixture.support.read_json(root / "project.json")
    project["boundaries"][0]["profile"] = d.ref(root, "profile.json", "madb.profile/2")
    save(root, "project.json", project)
    existing = {r["path"]: r["schema"] for r in read["inputs"]}
    existing["project.json"] = "madb.project/1"
    existing.update({p: "madb.source-input/1" for n in SOURCES for p in (n, fixture.PREFIX + "/" + n)})
    read["inputs"] = [d.ref(root, p, schema) for p, schema in sorted(existing.items())]
    read["selector"]["profile_digest"] = profile_digest
    read["component"] = composition["components"][0]
    lock = fixture.support.read_json(root / "lock.json")
    lock.update(project_digest=d.project_digest(project), tools=profile["tools"], components=composition["components"],
                inputs=[{k:r[k] for k in ("path","sha256","size")} for r in read["inputs"]],
                profiles=[{"id":profile["id"], "semantic_digest":profile_digest, "file":d.ref(root,"profile.json","madb.profile/2")}])
    save(root, "lock.json", lock)
    read.update(lock=d.ref(root,"lock.json","madb.lock/1"), profile=d.ref(root,"profile.json","madb.profile/2"))
    save(root, "read-request.json", read)
    work = root.parent / "metadata-work"
    work.mkdir()
    argv = [config["tools"]["python"]["path"], *config["tools"]["python"]["invocation_arguments"],
            root / fixture.PREFIX / fixture.ENTRY, "metadata", "--config", root / "backend.json"]
    contract = command(argv, cwd=work, input=d.canonical(read).encode())
    (root / "contract.json").write_bytes(contract)
    ir = json.loads(contract)
    request = dict(read, schema="madb.generate-request/1", output_root=OUTPUT,
                   contract=d.ref(root,"contract.json","madb.contract-ir/1"), input_digest=ir["source_digest"],
                   contract_digest=ir["structure_digest"], policy_digest=ir["policy_digest"], task_digest="sha256:"+"0"*64)
    save(root,"generate-request.json",request)
    tools = root.parent / "tools"
    tools.mkdir(); shutil.copy2(args.peer,tools/"arkts-peer")
    request["task_digest"] = command([args.probe,"task",root,tools],cwd=root.parent).decode().strip()
    save(root,"generate-request.json",request)
    return tools


def run(args):
    output=args.output.absolute()
    if output.exists() or output.is_symlink(): raise ValueError("output must be new")
    output.mkdir(parents=True)
    root=output/"inputs"
    tools=prepare(args,root)
    records=[]
    admission=json.loads(command([args.probe,"admit",root,tools],cwd=output))
    assert admission["admitted"] and admission["source_digest"]!=admission["image_digest"]
    records.append({"case":"distinct-package-source-and-image-admission","result":"passed"})
    alias=json.loads(command([args.probe,"reject-tool-alias",root,tools],cwd=output))
    assert alias == {"rejected":True,"code":"ExtensionIdentityMismatch","path":"recipe"}
    records.append({"case":"same-image-logical-tool-alias-rejected","result":"passed"})
    for mode in ("ok","cancel-before","cancel-result","cancel-commit","fail-output"):
        run_root=output/("run-"+mode)
        result=json.loads(command([args.probe,"run",root,tools,run_root,mode],cwd=output))
        (output/(mode+".json")).write_text(json.dumps(result,indent=2)+"\n")
        if mode=="ok":
            assert result["committed"] and not result["index_unchanged"]
            assert result["execution"]["process"]["cleanup_complete"]
        else:
            assert not result["committed"] and result["index_unchanged"]
        execution = result["execution"]
        if mode in ("cancel-before", "cancel-result"):
            assert execution["admission"] == "RejectedByCancellation"
        elif mode == "fail-output":
            assert execution["admission"] == "Unavailable"
        else:
            assert execution["admission"] == "EligibleForValidation"
        if mode == "cancel-before":
            assert execution["process"] is None
        else:
            process = execution["process"]
            assert process["cleanup_complete"] and not process["pending_cleanup"]
            assert process["termination"] == "Exited(ProcessExit { code: Some(0), signal: None })"
        if mode == "cancel-commit":
            assert "STORE002" in result["error"] and "cancelled before commit preparation" in result["error"]
        if mode == "fail-output":
            assert execution["process"]["diagnostic"]["code"] == "OutputInvalid"
        records.append({"case":mode,"result":"passed","source_batch":result["source_batch"]})
    # Independent public Source consumer: no helper workspace/private objects.
    public=output/"run-ok/public-source"
    manifest=json.loads((public/"outputs.json").read_bytes())
    actual=sorted(p.relative_to(public).as_posix() for p in public.rglob("*") if p.is_file())
    assert actual==sorted(["outputs.json",*fixture.PAYLOADS])
    for ref in manifest["files"]:
        data=(public/ref["path"]).read_bytes()
        assert len(data)==ref["size"] and "sha256:"+hashlib.sha256(data).hexdigest()==ref["sha256"]
    assert "ArkUIStateModel.ets" not in " ".join(actual)
    records.append({"case":"exported-source-inventory-and-byte-consumer","result":"passed","payloads":15})
    consumer=Path(__file__).with_name("consume_source.py")
    observed=json.loads(command([sys.executable,"-I","-S","-B",consumer,public],cwd=output))
    assert observed["consumed_payloads"]==15 and observed["generation_id"]==manifest["generation_id"]
    copied=output/"consumer-tampered"
    shutil.copytree(public,copied)
    (copied/"generated/facade/RuntimeFrame.ets").write_bytes(b"tampered")
    command([sys.executable,"-I","-S","-B",consumer,copied],cwd=output,reject="public payload byte identity differs")
    records.append({"case":"standalone-source-consumer-and-byte-tamper-rejection","result":"passed"})
    (output/"checks.json").write_text(json.dumps({"scope":"ordinary /2 Source generation only","checks":records,
        "not_claimed":["multi-component execution","SDK/native build","HAR/target consumer","whole-package acceptance"]},indent=2)+"\n")
    print(json.dumps({"checks":len(records),"output":str(output)}))

if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--bridge",type=Path,required=True)
    parser.add_argument("--probe",type=Path,required=True)
    parser.add_argument("--peer",type=Path,required=True)
    parser.add_argument("--observation",type=Path,required=True)
    run(parser.parse_args())
