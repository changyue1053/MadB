"""SKIE recipe's explicit workspace choice over the frozen common compiler runner."""
from __future__ import annotations
import hashlib
import importlib.util
from pathlib import Path
import sys

# Load from this frozen input tree, not an ambient installed adapters package.
_path = Path(__file__).resolve().parents[2] / "toolchains/common/compiler_workspace.py"
_name = "_madb_compiler_workspace_" + hashlib.sha256(str(_path).encode()).hexdigest()[:16]
if _name not in sys.modules:
    _spec = importlib.util.spec_from_file_location(_name, _path)
    _module = importlib.util.module_from_spec(_spec)
    sys.modules[_name] = _module
    _spec.loader.exec_module(_module)
_common = sys.modules[_name]
BASE = Path("/private/tmp/madb-compiler/c06-skie")
identity = _common.identity
execute = _common.execute


def acquire(key: str, timeout: float = 90, *, base: Path | None = None):
    return _common.acquire(key, timeout, base=BASE if base is None else base)
