"""C06 managed-language recipes over the same C04 frozen ports as C05."""
from ..common.pipeline import FrozenPipeline


class Pipeline(FrozenPipeline):
    driver = "adapters/generators/kotlin_rust/driver.py"
    worker = "adapters/toolchains/kotlin_rust/worker.py"
    packer = "adapters/artifacts/rust_c/pack.py"

    def public_generated(self):
        return [item for item in self.outputs["files"]
                if not item["path"].startswith("internal/")]

    def public_built(self):
        import json
        manifest = json.loads((self.built / "sdk-inventory.json").read_text())
        if manifest.get("schema") != "madb.sdk-inventory/1":
            raise ValueError("missing managed SDK output inventory")
        return manifest["files"]
