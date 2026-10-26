"""Finite values use the same frozen Source/Build/Candidate coordinator."""
from ..common.pipeline import FrozenPipeline

class Pipeline(FrozenPipeline):
    driver = "adapters/generators/rust_c/values_driver.py"
    worker = "adapters/toolchains/rust_c/values_worker.py"
    packer = "adapters/artifacts/rust_c/pack.py"

    def public_generated(self):
        return [item for item in self.outputs["files"] if item["path"].startswith("include/") or item["path"] in ("identity.json", "responsibilities.md")]

    def public_built(self):
        namespace = self.contract["sdk_namespace"]
        return ["lib/lib"+namespace+(".dylib" if "apple" in self.config["rust_target"] else ".so")]
