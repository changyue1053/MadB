"""C05 Rust/C recipe over the shared frozen execution coordinator."""
from ..common.pipeline import FrozenPipeline


class Pipeline(FrozenPipeline):
    driver = "adapters/generators/rust_c/driver.py"
    worker = "adapters/toolchains/rust_c/worker.py"
    packer = "adapters/artifacts/rust_c/pack.py"

    def public_generated(self):
        return [item for item in self.outputs["files"] if item["path"] not in ("identity.rs", "identity.h")]

    def public_built(self):
        if self.config["provider"] == "c":
            return ["lib/libmadb_c.a"]
        extension = "dylib" if "apple" in self.config["rust_target"] else "so"
        return ["lib/libmadb_engine." + extension]
