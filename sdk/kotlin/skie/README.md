# C06 Kotlin/Native → Swift SDK

This is a conformance-provider SDK with an explicitly exported GC and completion-race observation hooks, rather than a general production SDK without test exports. This profile uses Objective-C export, Kotlin 2.4.20, coroutines 1.11.0 and SKIE 0.10.15. Its executed target is **macOS arm64, minimum macOS 14**, with a static Release framework. It makes no iOS, simulator, Combine, NativeCoroutines or Swift-export claim. The reverse direction is the Objective-C-visible synchronous `Host.transform` protocol, invoked on the engine's owned serial worker.

## Source and generated responsibilities

- `Boundary.kt` owns admission, one worker per engine, explicit work cancellation, result ownership, bounded observations and close. SKIE owns the actual suspend/Flow/sealed/enum generation.
- A cancelled `Operation.run()` wait does not cancel the engine-owned work. `requestCancel()` requests cooperative work cancellation; a running synchronous host callback must return before completion is confirmed.
- `requestStop()` rejects new observation delivery and discards queued values. `awaitStopped()` confirms the native collection has exited. The Swift facade also gates and drains its consumer callback body, including a value already handed off by SKIE.
- `requestClose()` prevents new business admission. `awaitClosed()` awaits operation cleanup acknowledgements and native collections, then closes the owned worker. A coroutine job can be completed before its completion handler clears references; job completion alone is insufficient. Cancellation arriving between cleanup and its acknowledgement is rechecked before releasing the operation registry. One owned shutdown coroutine runs on the shared `Dispatchers.Default` executor; it joins the business worker outside the engine lock and publishes a shared shutdown acknowledgement. Cancelling an awaiter leaves this shutdown work running. The facade additionally drains its registered Swift deliveries. Calling an await-stop/close from its own Swift delivery reports `wouldDeadlock`; the nonblocking request is permitted.
- `TestSupport.collectGarbage()` observes final ARC/GC release after explicit close. The per-engine `armCompletionCleanup` and `armCompletionAcknowledgement` latches pause the two completion windows; `completionCleanupEntered` is nonblocking and `releaseCompletionCleanup` releases the latch. These hooks provide deterministic conformance evidence. Business cleanup never depends on GC timing or a test hook.

The engine has a **global eight pending event deliveries**, at most eight operations and eight observations. Each state observer retains at most one pending latest state. The global retained variable-payload budget is 4096 bytes; a payload is at most 1024 bytes. Weight is `UTF8(id).count + UTF8(text).count + bytes.count`; fixed record/enum/optional overhead is bounded by item limits, not represented as measured heap usage. Input, queued, emitting and undelivered result responsibilities count until handoff or cleanup. Fan-out checks the full admission before mutating any observer.

## Swift 6 consumption

`../../swift/skie/MadBSkieConcurrency.swift` supplies three resource-specific handles and copied Sendable value/event snapshots. Create the facade with `try MadBEngineHandle(expectedProfile: ...)`; it creates the native engine and owns its Swift delivery registrations. Obtain observations with `try engineHandle.subscribeEvents()` or `subscribeState()`. Existing native engines and foreign subscriptions cannot be attached to another facade owner. Copies share the same owner. A duplicate collection is rejected before installing cleanup, so it cannot stop the first collector. Public asynchronous wait admission is bounded at 32 and cancellation releases its permit. These wrappers add no producer task or payload queue and do not grant blanket `Sendable` conformance to Kotlin classes.

The narrow `@unchecked Sendable` resource claims rest on the actual Kotlin lock/Deferred/worker protocols. The imported mutable DTO is copied before crossing Swift actors. The Swift callback gate covers only admission and active callback leases after `AsyncSequence.next()` transfers a value. Ordinary generated APIs remain usable within their declared thread/ownership constraints.

## Frozen build

The Gradle wrapper pins 9.7.1 and its official distribution SHA256. Use the C06 toolchain installer/receipt for the exact JDK, Kotlin Native, LLVM and Maven closure. `gradle/verification-metadata.xml` and `gradle.lockfile` reject changed dependencies. With `-Pmadb.mavenRepo=<verified local repository>`, plugin and dependency resolution use only that repository, with no network fallback.

```sh
./gradlew --no-daemon --offline --dependency-verification=strict \
  -Pmadb.mavenRepo=/path/to/verified/maven-repository \
  -Pkotlin.native.home=/path/to/verified/kotlin-native \
  -PmadbProfileDigest=<64-hex-resolved-profile-digest> \
  -PmadbInterfaceDigest=<64-hex-interface-digest> \
  linkReleaseFrameworkMacosArm64
```

`openEngine(expectedProfile)` compares the standard `sha256:<hex>` profile identity before allocating its worker. Interface identity, native ABI version and bootstrap version are separately exposed. Framework output is `build/bin/macosArm64/releaseFramework/MadBSkie.framework`; SKIE explicitly enables `produceDistributableFramework()`.

The C04 producer recipe uses `/private/tmp/madb-compiler/c06-skie/<portable-input-digest>` because Kotlin emits an unmapped absolute DWARF compile-unit output path. The digest binds source bytes, profile/interface identity and portable tool/dependency hashes. Each identity is protected by an exclusive bounded file lock and owned-directory checks. Every invocation clears its known owned build contents and builds cold; compiler caches are not reused. Normal cancellation kills the dedicated compiler process group before releasing the lock. Abrupt termination leaves a process marker and refuses automatic cleanup until orphan-process recovery. The resolved profile explicitly declares this managed workspace.

Published frameworks contain regular files, headers, binary, module interfaces and DWARF. The optional `.swiftsourceinfo` editor source index is omitted; IDE consumers can navigate generated interfaces. Swift code generation disables incremental LLVM hashing to keep reproducible object bytes. The distributable framework retains its debug information and UUID behavior.

Public API metadata comes from `adapters/generators/skie/read_api.py`: Clang JSON AST of the generated Objective-C header plus Swift SymbolGraph from the actual compiled module. This captures parameter types, protocol direction and enhanced `async`/Flow/enum signatures rather than assuming declarations from filenames. Promised APIs must still be called by the independent Swift consumer.

## Known shape diagnostics

`adapters/generators/skie/probes` deliberately exports a real custom Objective-C cinterop type without `ClassInterop.CInteropFrameworkName`, and a generic lambda type. Swift consumers calling these generated methods must fail with the specific unavailable diagnostic. No test fixture injects the unavailable annotations itself. Adding an origin mapping also requires distributing and linking its framework; arbitrary generic-lambda shapes are not silently advertised as callable.

Upstream sources: [SKIE 0.10.15](https://skie.touchlab.co/changelog/0.10.15), [distributable frameworks](https://skie.touchlab.co/configuration/swift-compiler), [Flow limitations](https://skie.touchlab.co/features/flows), [cinterop origin](https://skie.touchlab.co/known-issues/cinterop), [generic lambda limitation](https://skie.touchlab.co/known-issues/lambda-types-in-type-argument), [Kotlin ARC integration](https://kotlinlang.org/docs/native-arc-integration.html).
