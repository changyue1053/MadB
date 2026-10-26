"""Actual fixed SDK AST/projection checks. Outputs remain outside the repository."""
from dataclasses import asdict
from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import composition_recipe as recipe


def run(args):
    if args.output.exists() or args.output.is_symlink():
        raise ValueError("test output must be new")
    args.output.mkdir(parents=True)
    output = args.output
    cases, observations = [], {}
    settings = dict(node=args.node, node_sha256=args.node_sha256, typescript=args.typescript,
                    typescript_sha256=args.typescript_sha256)

    def rejected(name, action, reason):
        try:
            action()
        except (ValueError, TypeError) as error:
            if reason not in str(error):
                raise AssertionError(f"{name}: wrong rejection: {error}") from error
            cases.append({"case": name, "result": "rejected", "reason": str(error)})
        else:
            raise AssertionError(name + " was accepted")

    for mode, count, files in (("ordinary", 5, 2), ("enhanced", 8, 4), ("disabled", 5, 2)):
        observation = recipe.observe_recipe(ROOT, output / mode, mode=mode, **settings)
        assert len(observation.required) == count
        assert len(observation.outputs) == files
        assert all(role.subject_kind in ("public-symbol", "sdk-export") for role in observation.required)
        observations[mode] = observation
        cases.append({"case": mode, "result": "observed", "required_roles": count, "files": files})
    ordinary, enhanced, disabled = (observations[mode] for mode in ("ordinary", "enhanced", "disabled"))
    assert ordinary == disabled
    assert enhanced.required[:5] == ordinary.required
    assert {Path(r.path).name for r in enhanced.outputs} - {Path(r.path).name for r in ordinary.outputs} == {
        "StateObservation.ets", "ArkUIStateModel.ets"}
    roundtrip = [r for r in ordinary.required if r.subject == "madb.c07.Engine.roundtrip.v1"]
    assert len(roundtrip) == 1 and roundtrip[0].stage == "projection"
    drains = [r for r in enhanced.required if r.stage == "drain"]
    assert len(drains) == 1 and drains[0].source.path == recipe.NATIVE
    releases = [r for r in enhanced.required if r.stage == "release"]
    assert len(releases) == 1 and releases[0].source.path == recipe.FACADE
    assert all("native-gateway" not in r.source.path for r in enhanced.required)
    cases.append({"case": "independent-roles-and-disabled-equality", "result": "matched"})

    # Independent compiler AST consumer verifies actual local module closure.
    inspector = r"""
const fs=require('node:fs'),path=require('node:path'),ts=require(process.argv[1]);
const root=process.argv[2], names=JSON.parse(process.argv[3]), external=new Set();
for(const name of names){
 const file=path.join(root,name), sf=ts.createSourceFile(name,fs.readFileSync(file,'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
 if(sf.parseDiagnostics.length)throw Error('generated syntax');
 for(const n of sf.statements)if(ts.isImportDeclaration(n)){
  const ref=n.moduleSpecifier.text;
  if(ref.startsWith('.')){
   const target=path.relative(root,path.resolve(path.dirname(file),ref+'.ets')).split(path.sep).join('/');
   if(!names.includes(target))throw Error('undeclared local import '+target);
  }else external.add(ref);
 }
}
process.stdout.write(JSON.stringify([...external].sort()));
"""
    for mode, observation in observations.items():
        result = subprocess.run([str(args.node), "-e", inspector, str(args.typescript), str(output / mode),
                                 json.dumps([r.path for r in observation.outputs])], capture_output=True, check=True, timeout=20)
        assert json.loads(result.stdout) == list(observation.external_imports)
    cases.append({"case": "actual-generated-import-closure-three-configurations", "result": "matched"})
    rejected("unknown-recipe", lambda: recipe.observe_recipe(ROOT, output / "unknown", mode="all", **settings),
             "unknown finite")
    bad_tool = dict(settings, node_sha256="0" * 64)
    rejected("wrong-node-bytes", lambda: recipe.observe_recipe(ROOT, output / "tool", mode="ordinary", **bad_tool),
             "unsupported Node image")
    bad_sdk = dict(settings, typescript_sha256="0" * 64)
    rejected("unknown-sdk", lambda: recipe.observe_recipe(ROOT, output / "sdk", mode="ordinary", **bad_sdk),
             "unsupported SDK compiler")
    rejected("submitted-required-not-an-input", lambda: recipe.observe_recipe(
        ROOT, output / "claims", mode="ordinary", required=[], **settings), "unexpected keyword")
    rejected("existing-output-domain", lambda: recipe.observe_recipe(ROOT, output / "ordinary", mode="ordinary", **settings),
             "output domain must be new")

    source = output / "source"
    for name in recipe.SOURCE_PINS:
        target = source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT / name).read_bytes())
    (output / "spacer").mkdir()
    escaped = output / "spacer/../source/generated-domain"
    rejected("parent-component-output-escape", lambda: recipe.observe_recipe(source, escaped, mode="ordinary", **settings),
             "invalid domain path")
    assert not (source / "generated-domain").exists()
    rejected("direct-source-output-overlap", lambda: recipe.observe_recipe(source, source / "generated-domain", mode="ordinary", **settings),
             "domains must be separate")
    ancestor = output / "ancestor-alias"
    ancestor.symlink_to(output, target_is_directory=True)
    rejected("source-ancestor-alias", lambda: recipe.observe_recipe(ancestor / "source", output / "alias-source-run", mode="ordinary", **settings),
             "symbolic domain path or ancestor")
    rejected("output-ancestor-alias", lambda: recipe.observe_recipe(source, ancestor / "alias-output-run", mode="ordinary", **settings),
             "symbolic domain path or ancestor")
    for name, edit, label in (
        (recipe.FACADE, lambda s: s.replace("observeState(listener:", "observeHidden(listener:"), "changed-real-declaration"),
        (recipe.STATE, lambda s: s.replace("engine.observeState(", "engine.observeEvents("), "changed-sdk-wiring"),
        (recipe.UI, lambda s: s.replace("@Trace", ""), "changed-sdk-decorator"),
    ):
        original = (source / name).read_text()
        changed = edit(original)
        assert changed != original
        (source / name).write_text(changed)
        rejected(label, lambda: recipe.observe_recipe(source, output / label, mode="enhanced", **settings),
                 "unsupported fixed SDK source bytes")
        (source / name).write_text(original)
    original = (source / recipe.FRAME).read_bytes()
    (source / recipe.FRAME).unlink()
    (source / recipe.FRAME).symlink_to(ROOT / recipe.FRAME)
    rejected("symlinked-source", lambda: recipe.observe_recipe(source, output / "symlink-source", mode="ordinary", **settings),
             "symbolic source")
    (source / recipe.FRAME).unlink()
    (source / recipe.FRAME).write_bytes(original)
    source_observation = recipe.observe_recipe(source, output / "source-bound", mode="ordinary", **settings)
    rejected("verify-source-ancestor-alias", lambda: recipe.verify_observation(source_observation, ancestor / "source", output / "source-bound"),
             "symbolic domain path or ancestor")
    rejected("verify-output-ancestor-alias", lambda: recipe.verify_observation(source_observation, source, ancestor / "source-bound"),
             "symbolic domain path or ancestor")
    rejected("verify-parent-component-alias", lambda: recipe.verify_observation(source_observation, source, output / "spacer/../source-bound"),
             "invalid domain path")
    ancestor.unlink()
    policy_bytes = (source / recipe.POLICY).read_bytes()
    (source / recipe.POLICY).write_bytes(policy_bytes + b"\n")
    rejected("late-source-change", lambda: recipe.verify_observation(source_observation, source, output / "source-bound"),
             "source changed after observation")
    (source / recipe.POLICY).write_bytes(policy_bytes)

    # Exercise the new SDK parser independently of the exact-byte recipe gate.
    for name, mode, changed, diagnostic in (
        (recipe.STATE, "state", (ROOT / recipe.STATE).read_text().replace("close(): Promise<void>", "close(): void"),
         "SDK method shape differs"),
        (recipe.UI, "ui", (ROOT / recipe.UI).read_text().replace("@Trace", ""), "SDK observed snapshot differs"),
    ):
        result = subprocess.run([str(args.node), str(recipe.BASE / "read_observation.cjs"), str(args.typescript), name, mode],
                                input=changed.encode(), capture_output=True, timeout=20)
        assert result.returncode != 0 and diagnostic in result.stderr.decode()
        cases.append({"case": "actual-sdk-ast-" + mode, "result": "rejected", "reason": result.stderr.decode().strip()})

    unexpected = output / "disabled/generated/facade/ArkUIStateModel.ets"
    unexpected.write_bytes((ROOT / recipe.UI).read_bytes())
    rejected("disabled-hidden-ui", lambda: recipe.verify_observation(disabled, ROOT, output / "disabled"), "undeclared output")
    unexpected.unlink()
    frame = output / "ordinary/generated/facade/RuntimeFrame.ets"
    original = frame.read_bytes()
    frame.unlink()
    rejected("missing-real-generated-dependency", lambda: recipe.verify_observation(ordinary, ROOT, output / "ordinary"),
             "missing/nonregular")
    frame.write_bytes(original + b"\n")
    rejected("late-output-change", lambda: recipe.verify_observation(ordinary, ROOT, output / "ordinary"), "output changed")
    frame.unlink()
    frame.symlink_to(ROOT / recipe.FRAME)
    rejected("symlinked-output", lambda: recipe.verify_observation(ordinary, ROOT, output / "ordinary"), "symbolic output")
    frame.unlink()
    frame.write_bytes(original)
    output_alias = output / "output-alias"
    output_alias.symlink_to(output / "ordinary", target_is_directory=True)
    rejected("aliased-output-root", lambda: recipe.verify_observation(ordinary, ROOT, output_alias), "symbolic domain path or ancestor")
    output_alias.unlink()
    for mode, observation in observations.items():
        recipe.verify_observation(observation, ROOT, output / mode)
    receipt = {"scope": "actual source, fixed SDK AST, production declaration projection and frozen output observations",
               "cases": cases, "observations": {k: asdict(v) for k, v in observations.items()},
               "test_inputs": [{"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in
                               (Path(__file__), Path(__file__).with_name("README.md"))],
               "not_claimed": ["registered package", "E02 session", "profile/2 OutputSet or finalizer", "C04 commit",
                               "SDK compilation", "Native/UI execution", "installed candidate acceptance"]}
    (output / "observations.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"checks": len(cases), "ordinary_roles": 5, "enhanced_roles": 8,
                      "disabled_equals_ordinary": ordinary == disabled}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--node", type=Path, required=True)
    parser.add_argument("--node-sha256", required=True)
    parser.add_argument("--typescript", type=Path, required=True)
    parser.add_argument("--typescript-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    run(parser.parse_args())
