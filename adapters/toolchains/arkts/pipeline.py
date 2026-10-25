"""Thin C07 selection over the existing C04 FrozenPipeline owner."""

from adapters.toolchains.common.pipeline import FrozenPipeline, committed_build_evidence
from pathlib import Path
import json
from adapters.generators.arkts.support import (
    verify_refs,
    copy_new,
    input_path,
    sha,
    clean_environment,
)


class Pipeline(FrozenPipeline):
    driver = "adapters/generators/arkts/driver.py"
    worker = "adapters/toolchains/arkts/worker.py"
    packer = "adapters/artifacts/arkts/pack.py"

    def owner_environment(self):
        return clean_environment()

    def build_phases(self):
        return ("compile", "link")

    def public_generated(self):
        return [
            r
            for r in self.outputs["files"]
            if r["path"] in ("identity.json", "declarations.json")
            or r["path"].startswith("generated/")
        ]

    def public_built(self):
        inventory = json.loads((self.built / "sdk-inventory.json").read_text())
        if inventory.get("schema") != "madb.sdk-inventory/1":
            raise ValueError("missing C07 SDK output inventory")
        files = inventory["files"]
        if self.config["provider"] != "kotlin":
            return files
        receipt = self._kotlin_compile_receipt()
        if files.count(receipt) > 1:
            raise ValueError("duplicate Kotlin compile receipt in SDK inventory")
        return files if receipt in files else files + [receipt]

    def _kotlin_compile_receipt(self):
        """Publish exactly the compile receipt already admitted by this Build."""
        name = "kotlin-compile.json"
        metadata = json.loads((self.built.parent / "metadata.json").read_text())
        evidence_refs = committed_build_evidence(metadata)
        verify_refs(self.built, evidence_refs)
        references = [item for item in metadata["files"] if item["path"] == name]
        if len(references) != 1:
            raise ValueError("committed Build lacks its Kotlin compile receipt")
        committed = {key: references[0][key] for key in ("path", "sha256", "size")}
        outputs = []
        for reference in evidence_refs:
            evidence = json.loads((self.built / reference["path"]).read_text())
            if evidence["phase"] == "compile":
                outputs.extend(
                    {key: item[key] for key in ("path", "sha256", "size")}
                    for item in evidence["outputs"]
                    if item["path"] == name
                )
        if outputs != [committed]:
            raise ValueError("Kotlin compile receipt differs from accepted Compile output")
        verify_refs(self.built, [committed])
        return name

    def common_inputs(self, metadata=False):
        return super().common_inputs(metadata) + getattr(self, "_package_logs", [])

    def package(self, *args, **kwargs):
        # Logs are accepted BuildEvidence references but are not committed by
        # core assemble_build_files. Admit their original bytes as extra Package
        # inputs without mutating the immutable Build or rewriting evidence.
        metadata = json.loads((self.built.parent / "metadata.json").read_text())
        refs = committed_build_evidence(metadata)
        verify_refs(self.built, refs)
        logs = {}
        for ref in refs:
            evidence = json.loads((self.built / ref["path"]).read_text())
            for log in evidence["logs"]:
                if log["path"] in logs and logs[log["path"]] != log:
                    raise ValueError("conflicting Build log paths")
                accepted = self._original_log(log)
                destination = self.root / log["path"]
                if destination.exists() or destination.is_symlink():
                    # A second Package may reuse exactly the same accepted log
                    # bytes. Existing different bytes are never overwritten.
                    verify_refs(self.root, [log])
                else:
                    copy_new(accepted, destination)
                verify_refs(self.root, [log])
                logs[log["path"]] = dict(log)
        self._package_logs = list(logs.values())
        return super().package(*args, **kwargs)

    def _original_log(self, reference):
        """Find bytes admitted by this immutable Build, not merely the latest run."""
        matches = sorted((self.root / "runs").glob("*/node-*/work/" + reference["path"]))
        for path in matches:
            try:
                checked = input_path(self.root, path.relative_to(self.root).as_posix())
            except (OSError, ValueError):
                continue
            if checked.stat().st_size == reference["size"] and sha(checked) == reference["sha256"]:
                return checked
        raise ValueError("original accepted Build log is unavailable: " + reference["path"])
