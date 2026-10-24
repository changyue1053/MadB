"""Compatibility entry point for the shared source-locked native tool installer."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from adapters.toolchains.rust_c.install_tools import main


if __name__ == "__main__":
    main()
