# Contributing

Select an existing work package from [the catalog](docs/catalog/implementation-plan.json). In the task or PR state its sub-scope, specification revision, allowed paths and validation. Apply [clean code](docs/engineering/02-clean-code.md) and [the Agent method](docs/engineering/07-agent-development.md) to both human and generated changes.

Set up the exact environment described in README, run `.venv/bin/python scripts/check.py`, and report any target you could not execute. Public format or semantic changes update the authoritative specification and applicable ADR before dependent implementation. Do not treat generated output as its own oracle.

C01's CLI help/version is a real small task for exercising this process. The architecture tests deliberately introduce forbidden dependencies in temporary workspaces and require the precise rejection. No additional Agent task platform is needed.

PR checks must pass against the submitted revision. Repository rules enforce required checks where configured; a workflow file alone does not establish merge protection. Preserve unexecuted or unavailable results rather than claiming a pass.

Contribute only material you have the right to contribute under the project's license, preserving required third-party notices. Report security issues as described in SECURITY.md.

## Main branch merge controls

The repository owner authorized public visibility on 2026-10-24. The effective `main` protection requires the `MadB C01 baseline` check and an up-to-date PR branch, applies to administrators, and prohibits force pushes and branch deletion. The required check is associated with GitHub Actions. A workflow file alone is not proof that these controls are active.

The reproducible request is stored in `.github/branch-protection.json`. An authorized maintainer can apply it with `gh api --method PUT repos/changyue1053/MadB/branches/main/protection --input .github/branch-protection.json`. Read back the effective settings and validate that a failing required check blocks merging after changing repository visibility, permissions or merge controls. Do not weaken the required checks to resolve a configuration failure.

C01 verification is recorded in the [MadB review](docs/reviews/2026-10-24-c01-public-repositories.md). Repository visibility does not imply a software release or platform certification; the current task catalog and the PR's actual checks remain the acceptance evidence.
