# C07 adapter boundaries

Run with `.venv/bin/python -B -m unittest discover -s tests/integration/c07_arkts_adapter`.
Tests cover immutable native/notice bindings, ELF/member closure, original Build
log evidence (including reruns and conflicting/old bytes), command timeout
output retention, compiler dependency roots, and public entry/policy gates.
Synthetic archive and policy fixtures are unit evidence, not actual SDK builds.
Actual compiler/C04 checks use `scripts/build_arkts_routes.py` and the public
author fixtures; independent Lab owns assertions and target runs.
