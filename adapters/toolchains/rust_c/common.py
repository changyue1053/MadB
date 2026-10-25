"""Compatibility import for C05 helpers; public document math is shared."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from documents import (canonical, semantic, sha_file, save, ref, profile_digest,
                       project_digest, task_digest, generation_task, unique)
