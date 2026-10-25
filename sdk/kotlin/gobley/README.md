# Kotlin/Native Gobley SDK

C06 provides generated `madb.boundary` Kotlin APIs, a bindings KLIB and its cinterop KLIB and the matching Rust dynamic library (`@rpath/libmadb_boundary_gobley.dylib`). Consumers link the dynamic library explicitly and set its runtime search path; the KLIB does not embed a competing static copy. Consumers also need the locked coroutines and atomicfu KLIBs. The producer packages the generated sources and runtime lock so an independent consumer can rebuild and verify the candidate.

```kotlin
import kotlinx.coroutines.runBlocking
import madb.boundary.*

fun main(args: Array<String>) = runBlocking {
    // Pass the profile identity pinned by the consuming application/release.
    val engine = openEngine(args.single())
    try {
        val input = Value("message-1", null, "你好", emptyList(), Phase.READY)
        val operation = engine.startOperation(input, object : Host {
            override fun transform(value: Value): Value = value.copy(text = "received:" + value.text)
        })
        try {
            println(operation.result().text)
        } finally {
            operation.requestCancel()
            operation.destroy()
        }
    } finally {
        engine.requestClose()
        // Await outside a cancelled caller context when the owner requires full teardown.
        kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) { engine.awaitClosed() }
        engine.destroy()
    }
}
```

`result()` cancellation stops this wait; `requestCancel()` requests operation cancellation. A callback already executing is not interrupted; the engine drains it and discards a cancelled late result. `requestClose()` rejects new work; `awaitClosed()` completes after owned work and callbacks drain. Dispose generated wrappers explicitly after the applicable protocol completes; their cleaner is a fallback. A shared engine remains owned by its creator, not by a page observer.

State subscriptions keep the declared latest state; event admission reports capacity exhaustion. Item, payload-byte and callback budgets belong to the public bounded engine. The candidate identifies this exact macOS arm64 Release configuration; no other Kotlin target is certified by these artifacts. `test-support` callbacks are separate P03 assets and not part of the production engine close scope.
