"""Provided responsibilities for the actual generated finite value plan and frozen inputs."""
from ownership import _escape, _row, _reference, _read, _origin, _value, _slot
from support import NativeError, load_json, semantic
from lowering import source_id

READER = "org.madb.native-values/reader-generator@0.1.0"
NATIVE = "madb-native-values@1"


def render_ownership(contract, request, config, input_root, plan):
    if config.get("provider")!="rust" or config.get("semantics")!="madb.native-values/1" or plan["namespace"]!=contract["sdk_namespace"]:
        raise NativeError("NATIVE006", "value responsibility context differs")
    lock=load_json(_read(request["lock"],input_root));profile=load_json(_read(request["profile"],input_root))
    inputs={r["path"]:_reference(r) for r in request["inputs"]};locked={r["path"]:_reference(r) for r in lock["inputs"]}
    if len(inputs)!=len(request["inputs"]) or inputs!=locked:
        raise NativeError("NATIVE006", "value responsibility input inventory differs from lock")
    for r in inputs.values():_read(r,input_root)
    if load_json(_read(inputs["backend.json"],input_root))!=config:raise NativeError("NATIVE006", "value backend changed")
    component=request["component"]
    component_files=[r for r in request["inputs"] if r["path"].startswith("adapters/generators/rust_c/") or r["path"].startswith("adapters/contracts/src/native_values") or r["path"]=="crates/madb-cli/src/bin/madb-native-values-read.rs"]
    if (component["id"],component["version"])!=("org.madb.native-values/reader-generator","0.1.0") or component not in lock["components"] or component["sha256"]!=semantic("madb.native-component/1",component_files):
        raise NativeError("NATIVE006", "value component bytes differ")
    if contract["selector"]!=request["selector"] or profile["id"]!=request["selector"]["profile_id"]:
        raise NativeError("NATIVE006", "value responsibility selector differs")
    sdk=profile["sdk"]
    if inputs.get(sdk["path"])!=_reference(sdk):raise NativeError("NATIVE006", "value SDK metadata is not frozen")
    symbols={s["public_id"]:s for s in contract["symbols"]};prefix=plan["namespace"]
    project=load_json(input_root/config["project"])
    def public_id(name):
        key=dict(reader="org.madb.native-values@1",package=project["project"],module=config["source"],kind="function",qualified_name=name,overload="none")
        return prefix+"."+source_id(key)[10:]
    expected={public_id("$recipe.bootstrap.v1"):prefix+"_bootstrap_v1",**{public_id(f["name"]):prefix+"_raw_"+f["name"] for f in plan["functions"]}}
    if len(symbols)!=len(contract["symbols"]) or set(symbols)!=set(expected) or any(symbols[k]["shape"]["link_name"]!=v for k,v in expected.items()):
        raise NativeError("NATIVE006", "provided value functions differ from checked IR")
    bootstrap=public_id("$recipe.bootstrap.v1")
    rows=[(bootstrap,"type-read","recipe-bootstrap",READER),(bootstrap,"abi-lowering","bootstrap-v1",READER),(bootstrap,"projection","identity-payload",NATIVE)]
    for function in plan["functions"]:
        subject=public_id(function["name"])
        rows += [(subject,"type-read","rust-declaration",READER),(subject,"abi-lowering","rust-to-c-values",READER)]
        rows += [(subject,"projection",scope,NATIVE) for scope in ("native-call","input-conversion","output-conversion")]
        if function["error"] is not None:rows.append((subject,"projection","domain-error",NATIVE))
        helper="helper:"+prefix+"_"+function["name"]
        rows += [(helper,"projection",scope,"include/"+prefix+".h via "+READER) for scope in ("identity-gate","provider-result-check")]
    output="# Native value mechanism responsibilities\n\nFinite generated source evidence; not runtime certification.\n\n"
    output+=f"- Boundary: `{_escape(contract['boundary_id'])}`\n- Profile: `{_escape(request['selector']['profile_id'])}` (`{_escape(request['selector']['profile_digest'])}`)\n"
    for label,field in (("Source","source_digest"),("Structure","structure_digest"),("Policy","policy_digest")):
        output+=f"- {label} digest: `{_escape(contract[field])}`\n"
    output+=f"- Reader/generator: `{READER}` (`{_escape(component['sha256'])}`)\n- Native SDK: `{NATIVE}`\n"
    output+=f"- SDK metadata: `{_escape(sdk['path'])}` (`{_escape(sdk['sha256'])}`, {sdk['size']} bytes)\n"
    output+="\n## Responsibilities\n\n| Subject | Stage | Scope | Owner |\n| --- | --- | --- | --- |\n"
    for row in sorted(rows,key=lambda r:tuple(v.encode() for v in r[:3])):output+=_row(*row)
    output+="\n## Effective policy inputs\n\n| Symbol | Slot | Values | Origins |\n| --- | --- | --- | --- |\n"
    for policy in sorted(contract["policies"],key=lambda p:(p["symbol_id"].encode(),_slot(p).encode())):
        values="; ".join(k+"="+_value(v) for k,v in sorted(policy["values"].items()));origins=[]
        for item in sorted(policy["provenance"],key=lambda p:p["property"]):
            value=item["property"]+"="+_origin(item["origin"])
            if item.get("replaced_default") is not None:value+=" (replaces "+_origin(item["replaced_default"])+")"
            origins.append(value)
        output+=_row(policy["symbol_id"],_slot(policy),values,"; ".join(origins))
    output+="\n## Frozen recipe inputs\n\n| Path | SHA-256 | Bytes |\n| --- | --- | --- |\n"
    for name,item in sorted(inputs.items()):output+=_row(name,item["sha256"],item["size"])
    output+='''
## Handoffs and limits

- Generated native entries own synchronous input conversion, one author call, result conversion and typed error mapping; no resource, async or cancellation owner is claimed.
- The ordinary helper owns bootstrap identity comparison and provider-result validation only; it does not allocate another Rust value or retry a call.
- Caller memory validity, alignment, non-overlap and absence of data races remain raw-entry preconditions; target execution and independent Lab acceptance require separate evidence.
'''
    return output
