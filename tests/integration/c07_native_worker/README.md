# C07 actual worker checks

From the repository root, using Clang C/C++ on macOS or Linux:

```sh
.venv/bin/python -B tests/integration/c07_native_worker/check.py
```

`worker_tests.cpp` retains ten author behavior cases using the real C17 provider:
100 idle/wake operations, success/domain/internal outcomes, queued and running
cancellation, TLS-exit blocking, full-eight-operation close, owner/self join
rejection, two cleanup callers and rejection before stop. The original assertions
are preserved; formatting and explicit empty-string initialization are the only
fixture cleanup changes.

Two separate lifetime regressions run in both Release and ASan/UBSan:

- `stale_thread_id.cpp`: sixteen new cleanup threads all receive completed join,
  including when the OS reuses an exited business thread's id. The old source
  was actually reproduced failing this assertion before the TLS-marker fix.
- `tls_destructor_self_join.cpp`: a real TLS destructor attempts self-join while
  an external cleanup caller waits. Self-join is rejected; Core retains Executor
  until the destructor exits and real join returns.

`failed_thread_creation.cpp` runs in its own Release process. A tiny test library
forces actual `pthread_create` to return `EAGAIN` using Darwin's process-local
interpose section or Linux `LD_PRELOAD`; no global preload setting is changed.
It verifies failure, zero leaked owners and the one-shot executor rule. This
interposer is never loaded into sanitizer or test-runner processes, because
those may create their own helper threads. Unsupported hosts fail explicitly.
Each result records the actual host; a macOS run does not certify Linux behavior.

Both sanitizer modes cover C/C++ only, with LeakSanitizer disabled. Missing tools,
failed injection, compile errors, a stuck join/timeout or any nonzero test exit
fail the gate. Source SHA-256, commands, process-local overrides and logs remain
in the printed temporary directory. These tests are host lifetime proof, not
OHOS async cleanup, NAPI env, device or full SDK acceptance.
