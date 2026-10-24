# Explicit CLI process budgets

`generate`, `build` (including `build --task metadata`) and `package` accept two
explicit caller-authorized process budgets:

- `--process-timeout-seconds SECONDS`: execution deadline per child process,
  default **30**, integer range **1..3600**.
- `--process-termination-grace-ms MILLISECONDS`: time allowed between SIGTERM and
  forced SIGKILL, default **250**, integer range **0..30000**. Zero permits
  immediate escalation. A tool that owns separate worker groups must drain those
  workers and release its locks within this bounded grace.

For an explicitly selected compiler owner requiring up to five seconds to drain:

```sh
madb build --root <workspace> --execution build-execution.json --frozen --process-timeout-seconds 900 --process-termination-grace-ms 6000
```

`build --task dependencies` uses a separate dependency-discovery interface. It
retains its existing defaults and explicitly rejects non-default values for
these options; neither budget is silently ignored.

The execution deadline begins after spawn and pipe setup. Neither setting is a
total workflow deadline. Termination grace changes only host process shutdown,
not SDK task cancellation, native-resource ownership or the separate pipe/child
cleanup limit. An outer watchdog must allow the graph's execution and bounded
cleanup work. No execution wire document or global process-policy default changes.

`cargo test -p madb-cli --locked` checks argument bounds and unchanged defaults.
Real process tests use the CLI's composed policy: a two-second child reaches a
one-second deadline and succeeds with three seconds; a cancelled owner drains a
separate child process group requiring 700ms of cleanup within a 2000ms grace,
releases its file lock and permits a new owner to retry. This last test exercises
host process supervision with CLI options, not the complete C06 compiler pipeline.
