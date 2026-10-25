package madb.boundary

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.*

private suspend fun verifyCompletionThrow(error: Boolean) {
    val success = AtomicInteger()
    val failure = AtomicInteger()
    val uncaught = AtomicInteger()
    val old = Thread.getDefaultUncaughtExceptionHandler()
    Thread.setDefaultUncaughtExceptionHandler { _, exception ->
        check(exception.message == "receiver threw after ownership transfer")
        uncaught.incrementAndGet()
    }
    try {
        val dropped = UniffiForeignFutureDroppedCallbackStruct()
        uniffiTraitInterfaceCallAsyncWithError<Int, BoundaryException>(
            makeCall = { if (error) throw BoundaryException.CallbackFailure("typed business error") else 41 },
            handleSuccess = { check(it == 41); success.incrementAndGet(); throw IllegalStateException("receiver threw after ownership transfer") },
            handleError = { status ->
                failure.incrementAndGet()
                RustBuffer.free(status.error_buf)
                throw IllegalStateException("receiver threw after ownership transfer")
            },
            lowerError = { FfiConverterTypeBoundaryError.lower(it) },
            uniffiOutDroppedCallback = dropped,
        )
        uniffiForeignFutureHandleMap.get(dropped.handle).join()
        dropped.free!!.callback(dropped.handle)
        check(success.get() == if (error) 0 else 1)
        check(failure.get() == if (error) 1 else 0)
        check(uncaught.get() == 1)
        check(uniffiForeignFutureHandleCount() == 0)
    } finally {
        Thread.setDefaultUncaughtExceptionHandler(old)
    }
}

@OptIn(DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
fun main() = runBlocking {
    withTimeout(15_000) {
        verifyCompletionThrow(false)
        verifyCompletionThrow(true)
        val value = Value("m1", Long.MIN_VALUE, "你好\u0000é", listOf(0u,127u,128u,255u), Phase.DONE)
        newSingleThreadContext("madb-p03-host").use { executor ->
            val echo = object : AsyncHost {
                override suspend fun transform(value: Value): Value = withContext(executor) { yield(); value }
            }
            check(asyncHostTransform(value, echo) == value)
            try {
                asyncHostTransform(value, object : AsyncHost {
                    override suspend fun transform(value: Value): Value = throw BoundaryException.CallbackFailure("typed business error")
                })
                error("typed error was lost")
            } catch (_: BoundaryException.CallbackFailure) { }
            val entered = CompletableDeferred<Unit>()
            val release = CompletableDeferred<Unit>()
            val cleanup = CompletableDeferred<Unit>()
            val trampoline = CompletableDeferred<Job>()
            val lateReturns = AtomicInteger()
            val invocations = AtomicInteger()
            val late = object : AsyncHost {
                override suspend fun transform(value: Value): Value {
                    trampoline.complete(currentCoroutineContext()[Job]!!)
                    try {
                        withContext(executor + NonCancellable) {
                            invocations.incrementAndGet()
                            entered.complete(Unit)
                            try { release.await() } finally { cleanup.complete(Unit) }
                        }
                    } catch (_: CancellationException) {
                        // Deliberately simulate a provider that completes despite cancellation.
                    }
                    lateReturns.incrementAndGet()
                    return value
                }
            }
            val call = launch { asyncHostTransform(value, late); error("cancelled caller resumed") }
            entered.await()
            call.cancelAndJoin()
            check(!cleanup.isCompleted)
            release.complete(Unit)
            cleanup.await()
            // Backend trampoline is joined separately from caller cancellation. This
            // acknowledged fixture does not claim Engine.close owns its coroutine.
            trampoline.await().join()
            check(invocations.get() == 1)
            check(lateReturns.get() == 1)
            check(uniffiForeignFutureHandleCount() == 0)
        }
        println("{\"check\":\"uniffi-jvm-p03\",\"actual\":\"passed\",\"cases\":5}")
    }
}
