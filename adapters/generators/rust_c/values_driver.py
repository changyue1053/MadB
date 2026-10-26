"""Frozen natural-Rust reader, compiler witnesses and finite C ABI generation."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import shlex
import shutil
import sys
import tempfile

from driver import _compiler_arguments, _finalize
from native_ast import read_header, abi_projection
from support import NativeError, canonical, digest, file_reference, input_path, load_json, run_tool, semantic, verify_tool
from values_codegen import render_values_shim, render_values_cbindgen_config
from values_client import render_values_client
from values_lowering import BASE_CONSTANTS, ZERO, SEMANTICS, enum_constants, interface_identity, lower
from values_ownership import render_ownership


def verify_context(request, config, root):
    allowed = {"schema", "provider", "semantics", "source", "project", "profile", "cbindgen_config", "base_cbindgen_config",
               "tools", "clang_arguments", "environment", "rust_target", "sdk_inputs"}
    if set(config) != allowed or config["schema"] != "madb.rust-c-backend/1" or config["semantics"] != SEMANTICS or config["provider"] != "rust":
        raise NativeError("NATIVE001", "finite values require the exact explicit backend recipe")
    if request.get("schema") not in ("madb.read-request/1", "madb.generate-request/1") or request["selector"]["target"] != config["rust_target"]:
        raise NativeError("NATIVE001", "finite value request or target differs")
    targets = {"aarch64-apple-darwin": "arm64-apple-macosx14.0", "x86_64-apple-darwin": "x86_64-apple-macosx14.0",
               "aarch64-unknown-linux-gnu": "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu": "x86_64-unknown-linux-gnu"}
    if config["rust_target"] not in targets:
        raise NativeError("NATIVE004", "unsupported finite value target")
    _compiler_arguments(config["clang_arguments"], targets[config["rust_target"]])
    declared = set()
    for reference in request["inputs"]:
        path = input_path(root, reference["path"]); data = path.read_bytes()
        if len(data) != reference["size"] or digest(data) != reference["sha256"] or reference["path"] in declared:
            raise NativeError("NATIVE006", "changed or duplicate frozen value input: " + reference["path"])
        declared.add(reference["path"])
    if not {config["source"], config["cbindgen_config"], config["base_cbindgen_config"]} <= declared:
        raise NativeError("NATIVE006", "finite author/config inputs must be frozen")
    if not {"values_reader", "rustc", "clang", "cbindgen", "finalizer"} <= config["tools"].keys():
        raise NativeError("NATIVE001", "finite reader/witness tools missing")
    for tool in config["tools"].values():
        if set(tool) - {"path", "sha256", "size", "id", "version"}:
            raise NativeError("NATIVE001", "value tools cannot have hidden invocation arguments")
        verify_tool(tool)
    for item in config["sdk_inputs"]:
        verify_tool(item)


def read_plan(config, root, scratch, namespace):
    source = input_path(root, config["source"]).read_bytes()
    try:
        text = source.decode("utf-8")
    except UnicodeError as error:
        raise NativeError("NATIVE004", "author module must be UTF-8") from error
    data = run_tool(config, "values_reader", [], stdin=canonical(dict(source_path=config["source"], source=text, namespace=namespace)))
    try:
        plan = json.loads(data)
    except (ValueError, UnicodeError) as error:
        raise NativeError("NATIVE003", "finite Rust reader returned invalid JSON") from error
    if plan.get("namespace") != namespace:
        raise NativeError("NATIVE006", "finite reader returned a different namespace")
    (scratch / "author.rs").write_bytes(source)
    (scratch / "witness.rs").write_text(plan["witness"])
    run_tool(config, "rustc", ["--target", config["rust_target"], "--edition=2024", "--crate-name=madb_value_witness",
        "--crate-type=lib", "--emit=metadata=" + str(scratch / "witness.rmeta") + ",dep-info=" + str(scratch / "witness.d"),
        "--remap-path-prefix", str(scratch) + "=/madb/value-witness", scratch / "witness.rs"], cwd=scratch)
    deptext = (scratch / "witness.d").read_text()
    dependencies = shlex.split(deptext.replace("\\\n", " ").splitlines()[0].partition(":")[2])
    allowed = {(scratch / "witness.rs").resolve(), (scratch / "author.rs").resolve()}
    if not dependencies or {Path(p).resolve() for p in dependencies} != allowed or "# env-dep:" in deptext:
        raise NativeError("NATIVE006", "rustc witness read undeclared author inputs or environment")
    if (scratch / "author.rs").read_bytes() != source:
        raise NativeError("NATIVE006", "witness changed its author input")
    return plan


def generated_headers(config, root, scratch, plan, identity):
    (scratch / "values_shim.rs").write_text(render_values_shim(plan, identity))
    include = scratch / "include"; include.mkdir(exist_ok=True)
    paths = []
    for base, key, name in ((True, "base_cbindgen_config", "madb_value_abi_v1.h"), (False, "cbindgen_config", plan["namespace"] + "_native.h")):
        expected = render_values_cbindgen_config(plan, base_only=base).encode()
        frozen = input_path(root, config[key])
        if frozen.read_bytes() != expected:
            raise NativeError("NATIVE006", "cbindgen recipe differs from the checked author plan")
        header = include / name; depfile = scratch / (name + ".d")
        run_tool(config, "cbindgen", ["--config", frozen, scratch / "values_shim.rs", "--output", header, "--depfile", depfile], cwd=scratch)
        dependencies = shlex.split(depfile.read_text().replace("\\\n", " ").partition(":")[2])
        allowed = {frozen.resolve(), (scratch / "values_shim.rs").resolve(), (scratch / "author.rs").resolve()}
        if not dependencies or any(Path(p).resolve() not in allowed for p in dependencies):
            raise NativeError("NATIVE006", "cbindgen read an undeclared finite source")
        paths.append(header)
    base = read_header(paths[0], config, value_abi=True, constant_names=set(BASE_CONSTANTS))
    native = read_header(paths[1], config, value_abi=True, constant_names=set(enum_constants(plan)),
                         record_sizes={name: v["size"] for name, v in base["layouts"].items()})
    surface = {}
    for key in base:
        if set(base[key]) & set(native[key]):
            raise NativeError("NATIVE005", "duplicate declaration across value headers")
        surface[key] = {**base[key], **native[key]}
    return surface


def header_dependencies(config, scratch, root, *, discovery=False):
    known = {Path(x["path"]).resolve() for x in config["sdk_inputs"]}
    paths = set()
    for header in sorted((scratch / "include").glob("*.h")):
        text = run_tool(config, "clang", [*config["clang_arguments"], "-M", "-x", "c", header]).decode()
        for item in shlex.split(text.replace("\\\n", " ").partition(":")[2]):
            path = Path(item).resolve(strict=True)
            if path.parent == (scratch / "include").resolve():
                continue
            if path.is_relative_to(root.resolve()):
                raise NativeError("NATIVE004", "indirect author header is unsupported")
            if not discovery and path not in known:
                raise NativeError("NATIVE006", "unlocked finite ABI SDK header: " + str(path))
            paths.add(path)
    return paths


def _read(request, config, root, scratch):
    project = load_json(input_path(root, config["project"]))
    plan = read_plan(config, root, scratch, project["sdk_namespace"])
    identity = dict(schema="madb.native-identity/1", bootstrap_version=1, interface_digest=ZERO, profile_digest=ZERO)
    surface = generated_headers(config, root, scratch, plan, identity)
    header_dependencies(config, scratch, root)
    return plan, surface, lower(surface, plan, config, request, project)


def metadata_seed(request, config, input_root, work):
    root, work = Path(input_root), Path(work)
    verify_context(request, config, root)
    with tempfile.TemporaryDirectory(prefix="value-reader-", dir=work) as temporary:
        _, _, seed = _read(request, config, root, Path(temporary))
    verify_context(request, config, root)
    return seed


def metadata(request, config, input_root, work):
    return _finalize(metadata_seed(request, config, input_root, work), config, input_root)


def generate(request, config, input_root, work):
    root, work = Path(input_root), Path(work)
    verify_context(request, config, root)
    with tempfile.TemporaryDirectory(prefix="value-generator-", dir=work) as temporary:
        scratch = Path(temporary)
        plan, surface, seed = _read(request, config, root, scratch)
        checked = _finalize(seed, config, root)
        if checked != load_json(input_path(root, request["contract"]["path"])) or any(checked[a] != request[b] for a,b in (("structure_digest","contract_digest"),("policy_digest","policy_digest"),("source_digest","input_digest"))):
            raise NativeError("NATIVE006", "fresh value metadata differs from the checked contract")
        identity = interface_identity(surface, plan, config, request["selector"]["profile_digest"])
        final_surface = generated_headers(config, root, scratch, plan, identity)
        if abi_projection(final_surface) != abi_projection(surface):
            raise NativeError("NATIVE006", "identity embedding changed the physical value ABI")
        header_dependencies(config, scratch, root)
        destination = scratch / "outputs"; destination.mkdir()
        shutil.copytree(scratch / "include", destination / "include")
        (destination / "include" / (plan["namespace"] + ".h")).write_text(render_values_client(plan, identity))
        for name in ("values_shim.rs", "author.rs"):
            shutil.copyfile(scratch / name, destination / name)
        (destination / "identity.json").write_bytes(canonical(identity) + b"\n")
        (destination / "reader-plan.json").write_bytes(canonical(plan) + b"\n")
        (destination / "responsibilities.md").write_text(render_ownership(checked, request, config, root, plan))
        probe = scratch / "client-syntax.c"
        calls = []
        for index, function in enumerate(plan["functions"]):
            record = plan["namespace"] + "_" + function["record"]
            calls.append(record + " input" + str(index) + " = {0}; (void)" + plan["namespace"] + "_" + function["name"] + "(input" + str(index) + (", 0" if len(function["parameters"]) == 2 else "") + ");")
        probe.write_text('#include "' + plan["namespace"] + '.h"\nint main(void) {\n' + "\n".join(calls) + "\nreturn 0; }\n")
        run_tool(config, "clang", [*config["clang_arguments"], "-I", destination / "include", "-fsyntax-only", probe])
        verify_context(request, config, root)
        sources = sorted(s["public_id"] for s in checked["symbols"])
        files = [file_reference(p, destination, sources) for p in sorted(destination.rglob("*")) if p.is_file()]
        for item in files:
            target = work / item["path"]; target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                raise NativeError("NATIVE001", "value generation output already exists")
            shutil.copyfile(destination / item["path"], target)
    result = dict(schema="madb.output-set/1", input_digest=request["input_digest"], task_digest=request["task_digest"],
                  profile_digest=request["selector"]["profile_digest"], manifest_path="outputs.json", files=files)
    result["generation_id"] = semantic("madb.generation/1", {k:result[k] for k in ("input_digest","task_digest","profile_digest","files")})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("operation", choices=("metadata","generate")); parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    try:
        data = sys.stdin.buffer.read(1024*1024+1)
        if len(data)>1024*1024: raise NativeError("NATIVE001", "value request exceeds 1 MiB")
        request = json.loads(data); config=load_json(args.config); work=Path.cwd(); root=work.parent/"inputs"
        value = metadata(request,config,root,work) if args.operation=="metadata" else generate(request,config,root,work)
        result=canonical(value)+b"\n"; (work/("contract.json" if args.operation=="metadata" else "outputs.json")).write_bytes(result); sys.stdout.buffer.write(result)
    except (NativeError,ValueError,KeyError,OSError) as error:
        sys.stderr.write(str(error)+"\n"); raise SystemExit(2) from error

if __name__ == "__main__": main()
