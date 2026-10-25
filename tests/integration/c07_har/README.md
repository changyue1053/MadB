# C07 HAR container checks

Run from the repository root:

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/c07_har
```

The 27 focused checks verify content/mode preservation, deterministic ordering,
root handling, unsafe paths, case/Unicode aliases, parent conflicts, links and
special files, PAX recursion, corrupt/truncated input and explicit budgets. These
checks do not invoke Hvigor, install dependencies or contact a target. The
independent cold-build and actual-ohpm observation is linked from the adapter.
