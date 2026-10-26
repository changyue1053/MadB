# L04 Observation local integration checks

Run from the repository root using the pinned Python and Apple toolchain. The
output directory must be new; it retains copied inputs, isolated build caches,
command logs and a `report.json` with source hashes and actual check results.

```sh
python3 -m unittest discover -s tests/integration/l04_swift_observation -v
python3 scripts/check_swift_observation.py --output target/observation-package-check
```

The default command runs the standalone package's Release tests and a separate
SwiftPM executable that imports its public product, receives observable state,
rejects duplicate registration, and confirms stop. It does not require a native
candidate; its report explicitly records `native_composition: not_requested`.

To additionally check the SKIE adapter against an installed public candidate:

```sh
python3 scripts/check_swift_observation.py \
  --output target/observation-skie-check \
  --candidate "$MADB_PUBLIC_SKIE_CANDIDATE" \
  --expected-candidate-digest "$MADB_TRUSTED_CANDIDATE_DIGEST"
```

Supply the expected digest from the trusted candidate delivery record, rather
than deriving it from the candidate being checked. A missing candidate, digest
mismatch, changed file, unlisted file, symbolic link, identity mismatch or wrong
toolchain fails the command before native consumption. The admission unit tests
use clearly synthetic, non-executable bytes; they never claim native coverage.

The native consumer receives only the verified candidate's public framework and
Swift facade, together with the new extension's copied public source and SKIE
integration. It uses the verified identity to check the loaded library. It checks
initial `nil`, pending-state coalescing, real values and owned bytes, duplicate
start, stop, duplicate stop, restart without invented replay, the shared engine
remaining usable, and zero native resource/payload/waiter counts after stop and
normal engine close. MainActor polling has deadlines, and the runner also has a
process timeout; either expiry is a failure.

The same native consumer also starts two independent observation models on one
engine. Both receive state; stopping either model leaves the other subscribed and
receiving, while the stopped model retains its last snapshot. Reentering one
model creates only its own subscription without replaying old state or resetting
the other model. Final observer cleanup returns native resource, payload and
waiter counts to zero before the application owner closes the engine. These are
ordinary model/owner checks for DX-05/06/08, not SwiftUI or UIKit page execution.

This is a new extension plus an existing C06 candidate: a local macOS Arm64
**development composition**, not a new C10 candidate, official package publishing,
iOS runtime, UIKit, or full-platform certification. No Kotlin/Gradle/Cargo source
or producer-private facade is compiled into the consumer. SwiftPM sandbox
settings and module/cache directories are explicit; host permission failures are
reported as failures and must be resolved without dropping checks.
