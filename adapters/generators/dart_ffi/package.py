"""Existing PackageRequest -> Candidate port; the host fixes every byte mapping."""
import json
from copy import deepcopy
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
import wire


def main():
    raw = sys.stdin.buffer.read(wire.MAX_DOCUMENT + 1)
    if len(raw)>wire.MAX_DOCUMENT:raise ValueError("bounded package request required")
    request=json.loads(raw);work=Path.cwd();root=work.parent/"inputs"
    if request["schema"]!="madb.package-request/1":raise ValueError("package request required")
    mapping=wire.read_json(wire.confined(root,"package-map.json"))
    files=[]
    for item in mapping:
        source=item["source"];wire.verify(root,[source]);data=wire.confined(root,source["path"]).read_bytes()
        wire.write_new(work,item["destination"],data);files.append(wire.reference(work,item["destination"]))
    value={"schema":"madb.candidate/1","selector":request["selector"],"generation_id":request["generation_id"],"contract_digest":request["contract_digest"],"policy_digest":request["policy_digest"],"lock_digest":request["lock"]["sha256"],"manifest_path":"candidate.json","files":sorted(files,key=lambda f:f["path"]),"dependencies":request["dependencies"],"public_symbols":request["public_symbols"],"required_cases":request["required_cases"],"build_evidence":request["build_evidence"]}
    projection=deepcopy(value)
    for field in ("files","dependencies","build_evidence"):projection[field].sort(key=lambda f:f["path"])
    for field in ("public_symbols","required_cases"):projection[field].sort()
    value["candidate_digest"]=wire.semantic("madb.candidate/1",projection)
    data=(wire.canonical(value)+"\n").encode();wire.write_new(work,"candidate.json",data);sys.stdout.buffer.write(data)


if __name__=="__main__":main()
