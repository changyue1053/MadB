"""Install the source-locked Rust/C native generators; usage: --root NEW_DIRECTORY."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from adapters.toolchains.rust_c.install_tools import main


if __name__ == "__main__":
    main()
