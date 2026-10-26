"""Explicit real reader -> captured context -> public explain CLI integration.

Requires a supplied pinned tool observation and actual madb binary. It does not
build an SDK or claim independent Lab/runtime acceptance. No tools are skipped.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import declaration_diagnostics as diagnostics
from adapters.generators.arkts import driver, support


def ref(data, path):
    return {"path": str(path), "sha256": "sha256:" + hashlib.sha256(data).hexdigest(), "size": len(data)}


def inventory(directory):
    result = {}
    for path in directory.rglob("*"):
        name = str(path.relative_to(directory))
        if path.is_symlink():
            result[name] = {"kind": "symlink", "target": str(path.readlink())}
        elif path.is_file():
            result[name] = ref(path.read_bytes(), name)
        elif path.is_dir():
            result[name] = {"kind": "directory"}
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tool-observation", required=True, type=Path)
    parser.add_argument("--madb", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        parser.error("report must be new")
    observation = args.tool_observation.read_bytes()
    tools = json.loads(observation)["config"]["tools"]
    config = {"tools": {name: tools[name] for name in ("node", "typescript")}}
    for tool in config["tools"].values():
        support.verify_tool(tool)
    cli = args.madb.resolve(strict=True)
    cli_ref = ref(cli.read_bytes(), cli)
    tracked = [
        "tests/integration/l02_diagnostics/check_explain_cli.py",
        "adapters/generators/diagnostic-explanations.json",
        "adapters/generators/arkts/declaration_diagnostics.py",
        "adapters/generators/arkts/read_facade.cjs", "adapters/generators/arkts/support.py",
        "adapters/generators/arkts/driver.py", "adapters/toolchains/arkts/project.py",
        "crates/madb-app/src/explain.rs", "adapters/contracts/src/explain.rs",
        "docs/schemas/diagnostic-context.schema.json", "docs/schemas/explain-result.schema.json",
        "tests/fixtures/c07_arkts_generation/reject-callback-result/Boundary.ets.in",
    ]
    sources = [ref((ROOT / name).read_bytes(), name) for name in tracked]
    commands, observations = [], []
    report = {"scope": "Real fixed reader failures and captured source contexts consumed by public explain CLI; no generated SDK, runtime, independent Lab or full L02 acceptance", "tool_observation": ref(observation, args.tool_observation), "tools": config["tools"], "cli": cli_ref, "sources": sources, "commands": commands, "observations": observations, "successful": False}
    try:
        with tempfile.TemporaryDirectory(prefix="madb-explain-integration-") as temporary:
            work = Path(temporary)
            version = subprocess.run([str(cli), "--version"], capture_output=True, text=True, timeout=10)
            assert version.returncode == 0, version.stderr
            report["cli_version"] = version.stdout.strip()
            reader_versions = []
            node_version = support.command(config, "node", ["--version"], cwd=work, capture=reader_versions).decode().strip()
            compiler_version = support.command(config, "node", ["-e", "process.stdout.write(require(process.argv[1]).version)", tools["typescript"]["path"]], cwd=work, capture=reader_versions).decode().strip()
            assert node_version == "v" + tools["node"]["version"]
            report["reader_versions"] = {"node": node_version, "compiler_api": compiler_version, "commands": reader_versions}

            def query(argv, expected):
                before = inventory(work)
                run = subprocess.run([str(cli), *argv], cwd=work, capture_output=True, text=True, timeout=10)
                commands.append({"argv": [str(cli), *argv], "exit_code": run.returncode, "stdout": run.stdout, "stderr": run.stderr})
                assert inventory(work) == before, "query changed its workspace"
                assert run.returncode == expected, commands[-1]
                if "--json" in argv:
                    assert run.stderr == "", run.stderr
                    result = json.loads(run.stdout)
                    assert run.stdout.endswith("\n") and result["exit_code"] == expected
                    return result
                return run.stdout

            callback = driver.render_template((ROOT / tracked[-1]).read_bytes(), "libexplain.so", "l02-explain")
            samples = [("modifiers", "\ufeff/* 🍎 */ export async function changed(): Promise<void> {}\n".encode(), "ARKTS_DECL_UNSUPPORTED_MODIFIERS"),
                       ("callback", callback, "ARKTS_DECL_CALLBACK_RESULT")]
            for label, data, code in samples:
                logical = f"author/{label}.ets"
                captured = []
                try:
                    support.command(config, "node", [ROOT / "adapters/generators/arkts/read_facade.cjs", tools["typescript"]["path"], logical, "source"], cwd=work, stdin=data, capture=captured)
                except RuntimeError:
                    assert captured[-1]["exit_code"] == 1 and captured[-1]["stdout"] == ""
                else:
                    raise AssertionError("negative declaration was accepted")
                source = ref(data, logical)
                bound = diagnostics.explain_reader_failure(captured[-1]["stderr"], source, data)
                assert bound.explanation.code == code
                location = {"path": logical, "line": bound.source.line, "column": bound.source.column}
                context = {"schema": "madb.diagnostic-context/1", "diagnostic": {"code": code, "message": bound.explanation.message, "location": location}, "source": source, "position_encoding": "utf16-js", "reported_precision": "compiler-derived"}
                source_path = work / logical
                source_path.parent.mkdir(exist_ok=True)
                source_path.write_bytes(data)
                context_path = work / f"{label}.json"
                context_raw = (json.dumps(context, ensure_ascii=False, indent=2) + "\n").encode()
                context_path.write_bytes(context_raw)
                result = query(["explain", code, "--context", context_path.name, "--json"], 0)
                assert result["source"]["reference"] == source
                assert result["source"]["byte_offset"] == bound.source.byte_offset
                assert result["source"]["location"] == location
                assert result["context"] == {**ref(context_raw, context_path.name), "schema": "madb.diagnostic-context/1"}
                assert result["explanation"]["meaning"] == bound.explanation.meaning
                observations.append({"label": label, "source": source, "source_utf8": data.decode(), "reader_commands": captured, "context": context, "result": result})
                source_path.write_bytes(data + b" ")
                wrong = query(["explain", code, "--context", context_path.name, "--json"], 2)
                assert wrong["explanation"] is None and wrong["source"] is None
                source_path.write_bytes(data)
                other = "ARKTS_DECL_CALLBACK_RESULT" if code.endswith("MODIFIERS") else "ARKTS_DECL_UNSUPPORTED_MODIFIERS"
                wrong = query(["explain", other, "--context", context_path.name, "--json"], 2)
                assert wrong["diagnostics"][0]["code"] == "EXPLAIN002"
                text = query(["explain", code, "--context", context_path.name], 0)
                assert bound.explanation.meaning in text
            unknown = query(["explain", "ARKTS_DECL_FUTURE", "--context", "absent.json", "--json"], 2)
            assert unknown["diagnostics"][0]["code"] == "EXPLAIN001"
            missing = query(["explain", "SEM007", "--context", "absent.json", "--json"], 3)
            assert missing["diagnostics"][0]["code"] == "EXPLAIN003"
        for tool in config["tools"].values():
            support.verify_tool(tool)
        assert sources == [ref((ROOT / name).read_bytes(), name) for name in tracked]
        assert cli_ref == ref(cli.read_bytes(), cli)
        assert args.tool_observation.read_bytes() == observation
        report["successful"] = True
    except Exception as error:
        report["failure"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        with args.report.open("x") as output:
            json.dump(report, output, ensure_ascii=False, indent=2)
            output.write("\n")


if __name__ == "__main__":
    main()
