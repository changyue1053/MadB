@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package com.madb.c07
/** Ordinary compilation contains no mutable controls or exported test setters. */
internal object ExecutionProbe {
    fun nativeAllocated(size: Int) {}
    fun nativeFreed(size: Int) {}
    fun ledgerCharge(size: Int) {}
    fun ledgerRetire(size: Int) {}
    fun stagingAdd(size: Int) {}
    fun stagingEnd(size: Int, committed: Boolean) {}
    fun memoryMetric(index: Int): ULong = ULong.MAX_VALUE

    fun bindNapi(env: kotlinx.cinterop.COpaquePointer?) {}
    fun admissionStage(requested: Int): Int = -1
    fun beginEpoch() { }
    fun work(allowed: () -> Boolean) { }
    fun beforeWorkExit(allowed: () -> Boolean) { }
    fun source(allowed: () -> Boolean) { }
    fun selectedWork(generation: ULong, id: ULong) { }
    fun committedWork(status: Int) { }
    fun set(kind: Int, enabled: Int) { fail("Unsupported") }
    val deliveryState: Int get() = 0
    val deliveryKey: String? get() = null
    fun endEngine() { }
    fun stoppedSubscription(id: ULong) { }
    fun armDelivery(key: String, id: ULong) { fail("Unsupported") }
    fun takeDelivery(key: String, id: ULong): Boolean = false
    fun releaseDelivery() { fail("Unsupported") }
    fun finishDelivery(invoked: Boolean) { }
    fun publicationHeld(selected: Int, bytes: Int) { }
    fun publicationDone(committed: Boolean) { }
    fun armClone(nth: Int, mode: Int) { fail("Unsupported") }
    fun cloneFault(): Int = -1
    fun allocated() { }
    fun released() { }
    fun armTransfer(phase: Int) { fail("Unsupported") }
    fun transfer(phase: Int) { }
    fun guardEntered() { }
    fun guardReleased() { }
    fun claimRolledBack() { }
    fun frame(): String = ""
    fun metric(index: Int): ULong = 0uL
}
