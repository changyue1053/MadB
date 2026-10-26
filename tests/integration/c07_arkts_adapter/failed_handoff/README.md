# Kotlin failed completion handoff regression

This runner compiles the actual `OwnedCore`, `OwnedModel`, `OwnedExports`, and test `ExecutionProbe` sources. The managed return-owner implementation is extracted verbatim up to its NAPI error-reporting functions from the explicitly supplied runtime input. JVM implementations of native memory, NAPI, locks, executor, and lifecycle are visibly marked fixtures. It verifies business ownership and interleavings, not platform cleanup, Kotlin thread attachment, native allocator sizes, physical join, TSFN finalization, or target acceptance.

Pass an immutable repository, optional source overlay, actual patched `ManagedOwnedBoundary.kt`, locked CPF Maven root, JDK binary, and fresh output directory:

```
python3 run.py --repository REPOSITORY [--overlay OVERLAY] \
  --managed-boundary PATCHED_RUNTIME/ManagedOwnedBoundary.kt \
  --maven-root PUBLIC_MAVEN --java JDK/bin/java --output NEW_DIRECTORY
```

All inputs, exact compiler/runner arguments, stdout, stderr, and exit codes are saved in `receipt.json`. No dependencies are downloaded by the runner. Fifteen cases cover healthy result/close settlement, late failed upgrade, repeated handoff, rejection of new native observations, all 32 charged waiters, terminal-before-Work-exit retention and 1→0 monotonicity, entered return owner success/failure, pre-guard orphan recovery, invalid mode, stale runtime, and the separate publication-abort wake defect.

The executor fixture reports an error for every wake after stop. Abandon and rejected commit must never invoke that wake; a committed publication must still invoke it; another genuine wake error must remain a Core failure. This checks the narrow condition without weakening the general executor error contract.

Run `--mutant ignore-handoff`, `--mutant drop-entered`, and `--mutant wake-abandon` into separate fresh outputs. Each mutant changes exactly one production responsibility condition and must fail a relevant ownership assertion. A compile failure is never accepted as a mutant witness. Real KSP/native compilation, observer coverage, public HAR checking, and target physical-ACK evidence remain separate gates.
