# C07 private native protocol checks

From the repository root, with Clang C and C++ compilers on PATH:

```sh
.venv/bin/python -B tests/integration/c07_native_protocol/check.py
```

The script builds real C17 provider objects and the C++17 protocol separately,
then runs both suites in Release and ASan/UBSan. Missing compilers, a sanitizer
failure, timeout or nonzero test exit fails the check. Logs and exact commands
remain in the printed temporary directory. LeakSanitizer is explicitly disabled;
allocator counters prove the tested pairs, not every possible heap allocation.

- `author/`: 20 behavior cases, a real malloc/free provider and checked add-one
  business operation; includes 100 concurrent terminal races per configuration.
- `independent/`: 25 independent oracles, a separate allocator ledger and add-seven
  business operation. Expected values, budgets, gated interleavings and exact
  callback releases do not come from the production implementation.

The state pre-entry regression was reproduced against the old implementation:
a value replaced before callback entry must deliver only the latest value. The
original failure and final independent review remain linked in the C07 record.

These are pure host protocol tests. They do not claim real OHOS worker exit,
Node-API references, TSFN, ArkTS Promise, HAR or full candidate acceptance.
