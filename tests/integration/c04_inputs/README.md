# C04 captured-input and retained-run regressions

Run from the repository root:

```sh
python3 -m unittest discover -s tests/integration/c04_inputs -v
```

The suite builds the test-only `frozen_probe` Rust example with `--locked`, then invokes the real app/host ports and a real Python helper. It adds no installed CLI command. Linux and macOS are supported; Unix permissions, links, FIFO, `fork`, signals and `ps` are required. Missing prerequisites fail the suite rather than producing a skipped pass.

## Independent fixture oracle

Python creates the source files and computes their SHA-256 digests and byte counts with `hashlib`. It writes an independent JSON fixture containing `source_root`, `run_parent`, `run_name`, `file_refs`, the resolved Python tool path/digest/size, and the requested test `mode`/`action`. The Rust driver consumes these expectations; it does not use production capture or materialization code to manufacture the expected manifest.

The tool runs from `work/` with explicit `../inputs/helper.py` and `../inputs/value` arguments. Its normal output is calculated by reading the actual copied value file, then writing observed text, digest and length to both stdout and `work/result.json`. Python compares those observations with independently prepared expectations and also inspects the filesystem. The driver serializes actual admission, diagnostic, termination, cleanup and directory observations; it contains no hardcoded success verdict.

Mutation modes deliberately write `tool_claim: complete` and exit zero after changing the input copy. That text is an untrusted tool claim used by a negative test: acceptance requires the host to reject it with `EXEC017`, not to accept the claim.

## Real-process and filesystem matrix

| Scenario | Required observation |
| --- | --- |
| Normal execution | Actual copied bytes match the independent expected text/hash/length; source and copy have distinct inodes; output remains in `work/`. |
| Source changes after capture | The tool still reads the captured bytes; the source change remains visible only in the original source. |
| Prepared nested inputs | Run root and `work/` are `0700`, input directories `0500`, input files `0400`; dropping the prepared owner retains the directory. |
| Existing directory, existing symlink or nested run name | `INPUT005`; existing contents stay intact and `retained_path` is absent because this invocation did not create the directory. |
| Wrong expected digest or missing declared input | `INPUT003` during capture; no run directory is created. |
| Real symlink, hardlink or FIFO source | `HOST002` during capture; no run directory is created. |
| Input mutation before spawn | Content change, extra file, extra directory, missing file, symlink replacement and hardlink replacement each produce `EXEC017` at `frozen-inputs`; the helper never starts. |
| Input mutation during execution | The same six changes reject even a zero exit with admission `Unavailable` and `EXEC017`; raw process observations and produced files remain available. |
| Working directory other than `work` | `EXEC002`; no helper starts and the prepared directory remains. |
| Cancellation before or during execution | `RejectedByCancellation`; no child starts in the first case, while the running case retains partial work and observes the leader reaped. |
| Escaped child still holding pipes | `Unavailable` with `EXEC016`, incomplete local cleanup and an observed live helper child; the run directory remains. |
| Escaped child with standard pipes closed | Local cleanup may be complete and admission may be `EligibleForValidation`, while the child is still observed alive; dropping the run owner still retains the directory. |

`EligibleForValidation` only permits subsequent validation. It does not mean artifact correctness, transaction commit or publication succeeded. All created run directories are retained by the production owner, including normal execution, cancellation and rejected results. A retained path identifies an owned directory for later inspection; it grants no automatic deletion permission.

## Fixture process ownership and cleanup

The helper records its real PIDs in `work/pids`. Its command line includes the unique temporary run path. Before signalling a recorded PID, the harness checks `ps` for that marker and `../inputs/helper.py`, and ignores exited/zombie processes. It sends `SIGKILL` only to matching live fixture workers, then polls up to two seconds and asserts none of those workers remain running. Only after that assertion does it restore permissions on the test-owned tree, without following symlinks, and remove the temporary fixture directory. This cleanup belongs to the test harness; it is not a production `Drop` or GC policy.

Cancellation waits for a helper-created readiness marker. Escaped-child cases use a pipe handshake before the leader exits. Polling observes these barriers; fixed sleeps do not decide when the tested race occurred. Process timeouts, readiness deadlines, a 12-second driver watchdog and 15-second helper alarms bound failures. A panic, nonzero driver exit, unexpected stderr or watchdog expiry fails the test; it is never interpreted as an expected rejection.

This suite verifies the declared-file slice. It does not establish a complete dependency closure, source directory membership or missing-file dependencies, complete toolchain identity, absence of transient/ABA mutations, adversarial isolation, DAG scheduling, E02 request state, transactional publication, or termination of arbitrary escaped descendants. Read-only permissions reduce accidental mutation; they are not a sandbox. The tests expressly demonstrate why direct-child/pipe completion cannot authorize automatic directory deletion.
