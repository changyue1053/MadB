"""Install or verify the fixed C06 managed compilers and dependency files."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from adapters.toolchains.kotlin_rust.install_tools import main


if __name__ == "__main__":
    main()
