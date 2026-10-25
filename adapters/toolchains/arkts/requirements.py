"""Select the reviewed independent ordinary/test requirements without fallback."""

from pathlib import Path
from adapters.generators.arkts.support import read_json, sha

PROFILE = "c07-c-arkts-aarch64-ohos23"
ROUTE = "madb.arkts-c-napi"


def validate(value, requirements, profile_id=PROFILE, route=ROUTE):
    if value.get("schema") != "madb.lab-c07-candidate-case-allocation/1":
        raise ValueError("unsupported independent case allocation schema")
    profiles = {p["profile_id"]: p for p in value["profiles"]}
    cases = {c["id"]: c for c in value["cases"]}
    if len(profiles) != len(value["profiles"]) or len(cases) != len(value["cases"]):
        raise ValueError("duplicate case/profile allocation")
    ordinary = profiles[profile_id]
    companion = profiles[profile_id + "-test"]
    for p, kind, other in [
        (ordinary, "ordinary", companion),
        (companion, "test-companion", ordinary),
    ]:
        ids = p["required_cases"]
        if p["route"] != route or p["kind"] != kind or p["relation_profile"] != other["profile_id"]:
            raise ValueError("case allocation profile/route/kind/pair differs")
        owned = {c["id"] for c in cases.values() if c["primary_profile_id"] == p["profile_id"]}
        if not ids or len(ids) != len(set(ids)) or set(ids) != owned:
            raise ValueError("case allocation differs from independent per-case ownership")
    left = set(ordinary["required_cases"])
    right = set(companion["required_cases"])
    if left & right or left | right != set(requirements["required_cases_by_route"][route]):
        raise ValueError("ordinary/test pair omits or duplicates route requirements")
    if len(left) != 5 or len(right) != 25:
        raise ValueError("reviewed C07 allocation must be five ordinary plus twenty-five companion")
    return ordinary


def load(path, expected_sha256, requirements_source=None, profile_id=PROFILE, route=ROUTE):
    path = Path(path)
    value = read_json(path)
    if value.get("schema") != "madb.lab-c07-candidate-case-allocation/1":
        raise ValueError("unsupported independent case allocation schema")
    if sha(path) != expected_sha256:
        raise ValueError("case allocation differs from reviewed original pin")
    source = value["source_requirements"]
    source_path = Path(requirements_source or source["path"])
    if sha(source_path) != "sha256:" + source["sha256"]:
        raise ValueError("independent route requirements changed")
    requirements = read_json(source_path)
    return value, validate(value, requirements, profile_id, route)
