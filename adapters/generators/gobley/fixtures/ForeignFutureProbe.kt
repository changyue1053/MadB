@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package madb.boundary

import kotlinx.cinterop.invoke
import kotlinx.cinterop.CFunction
import kotlinx.cinterop.reinterpret
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.atomicfu.atomic

// Same generated helper and generated Rust object used by the binding. Throwing
// here stays within Kotlin; no exception crosses an extern C callback frame.
private suspend fun notificationFailure() {
    val ledger = TestLedger()
    val resource = ledger.createResource()
    val transfers = atomic(0)
    val errorTerminals = atomic(0)
    val future = uniffiTraitInterfaceCallAsync(
        makeCall = { resource },
        handleSuccess = { owned ->
            transfers.incrementAndGet()
            owned.destroy()
            throw IllegalStateException("fault after completion handoff")
        },
        handleError = { status ->
            errorTerminals.incrementAndGet()
            RustBufferHelper.free(status.errorBuf)
        },
    )
    val job = uniffiForeignFutureHandleMap.get(future.handle)
    job.join()
    future.free!!.reinterpret<CFunction<(ULong) -> Unit>>().invoke(future.handle.toULong())
    val releases = ledger.released()
    ledger.destroy()
    println("notification transfer=${transfers.value} error=${errorTerminals.value} released=$releases")
    check(transfers.value == 1 && errorTerminals.value == 0 && releases == 1uL)
}

private suspend fun cancellationWithLateResult() {
    val ledger = TestLedger()
    val started = CompletableDeferred<Unit>()
    val cancelled = CompletableDeferred<Unit>()
    val finish = CompletableDeferred<Unit>()
    val transfers = atomic(0)
    val errorTerminals = atomic(0)
    val future = uniffiTraitInterfaceCallAsync(
        makeCall = {
            started.complete(Unit)
            try {
                finish.await()
            } catch (expected: CancellationException) {
                cancelled.complete(Unit)
                withContext(NonCancellable) { finish.await() }
            }
            ledger.createResource()
        },
        handleSuccess = { owned -> transfers.incrementAndGet(); owned.destroy() },
        handleError = { status -> errorTerminals.incrementAndGet(); RustBufferHelper.free(status.errorBuf) },
    )
    val job = uniffiForeignFutureHandleMap.get(future.handle)
    started.await()
    future.free!!.reinterpret<CFunction<(ULong) -> Unit>>().invoke(future.handle.toULong())
    cancelled.await()
    finish.complete(Unit)
    job.join()
    val releases = ledger.released()
    ledger.destroy()
    println("late transfer=${transfers.value} error=${errorTerminals.value} released=$releases")
    check(transfers.value == 1 && errorTerminals.value == 0 && releases == 1uL)
}

private suspend fun loweringFailure() {
    val success = atomic(0)
    val errors = atomic(0)
    val future = uniffiTraitInterfaceCallAsync<Unit>(
        makeCall = { throw IllegalArgumentException("fault before handoff") },
        handleSuccess = { success.incrementAndGet() },
        handleError = { status -> errors.incrementAndGet(); RustBufferHelper.free(status.errorBuf) },
    )
    uniffiForeignFutureHandleMap.get(future.handle).join()
    future.free!!.reinterpret<CFunction<(ULong) -> Unit>>().invoke(future.handle.toULong())
    check(success.value == 0 && errors.value == 1)
}

fun main() = runBlocking {
    notificationFailure()
    cancellationWithLateResult()
    loweringFailure()
    unsignedRoundtrips()
    check(uniffiForeignFutureHandleMap.size == 0)
    println("P03_NATIVE_PATCH_PASSED")
}

private fun unsignedRoundtrips() {
    val host = object : UnsignedHost {
        override fun echoU8(value: UByte): UByte = value.toInt().inv().toUByte()
        override fun echoU16(value: UShort): UShort = value.toInt().inv().toUShort()
        override fun echoU32(value: UInt): UInt = value.inv()
        override fun echoU64(value: ULong): ULong = value.inv()
    }
    for (value in listOf<UByte>(0u, 128u, 255u)) {
        check(unsignedU8(value) == value)
        check(unsignedViaHostU8(value, host) == value.toInt().inv().toUByte())
    }
    for (value in listOf<UShort>(0u, 32768u, 65535u)) {
        check(unsignedU16(value) == value)
        check(unsignedViaHostU16(value, host) == value.toInt().inv().toUShort())
    }
    for (value in listOf<UInt>(0u, 0x80000000u, UInt.MAX_VALUE)) {
        check(unsignedU32(value) == value)
        check(unsignedViaHostU32(value, host) == value.inv())
    }
    for (value in listOf<ULong>(0uL, 0x8000000000000000uL, ULong.MAX_VALUE)) {
        check(unsignedU64(value) == value)
        check(unsignedViaHostU64(value, host) == value.inv())
    }
    println("UNSIGNED_NATIVE_FORWARD_REVERSE_PASSED")
}
