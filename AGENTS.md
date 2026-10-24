# MadB Agent entry

Read [the common method](docs/engineering/07-agent-development.md), then the selected task in [the catalog](docs/catalog/implementation-plan.json), its `documents`, acceptance and applicable decisions. The task catalog is the only progress authority; HTML and historical notes are references.

Before edits, record task ID/sub-scope, specification revision, paths/non-goals and real checks in the current task or PR. Work autonomously within that scope. Preserve unrelated work. Architecture, API/ABI, ownership, thread or cancellation changes need an explicit documented decision; do not silently change a route.

Follow [module boundaries](docs/architecture/02-modules-and-ports.md), [clean code](docs/engineering/02-clean-code.md) and [quality gates](docs/engineering/05-review-and-quality-gates.md). Core crates do not import concrete backends. Wire schemas are not domain models. Edit templates/catalogs and regenerate HTML; do not patch generated output alone.

From this repository root, use Python/Node versions in the version files and the project's Rust toolchain. Install the README's locked development dependencies, then run `.venv/bin/python scripts/check.py`. For focused document changes use `.venv/bin/python docs/tools/check_docs.py` and the affected documentation tests; generate views with `.venv/bin/python docs/tools/render_visuals.py`.

Report actual changes, commands/results and unresolved acceptance. Do not weaken checks, invent evidence, update expectations merely to remove failures, or count partial C01 work as completion. Missing tools/targets and unexecuted remote CI remain missing; local tests do not prove branch protection or platform certification. Publish only within explicit task authorization and after applicable publication checks.
