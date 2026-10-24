# C04 single-task and captured-result regressions

Run from the repository root with the project's locked development environment:

```sh
.venv/bin/python -m unittest discover -s tests/integration/c04_tasks -v
```

The suite builds the test-only `task_probe` composition root with `--locked`, then exercises the real contracts, app and host layers with a real Python recipe. It adds no installed CLI command. Unix permissions, signals, FIFO, symlinks, hardlinks and `ps` are required; missing prerequisites fail instead of becoming skipped passes.

## Independent inputs and observations

`fixture.py` materializes the existing synthetic C02 context using the independent C04 Python fixture helpers. It replaces the profile/lock tool image with the resolved real Python executable and recomputes profile, project, task and byte-reference identities with Python `hashlib` and the documented ASCII canonical projection. No production Rust function manufactures expected identities or test verdicts, and no checked-in golden fixture is rewritten.

The profile/IR target remains a synthetic C02 contract identity. Executing this Python control fixture does not compile or certify a native SDK for that target. `input_digest` retains the checked IR source lineage and differs from the per-task file inventory, which independently includes the actual helper and value bytes. `generation_id` is carried from the existing fixture for request/result matching; the test does not prove that generation was committed.

The recipe receives the checked BuildTask on stdin, reads `../inputs/value`, writes `output.bin` and `logs/build.txt`, and returns BuildEvidence on stdout. The test's expected input, transformed bytes and log bytes are fixed independently in Python. Assertions compare captured bytes, hashes, counts, raw process observations and actual files; an exit-zero or `passed` claim alone never satisfies the oracle.

The driver only exposes observations returned by the production path. Fixture subprocess errors, driver panics, unexpected stderr and watchdog expiry fail the test rather than being interpreted as expected rejections.

## Executed boundary matrix

| Cases | Required result |
|---|---|
| Normal recipe | Captured output and log bytes/hash/size match the independent oracle; produced work files remain after Drop. |
| Source changes after preparation | The actual recipe still reads the captured input bytes. |
| Work file changes after result capture | The retained value keeps its original captured bytes while the changed disk file is separately observable. |
| Wrong task digest/source lineage/selector, unexecuted dependencies, unsupported cwd | Binding fails before allocation or process start. |
| Component package substituted for an image, installed-image digest mismatch | `LOCK004`; no run or helper marker. |
| Profile/lock bytes differ from request references | `FILE004`; no execution. |
| Duplicate environment names, output directory case aliases or ancestors | Existing digest projection reports `HASH004` for duplicate set identity; output path checks report `TASK001`. |
| Declared input bytes no longer match | Preparation fails before a run directory is allocated. |
| Zero exit with malformed payload or wrong task/generation/profile/input identity | The result is rejected with the precise format/port diagnostic. |
| Wrong output/log hash, missing output/log, extra file/directory | `EXEC018` wraps the specific input/inventory validation failure; no result value escapes. |
| Symlink, hardlink or FIFO output; oversized declaration or actual file | Bounded filesystem/manifest checks reject, without blocking on FIFO or reading the oversized file into memory. |
| Failed evidence, output/log overlap or parent-directory case alias | Explicit status/inventory failure despite exit zero. |
| Nonzero process claims passed | Process admission is unavailable; the result validator is never invoked. |
| Input changed during execution | `EXEC017`; result validation is not entered. |
| Pre-cancel and running cancellation | No value is exposed; the first case never starts a helper, the running case retains partial work. |
| Cancellation inside result validation | The real process has exited zero; the wrapper first runs the real evidence validator, then requests cancellation before returning. Final admission is rejected and captured value is absent. |
| Input or work directory replaced inside result validation | The wrapper first validates the evidence, then changes the input bytes or renames work and creates a new work directory. Final verification returns `EXEC017`; no value escapes. |
| Late cancellation | Returns `AlreadyFinished`; it does not revoke the accepted observation. |

The validation wrappers target a defined execution phase, not elapsed wall time. In the work-replacement case, output capture still reads through the already opened descriptor; the final directory-identity check must reject the replaced named directory. This prevents an unrelated missing-file check from accidentally satisfying the regression.

## Fixture process ownership and cleanup

A unique external marker path is passed as an explicit argument. The helper writes its PID there; the running-cancellation mode additionally writes a readiness flag after installing its signal handler. The marker is deliberately outside work so it cannot pollute the exact output/log inventory. The fixture is trusted and this path is not a confinement claim.

Before signalling a recorded PID, teardown inspects `ps` and requires both the unique marker argument and `../inputs/helper.py`; exited and zombie processes are ignored. The harness stops matching live workers and confirms they are no longer running before restoring test-owned directory permissions and deleting the temporary tree. It never follows tampered symlinks while changing permissions. Normal-exit subcases likewise check that their helper has stopped before removing their run. This is test-only cleanup, not a production GC policy.

Readiness polling coordinates running cancellation. The result-validation callbacks establish the other ordering points deterministically. A three-second process budget, five-second readiness bound, twelve-second driver watchdog and fifteen-second helper alarm bound failures; fixed sleeps are polling intervals, not race oracles.

All successful and rejected executions retain their production directory owner until inspected, then Drop leaves the directory in place. These tests do not prove a complete dependency closure, consumed-generation provenance, DAG scheduling, E02 frames, three-layer commits, current-index updates, cache correctness, native SDK behavior, absence of ABA changes or hostile-process isolation.
