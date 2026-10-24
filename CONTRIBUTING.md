# Contributing

Select an existing work package from [the catalog](docs/catalog/implementation-plan.json). In the task or PR state its sub-scope, specification revision, allowed paths and validation. Apply [clean code](docs/engineering/02-clean-code.md) and [the Agent method](docs/engineering/07-agent-development.md) to both human and generated changes.

Set up the exact environment described in README, run `.venv/bin/python scripts/check.py`, and report any target you could not execute. Public format or semantic changes update the authoritative specification and applicable ADR before dependent implementation. Do not treat generated output as its own oracle.

C01's CLI help/version is a real small task for exercising this process. The architecture tests deliberately introduce forbidden dependencies in temporary workspaces and require the precise rejection. No additional Agent task platform is needed.

PR checks must pass against the submitted revision. Repository rules enforce required checks where configured; a workflow file alone does not establish merge protection. Preserve unexecuted or unavailable results rather than claiming a pass.

Contribute only material you have the right to contribute under the project's license, preserving required third-party notices. Report security issues as described in SECURITY.md.

## Private repository merge controls

The C01 branch-protection request is stored in `.github/branch-protection.json`. GitHub currently rejects it with HTTP 403 because the selected private repository lacks the required plan capability. This file is prepared configuration, not an active protection rule; C01 remains unaccepted and its draft PR stays unmerged. Do not make the repository public or remove the check requirement to work around it.

After the account has the required capability, the authorized maintainer can apply the request with `gh api --method PUT repos/changyue1053/MadB/branches/main/protection --input .github/branch-protection.json`, read back the effective configuration, and verify a failing required check blocks merging. Only then may C01's remaining acceptance be assessed.
