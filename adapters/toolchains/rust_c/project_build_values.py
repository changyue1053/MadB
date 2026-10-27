#!/usr/bin/env python3
"""Finite project rebuild document preparation; execution remains inside C04.

The preserved initializer is reused without changing its frozen runtime list.
This extra adapter is itself an input of every project read/lock snapshot.
"""
import sys
from pathlib import Path

# Only the immutable runtime directory materialized by Host is imported.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import init_values

init_values.RUNTIME_FILES = (*init_values.RUNTIME_FILES,
    "adapters/toolchains/rust_c/project_build_values.py")

if __name__ == "__main__":
    raise SystemExit(init_values.main())
