# C04 controlled process regressions

Run `.venv/bin/python -m unittest discover -s tests/integration/c04_process` from the repository root. The suite compiles a test-only Rust example that calls the real app/host ports, then launches a real Python helper. It does not add an installed CLI command.

Assertions cover literal argv, an empty-by-default environment with an explicit allowlist, tool bytes, descriptor-checked work directories, bounded pipes, concurrent stdin/stderr backpressure, exits/signals, cancellation, escalation, process-group workers and cleanup gaps. Readiness barriers order cancellation and mutation; deadlines bound execution rather than pretending to prove scheduling. Test-owned tool wrappers are the only executable images modified in mutation tests.

The process gate's deterministic concurrent-start/cancel/result races live in `crates/madb-app/tests/process_admission.rs`. The driver serializes observations; it never decides the expected outcome. These tests do not prove E02 request state, payload validity, complete toolchain identity, transactional publication, an OS sandbox, or termination of escaped descendants. An escaped worker intentionally retains pipes; the suite expects a cleanup gap and then explicitly cleans its own worker. PID cleanup checks the unique helper path to avoid signalling an unrelated reused PID.

Linux and macOS are supported host targets. The harness requires Unix fork, signals and `ps`; absence is a failure, not a skipped pass. A 12-second external watchdog and helper alarms make a stuck execution a test failure.
