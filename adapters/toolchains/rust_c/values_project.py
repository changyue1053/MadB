"""Prepare the approved finite natural-Rust value recipe without inventing author structure."""
from copy import deepcopy
import json
from pathlib import Path
import re
import shutil
import sys
import tempfile

from .common import profile_digest, project_digest, ref, save, semantic, sha_file, unique
from .project import _public_observation

ZERO = "sha256:" + "0" * 64
COMPONENT = "org.madb.native-values/reader-generator"


def copy_sources(repo, root, author):
    names=[]
    directories=("adapters/generators/rust_c", "adapters/toolchains/rust_c", "adapters/toolchains/common", "adapters/artifacts/rust_c")
    for relative in directories:
        for source in sorted((repo/relative).rglob("*")):
            if source.is_file() and source.suffix in (".py", ".toml", ".md", ".c", ".h"):
                name=source.relative_to(repo).as_posix();target=root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);names.append(name)
    # These bytes explain the fixed reader image; they are tool source inputs,
    # never compiled or imported by a public SDK consumer.
    for name in ("adapters/contracts/src/native_values.rs", "crates/madb-cli/src/bin/madb-native-values-read.rs", "adapters/contracts/Cargo.toml", "Cargo.lock", "LICENSE", "NOTICE"):
        source=repo/name
        if source.is_file():
            target=root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);names.append(name)
    nested=repo/"adapters/contracts/src/native_values"
    if nested.exists():
        for source in sorted(nested.rglob("*.rs")):
            name=source.relative_to(repo).as_posix();target=root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);names.append(name)
    data=Path(author).read_bytes();(root/"author.rs").write_bytes(data);names.append("author.rs")
    schemas=[]
    for source in sorted((repo/"docs/schemas").glob("*.schema.json")):
        name=source.relative_to(repo).as_posix();target=root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);schemas.append(name)
    return sorted(set(names)), schemas


def policies(seed):
    result=[]
    for symbol in seed["symbols"]:
        for slot, values in ((dict(kind="call"), dict(threading="caller",blocking="unbounded",delivery="work-completed",cancellation="none",cancellation_confirmation="not-supported")),
                             (dict(kind="return"),dict(ownership="copy"))):
            result.append(dict(selector=dict(kind="public_id",id=symbol["public_id"]),slot=slot,families=[],values=values,
                origin=dict(authority="project-policy",id="native-values."+slot["kind"],location=dict(path="project.json",line=1,column=1))))
    return result


def prepare(repo, workspace, observation, author, namespace, variant, reader_image):
    repo,root=Path(repo).resolve(),Path(workspace).resolve()
    if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]{0,63}",variant):raise ValueError("invalid finite value variant name")
    observation=deepcopy(observation);reader=Path(reader_image).resolve(strict=True)
    observation["tools"]["values_reader"]=dict(path=str(reader),id="madb-native-values-read",version="0.1.0",sha256=sha_file(reader),size=reader.stat().st_size)
    root.mkdir(parents=True,exist_ok=False);names,schema_names=copy_sources(repo,root,author)
    config=dict(schema="madb.rust-c-backend/1",provider="rust",semantics="madb.native-values/1",source="author.rs",project="project.json",profile="profile.json",
        cbindgen_config="values-cbindgen.toml",base_cbindgen_config="values-base-cbindgen.toml",tools=deepcopy(observation["tools"]),
        clang_arguments=list(observation["clang_arguments"]),environment=dict(observation["environment"]),rust_target=observation["target"],sdk_inputs=deepcopy(observation["sdk_inputs"]))
    sys.path.insert(0,str(root/"adapters/generators/rust_c"))
    try:
        import values_driver as native
        from values_codegen import render_values_cbindgen_config
        from values_lowering import lower
        with tempfile.TemporaryDirectory(prefix="madb-value-discovery-") as temporary:
            scratch=Path(temporary);plan=native.read_plan(config,root,scratch,namespace)
            for base,key in ((True,"base_cbindgen_config"),(False,"cbindgen_config")):
                (root/config[key]).write_text(render_values_cbindgen_config(plan,base_only=base));names.append(config[key])
            zero=dict(schema="madb.native-identity/1",bootstrap_version=1,interface_digest=ZERO,profile_digest=ZERO)
            surface=native.generated_headers(config,root,scratch,plan,zero)
            deps=native.header_dependencies(config,scratch,root,discovery=True)
            known={Path(x["path"]).resolve():x for x in observation["sdk_inputs"]}
            for path in deps:known[path]=dict(path=str(path),sha256=sha_file(path),size=path.stat().st_size)
            observation["sdk_inputs"]=[known[p] for p in sorted(known)];config["sdk_inputs"]=deepcopy(observation["sdk_inputs"])
        save(root,"toolchain.json",_public_observation(observation));save(root,"backend.json",config);names += ["backend.json","toolchain.json"]
        features=["core.values@1","core.exact-numbers@1"]+(["core.errors@1","core.variants@1"] if plan["errors"] else [])
        target=observation["target"]
        profile=dict(schema="madb.profile/1",id="l02-native-values-"+variant+"-"+target,route="RUST-C-ABI",provider=dict(language="rust",runtime="native"),
            consumer=dict(language="c",runtime="native"),direction="consumer-to-provider",target=target,abi="c11",artifact_kind="dynamic-library",build_system="rustc-clang",
            deployment_target="14.0" if "apple" in target else "native-host",
            tools=[dict(id=t["id"],version=t["version"],sha256=t["sha256"],host=target,target=target,source="system://observed-native-toolchain/"+name,arguments=[])
                   for name,t in sorted(observation["tools"].items())],sdk=ref(root,"toolchain.json","madb.native-toolchain/1"),macros=[],
            required_features=sorted(features),supported_features=sorted(features),restrictions=[],
            defaults=dict(values=dict(ownership="copy"),origin=dict(authority="profile-default",id="native-values.copy",location=dict(path="profile.json",line=1,column=1))),backend_config=None)
        save(root,"profile.json",profile)
        selector=dict(route="RUST-C-ABI",profile_id=profile["id"],profile_digest=profile_digest(profile),target=target,direction="consumer-to-provider")
        project=dict(schema="madb.project/1",project="madb-native-values",sdk_namespace=plan["namespace"],
            boundaries=[dict(id=variant,route="RUST-C-ABI",direction="consumer-to-provider",source=dict(kind="rust-c-declaration",path="author.rs"),
                profile=ref(root,"profile.json","madb.profile/1"),target=target,output="generated")],policies=[],policy_families=[],artifacts=dict(package="madb-native-values-"+variant,manifest="candidate.json"))
        inputs=unique(ref(root,n,"madb.source-input/1") for n in names)
        preliminary=dict(schema="madb.read-request/1",boundary_id=variant,selector=selector,inputs=inputs)
        seed=lower(surface,plan,config,preliminary,project);project["policies"]=policies(seed);save(root,"project.json",project)
    finally:sys.path.pop(0)
    component_files=[r for r in inputs if r["path"].startswith("adapters/generators/rust_c/") or r["path"].startswith("adapters/contracts/src/native_values") or r["path"]=="crates/madb-cli/src/bin/madb-native-values-read.rs"]
    component=dict(id=COMPONENT,version="0.1.0",sha256=semantic("madb.native-component/1",component_files))
    lock=dict(schema="madb.lock/1",project_digest=project_digest(project),profiles=[dict(id=profile["id"],semantic_digest=profile_digest(profile),file=ref(root,"profile.json","madb.profile/1"))],
        tools=profile["tools"],components=[component],inputs=[{k:r[k] for k in ("path","sha256","size")} for r in inputs],schemas=[ref(root,n) for n in schema_names])
    save(root,"madb.lock",lock)
    read=dict(schema="madb.read-request/1",boundary_id=variant,selector=selector,inputs=inputs,component=component,lock=ref(root,"madb.lock","madb.lock/1"),
              profile=ref(root,"profile.json","madb.profile/1"),output_root="staging/read")
    save(root,"read.json",read)
    return dict(profile=profile,project=project,read=read,lock=lock,config=config)
