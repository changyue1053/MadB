# L05 state observation host checks

Task scope: the initial optional state/page lifecycle slice under
`sdk/arkts/observation`, using the ordinary C07 API and ADR-0013. It does not
modify C07, its budgets, generation, native owners or completion protocol.

Run the project-pinned Node version from the repository root:

```sh
node tests/integration/l05_arkts_observation/check.cjs
```

The check executes the actual two `.ets` sources after type erasure. It runs
real host Promise microtasks against an independently controlled Engine and
manually released close barriers. It covers inert construction, single
registration, stop-versus-drain, entered/nested delivery, synchronous registration
stop/close, concurrent and reentrant close, stale generations, failure identity,
retained ownership and retry, registration failure, and snapshot reset semantics.
Fake Engine close methods fail if the page adapter attempts to close a shared
Engine or subscribe to the event lane.

The output records source SHA-256 values, Node version and successful cases.
It writes no receipt or generated source into the working tree. A failure exits
nonzero; no test is skipped or converted to pass when an environment is missing.

The host deliberately removes `@ObservedV2`/`@Trace`; it does not implement or
simulate ArkUI notification semantics. Type erasure is not ArkTS compilation.
The controlled close ACK is a test input, not proof of native cleanup. Formal
SDK compilation, actual target UI notifications, lifecycle/device/env tests,
native drain and installed candidate consumption remain distinct checks. Neither
this check nor an SDK compiler result certifies C07 or completes L05.

The [API26 SDK probe](SDK_README.md) provides a separate reproducible compiler,
declaration-export and local HAR consumer check. It does not replace target tests.

## Public development source delivery

Export a fresh, explicit input directory for the independent Lab consumer:

```sh
python3 -B tests/integration/l05_arkts_observation/prepare_sources.py \
  --output /absolute/path/to/fresh/l05-source
```

The output contains the exact public observation sources, their README, LICENSE,
NOTICE and a byte/size inventory. Freeze the source SHA-256 values independently
before passing the directory and trusted hashes to Lab's `scripts/verify_l05.py`.
Lab consumes this delivery and a separately frozen public C ordinary HAR; it
does not import this checkout or regenerate the candidate. The small Lab
`Boundary.ets` shim re-exports only the candidate's public types.

That separate runner builds an actual Release HAP and verifies real Native
dual-observer/stop/reentry behavior plus actual ArkUI V2 page hide/return and
visible updates. Its report distinguishes compilation, Native execution, UI
hierarchy assertions and cleanup, and binds all results to exact inputs and the
selected target. Development source delivery is not C10 packaging or publication;
the frozen C07 candidate's certification is not transferred to new source code.
